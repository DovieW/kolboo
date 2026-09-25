use super::*;

#[test]
fn window_queries_exclude_self_and_keep_titles_opt_in() {
    // An owned child keeps /proc identity deterministic without probing the
    // user's applications. Wait for its shell to be ready before inspecting
    // /proc; the spawned process may not have replaced its image yet.
    let mut child = Command::new("sh")
        .args(["-c", "printf x; while IFS= read -r line; do :; done"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    let mut ready = [0_u8; 1];
    child
        .stdout
        .as_mut()
        .unwrap()
        .read_exact(&mut ready)
        .unwrap();
    assert_eq!(ready, [b'x']);
    let expected_path = std::fs::read_link(format!("/proc/{pid}/exe"))
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let windows = || {
        vec![
            RawWindow {
                pid,
                title: "synthetic title".into(),
                active: true,
            },
            RawWindow {
                pid: std::process::id(),
                title: "self".into(),
                active: false,
            },
            RawWindow {
                pid: 0,
                title: "invalid".into(),
                active: false,
            },
            RawWindow {
                pid: u32::MAX,
                title: "missing".into(),
                active: false,
            },
        ]
    };
    let private = visible_windows(windows(), false);
    let titled = visible_windows(windows(), true);
    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
    assert_eq!(private.len(), 1);
    assert!(private[0].title.is_empty());
    assert_eq!(private[0].process_path, expected_path);
    assert_eq!(titled.len(), 1);
    assert_eq!(titled[0].title, "synthetic title");
    assert_eq!(private[0].process_path, titled[0].process_path);
}

#[test]
fn malformed_desktop_responses_are_content_free_errors_or_ignored_records() {
    assert_eq!(
        json_windows("private invalid payload").unwrap_err(),
        "The desktop returned invalid window data."
    );
    assert_eq!(
        sway_windows("private invalid payload").unwrap_err(),
        "Sway returned invalid window data."
    );
    assert_eq!(
        hypr_windows("private invalid payload", None).unwrap_err(),
        "Hyprland returned invalid window data."
    );
    assert!(
        sway_windows(r#"{"nodes":[{"pid":-1},{"pid":4294967296},{"name":"no pid"}]}"#)
            .unwrap()
            .is_empty()
    );
    let windows = hypr_windows(r#"[{"pid":123},{"pid":-1},{"pid":4294967296},{}]"#, None).unwrap();
    assert_eq!(windows.len(), 1);
    assert!(windows[0].title.is_empty());
    assert!(!windows[0].active);
    assert!(x11_window_ids("invalid").is_empty());
    assert!(x11_window_ids("ids = junk, 0x111111111111111111111").is_empty());
    assert!(x11_window_properties("_NET_WM_PID = junk", false).is_none());
    assert!(x11_window_properties("_NET_WM_PID = 123", false)
        .unwrap()
        .title
        .is_empty());
}

#[test]
fn desktop_helper_output_is_bounded_and_must_be_utf8() {
    assert_eq!(
        command_output("head", &["-c", "8388609", "/dev/zero"]).unwrap_err(),
        "head returned too much window data."
    );
    assert_eq!(
        command_output("printf", &["\\377"]).unwrap_err(),
        "printf returned invalid text."
    );
}

#[test]
fn selects_desktop_backend_without_using_xwayland_as_a_false_fallback() {
    assert_eq!(backend("x11", "GNOME", false, false), Backend::X11);
    assert_eq!(backend("wayland", "KDE", false, false), Backend::Kwin);
    assert_eq!(backend("wayland", "GNOME", false, false), Backend::Gnome);
    assert_eq!(backend("wayland", "", true, false), Backend::Sway);
    assert_eq!(backend("wayland", "", false, true), Backend::Hyprland);
    assert_eq!(
        backend("wayland", "COSMIC", false, false),
        Backend::Unsupported
    );
}

#[test]
fn parses_sway_nested_and_floating_windows() {
    let windows = sway_windows(
        r#"{"nodes":[{"pid":123,"name":"Editor","focused":true,"nodes":[]}],"floating_nodes":[{"pid":456,"name":"Chat","focused":false}]}"#,
    )
    .unwrap();
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0].pid, 123);
    assert!(windows[0].active);
    assert_eq!(windows[1].title, "Chat");
}

#[test]
fn parses_gnome_extension_window_shape() {
    let windows =
        json_windows(r#"[{"class":"Editor","pid":123,"focus":true,"title":"Notes"}]"#).unwrap();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].pid, 123);
    assert_eq!(windows[0].title, "Notes");
    assert!(windows[0].active);
}

#[test]
fn parses_hyprland_and_x11_window_data() {
    let windows = hypr_windows(
        r#"[{"pid":123,"title":"Editor"},{"pid":456,"title":"Chat"}]"#,
        Some(456),
    )
    .unwrap();
    assert!(!windows[0].active);
    assert!(windows[1].active);
    assert_eq!(
        x11_window_ids("_NET_CLIENT_LIST = 0x1, 0x2"),
        ["0x1", "0x2"]
    );
    assert_eq!(
        x11_window_ids("_NET_CLIENT_LIST(WINDOW): window id # 0x1, 0x2"),
        ["0x1", "0x2"]
    );
    let window =
        x11_window_properties("_NET_WM_PID = 123\n_NET_WM_NAME = \"Editor\"", true).unwrap();
    assert_eq!(window.pid, 123);
    assert_eq!(window.title, "Editor");
    assert!(window.active);
}

#[test]
fn command_query_reports_success_and_unavailable_helpers() {
    assert_eq!(
        command_output("printf", &["window-data"]).unwrap(),
        "window-data"
    );
    assert!(command_output("false", &[]).is_err());
    assert!(command_output("kolboo-nonexistent-window-helper", &[]).is_err());
}

#[test]
fn kwin_query_is_read_only_and_uses_per_request_address() {
    let script = kwin_script(":1.42", "token", true);
    assert!(script.contains("workspace.windowList()"));
    assert!(script.contains("workspace.activeWindow"));
    assert!(script.contains("callDBus(\":1.42\""));
    assert!(!script.contains("activateWindow"));
    assert!(!kwin_script(":1.42", "token", false).contains("workspace.windowList()"));
}

#[test]
#[ignore = "requires a live KDE Plasma desktop session"]
fn kwin_manual_smoke() {
    assert!(!kwin_snapshot(true).unwrap().windows.is_empty());
    let programs = list_open_windows(false).unwrap();
    assert!(!programs.is_empty());
    assert!(programs.iter().all(|program| program.title.is_empty()));
    assert!(programs
        .iter()
        .all(|program| !program.process_path.is_empty()));
}
