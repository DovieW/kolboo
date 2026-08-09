//! Runtime platform and desktop-session capability detection.
//!
//! Compile-time operating-system checks are not enough on Linux: X11 and
//! Wayland expose different global-input capabilities. Keep environment parsing
//! pure so fallbacks can be tested without mutating the process environment.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinuxDisplayServer {
    X11,
    Wayland,
    Unknown,
}

/// GTK/Tao backend selected for Kolboo's Linux windows.
///
/// This is intentionally separate from `LinuxDisplayServer`: an app can run
/// through XWayland inside a Wayland desktop session. The session classification
/// must remain Wayland for input and clipboard security decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinuxWindowBackendChoice {
    PreserveEnvironment,
    X11,
    Wayland,
}

impl LinuxDisplayServer {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::X11 => "x11",
            Self::Wayland => "wayland",
            Self::Unknown => "unknown",
        }
    }
}

fn non_empty(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

pub(crate) fn detect_linux_display_server(
    session_type: Option<&str>,
    wayland_display: Option<&str>,
    x11_display: Option<&str>,
) -> LinuxDisplayServer {
    match session_type.map(str::trim).map(str::to_ascii_lowercase) {
        Some(session) if session == "wayland" => return LinuxDisplayServer::Wayland,
        Some(session) if session == "x11" => return LinuxDisplayServer::X11,
        _ => {}
    }

    if non_empty(wayland_display) {
        return LinuxDisplayServer::Wayland;
    }

    if non_empty(x11_display) {
        return LinuxDisplayServer::X11;
    }

    LinuxDisplayServer::Unknown
}

fn normalized_backend(value: Option<&str>) -> Option<&str> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| {
            if value.eq_ignore_ascii_case("x11") {
                Some("x11")
            } else if value.eq_ignore_ascii_case("wayland") {
                Some("wayland")
            } else {
                None
            }
        })
}

pub(crate) fn choose_linux_window_backend(
    display_server: LinuxDisplayServer,
    x11_display: Option<&str>,
    kolboo_override: Option<&str>,
    gdk_backend: Option<&str>,
    winit_backend: Option<&str>,
) -> LinuxWindowBackendChoice {
    if let Some(value) = normalized_backend(kolboo_override) {
        return if value == "x11" {
            LinuxWindowBackendChoice::X11
        } else {
            LinuxWindowBackendChoice::Wayland
        };
    }

    // Propagate the older/documented Tao selector to GTK when present. Unlike
    // a desktop-provided GDK default, this variable is an application-level
    // runtime choice and should remain authoritative.
    if let Some(value) = normalized_backend(winit_backend) {
        return if value == "x11" {
            LinuxWindowBackendChoice::X11
        } else {
            LinuxWindowBackendChoice::Wayland
        };
    }

    // Standard xdg-shell toplevels cannot request global coordinates. Kolboo's
    // anchored overlay therefore uses XWayland when it is available. Desktop
    // environments commonly export GDK_BACKEND=wayland themselves, so only the
    // Kolboo-specific override above opts back into native Wayland.
    if display_server == LinuxDisplayServer::Wayland && non_empty(x11_display) {
        return LinuxWindowBackendChoice::X11;
    }

    // Outside the automatic Wayland compatibility case, respect an explicitly
    // configured GTK backend. Tao 0.35 uses GTK/GDK for its actual connection.
    if non_empty(gdk_backend) {
        return LinuxWindowBackendChoice::PreserveEnvironment;
    }

    LinuxWindowBackendChoice::PreserveEnvironment
}

#[cfg(target_os = "linux")]
pub(crate) fn configure_linux_window_backend() -> LinuxWindowBackendChoice {
    let display_server = current_linux_display_server();
    let x11_display = std::env::var("DISPLAY").ok();
    let kolboo_override = std::env::var("KOLBOO_LINUX_WINDOW_BACKEND").ok();
    let gdk_backend = std::env::var("GDK_BACKEND").ok();
    let winit_backend = std::env::var("WINIT_UNIX_BACKEND").ok();

    let choice = choose_linux_window_backend(
        display_server,
        x11_display.as_deref(),
        kolboo_override.as_deref(),
        gdk_backend.as_deref(),
        winit_backend.as_deref(),
    );

    match choice {
        LinuxWindowBackendChoice::X11 => {
            std::env::set_var("GDK_BACKEND", "x11");
            std::env::set_var("WINIT_UNIX_BACKEND", "x11");
        }
        LinuxWindowBackendChoice::Wayland => {
            std::env::set_var("GDK_BACKEND", "wayland");
            std::env::set_var("WINIT_UNIX_BACKEND", "wayland");
        }
        LinuxWindowBackendChoice::PreserveEnvironment => {}
    }

    choice
}

#[cfg(target_os = "linux")]
pub(crate) fn current_linux_display_server() -> LinuxDisplayServer {
    let session_type = std::env::var("XDG_SESSION_TYPE").ok();
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
    let x11_display = std::env::var("DISPLAY").ok();

    detect_linux_display_server(
        session_type.as_deref(),
        wayland_display.as_deref(),
        x11_display.as_deref(),
    )
}

pub(crate) fn automatic_text_injection_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        return current_linux_display_server() != LinuxDisplayServer::Wayland;
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

pub(crate) fn should_use_clipboard_fallback(automatic_insertion_requested: bool) -> bool {
    #[cfg(target_os = "linux")]
    {
        return should_use_clipboard_fallback_for(
            current_linux_display_server(),
            automatic_insertion_requested,
        );
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = automatic_insertion_requested;
        false
    }
}

fn should_use_clipboard_fallback_for(
    display_server: LinuxDisplayServer,
    automatic_insertion_requested: bool,
) -> bool {
    automatic_insertion_requested && display_server == LinuxDisplayServer::Wayland
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_session_type_wins_over_stale_display_variables() {
        assert_eq!(
            detect_linux_display_server(Some(" x11 "), Some("wayland-0"), Some(":0")),
            LinuxDisplayServer::X11
        );
        assert_eq!(
            detect_linux_display_server(Some("WAYLAND"), None, Some(":0")),
            LinuxDisplayServer::Wayland
        );
    }

    #[test]
    fn display_variables_provide_a_fallback_when_session_type_is_missing() {
        assert_eq!(
            detect_linux_display_server(None, Some("wayland-0"), Some(":0")),
            LinuxDisplayServer::Wayland
        );
        assert_eq!(
            detect_linux_display_server(None, None, Some(":0")),
            LinuxDisplayServer::X11
        );
    }

    #[test]
    fn empty_or_unrecognized_values_are_unknown() {
        assert_eq!(
            detect_linux_display_server(Some("tty"), Some(" "), Some("")),
            LinuxDisplayServer::Unknown
        );
    }

    #[test]
    fn wayland_falls_back_only_when_automatic_insertion_was_requested() {
        assert!(should_use_clipboard_fallback_for(
            LinuxDisplayServer::Wayland,
            true
        ));
        assert!(!should_use_clipboard_fallback_for(
            LinuxDisplayServer::Wayland,
            false
        ));
        assert!(!should_use_clipboard_fallback_for(
            LinuxDisplayServer::X11,
            true
        ));
    }

    #[test]
    fn wayland_session_uses_xwayland_when_available() {
        assert_eq!(
            choose_linux_window_backend(LinuxDisplayServer::Wayland, Some(":0"), None, None, None,),
            LinuxWindowBackendChoice::X11
        );
    }

    #[test]
    fn ambient_wayland_gdk_backend_does_not_disable_overlay_compatibility() {
        assert_eq!(
            choose_linux_window_backend(
                LinuxDisplayServer::Wayland,
                Some(":0"),
                None,
                Some("wayland"),
                None,
            ),
            LinuxWindowBackendChoice::X11
        );
    }

    #[test]
    fn explicit_gdk_backend_is_preserved_outside_wayland_compatibility() {
        assert_eq!(
            choose_linux_window_backend(
                LinuxDisplayServer::X11,
                Some(":0"),
                None,
                Some("x11"),
                None,
            ),
            LinuxWindowBackendChoice::PreserveEnvironment
        );
    }

    #[test]
    fn kolboo_override_has_highest_priority() {
        assert_eq!(
            choose_linux_window_backend(
                LinuxDisplayServer::Wayland,
                Some(":0"),
                Some("wayland"),
                Some("x11"),
                None,
            ),
            LinuxWindowBackendChoice::Wayland
        );
    }

    #[test]
    fn documented_winit_choice_is_propagated_to_gtk() {
        assert_eq!(
            choose_linux_window_backend(
                LinuxDisplayServer::Wayland,
                Some(":0"),
                None,
                None,
                Some("wayland"),
            ),
            LinuxWindowBackendChoice::Wayland
        );
    }

    #[test]
    fn native_wayland_is_preserved_without_xwayland() {
        assert_eq!(
            choose_linux_window_backend(LinuxDisplayServer::Wayland, None, None, None, None,),
            LinuxWindowBackendChoice::PreserveEnvironment
        );
    }
}
