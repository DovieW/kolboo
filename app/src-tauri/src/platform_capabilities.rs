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
}
