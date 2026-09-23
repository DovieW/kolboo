//! Linux window identity for per-program profiles. Queries run only when the
//! user opens the picker or a recording asks for its foreground program.
//! No window titles or focus history are retained by this module.

use dbus::blocking::{Connection, SyncConnection};
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct OpenWindowInfo {
    pub title: String,
    pub process_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    Kwin,
    Gnome,
    Sway,
    Hyprland,
    X11,
    Unsupported,
}

fn backend(session: &str, desktop: &str, sway_socket: bool, hyprland_socket: bool) -> Backend {
    let session = session.to_ascii_lowercase();
    let desktop = desktop.to_ascii_lowercase();
    if session == "x11" {
        Backend::X11
    } else if hyprland_socket || desktop.contains("hyprland") {
        Backend::Hyprland
    } else if sway_socket || desktop.contains("sway") {
        Backend::Sway
    } else if desktop.contains("kde") || desktop.contains("plasma") {
        Backend::Kwin
    } else if desktop.contains("gnome") {
        Backend::Gnome
    } else {
        Backend::Unsupported
    }
}

fn current_backend() -> Backend {
    backend(
        &std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),
        &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        std::env::var_os("SWAYSOCK").is_some(),
        std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some(),
    )
}

#[derive(Debug, Clone, Deserialize)]
struct RawWindow {
    #[serde(default)]
    pid: u32,
    #[serde(default, alias = "name")]
    title: String,
    #[serde(default, alias = "focus", alias = "focused")]
    active: bool,
}

fn process_path(pid: u32) -> Option<String> {
    if pid == 0 || pid == std::process::id() {
        return None;
    }
    std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()?
        .to_str()
        .map(str::to_owned)
}

fn visible_windows(raw: Vec<RawWindow>, include_titles: bool) -> Vec<OpenWindowInfo> {
    raw.into_iter()
        .filter_map(|window| {
            let process_path = process_path(window.pid)?;
            Some(OpenWindowInfo {
                title: if include_titles {
                    window.title
                } else {
                    String::new()
                },
                process_path,
            })
        })
        .collect()
}

fn command_output(program: &str, args: &[&str]) -> Result<String, String> {
    // Redirect to a bounded temporary file so a large Sway tree cannot fill a
    // pipe and deadlock while we wait for the compositor helper to exit.
    let mut output_file = tempfile::tempfile()
        .map_err(|_| format!("Could not prepare the {program} window query."))?;
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::from(output_file.try_clone().map_err(|_| {
            format!("Could not prepare the {program} window query.")
        })?))
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| format!("{program} is unavailable for this desktop session."))?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) => return Err(format!("{program} could not query this desktop session.")),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{program} timed out querying this desktop session."
                ));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program} could not query this desktop session."));
            }
        }
    }
    if output_file
        .metadata()
        .map(|meta| meta.len())
        .unwrap_or(u64::MAX)
        > 8 * 1024 * 1024
    {
        return Err(format!("{program} returned too much window data."));
    }
    output_file
        .seek(SeekFrom::Start(0))
        .map_err(|_| format!("{program} returned unreadable window data."))?;
    let mut output = String::new();
    output_file
        .read_to_string(&mut output)
        .map_err(|_| format!("{program} returned invalid text."))?;
    Ok(output)
}

fn json_windows(payload: &str) -> Result<Vec<RawWindow>, String> {
    serde_json::from_str(payload).map_err(|_| "The desktop returned invalid window data.".into())
}

fn gnome_windows() -> Result<Vec<RawWindow>, String> {
    let connection = Connection::new_session()
        .map_err(|_| "Could not connect to the desktop session bus.".to_string())?;
    let proxy = connection.with_proxy(
        "org.gnome.Shell",
        "/org/gnome/Shell/Extensions/WindowsExt",
        Duration::from_secs(2),
    );
    let (payload,): (String,) = proxy
        .method_call("org.gnome.Shell.Extensions.WindowsExt", "List", ())
        .map_err(|_| {
            "GNOME Wayland needs the opt-in Window Calls Extended extension for program profiles."
                .to_string()
        })?;
    json_windows(&payload)
}

fn gnome_active_pid() -> Option<u32> {
    let connection = Connection::new_session().ok()?;
    let proxy = connection.with_proxy(
        "org.gnome.Shell",
        "/org/gnome/Shell/Extensions/WindowsExt",
        Duration::from_secs(2),
    );
    let (pid,): (String,) = proxy
        .method_call("org.gnome.Shell.Extensions.WindowsExt", "FocusPID", ())
        .ok()?;
    pid.parse().ok()
}

fn sway_collect(node: &Value, windows: &mut Vec<RawWindow>) {
    if let Some(pid) = node
        .get("pid")
        .and_then(Value::as_u64)
        .and_then(|pid| u32::try_from(pid).ok())
    {
        windows.push(RawWindow {
            pid,
            title: node
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            active: node.get("focused").and_then(Value::as_bool) == Some(true),
        });
    }
    for key in ["nodes", "floating_nodes"] {
        if let Some(children) = node.get(key).and_then(Value::as_array) {
            for child in children {
                sway_collect(child, windows);
            }
        }
    }
}

fn sway_windows(payload: &str) -> Result<Vec<RawWindow>, String> {
    let tree: Value = serde_json::from_str(payload)
        .map_err(|_| "Sway returned invalid window data.".to_string())?;
    let mut windows = Vec::new();
    sway_collect(&tree, &mut windows);
    Ok(windows)
}

fn hypr_windows(payload: &str, active_pid: Option<u32>) -> Result<Vec<RawWindow>, String> {
    let clients: Vec<Value> = serde_json::from_str(payload)
        .map_err(|_| "Hyprland returned invalid window data.".to_string())?;
    Ok(clients
        .iter()
        .filter_map(|client| {
            let pid = u32::try_from(client.get("pid")?.as_u64()?).ok()?;
            Some(RawWindow {
                pid,
                title: client
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                active: active_pid == Some(pid),
            })
        })
        .collect())
}

fn x11_window_ids(payload: &str) -> Vec<&str> {
    payload
        .split_once('#')
        .or_else(|| payload.split_once('='))
        .map(|(_, ids)| {
            ids.split(',')
                .map(str::trim)
                .filter(|id| id.starts_with("0x") && id.len() <= 18)
                .collect()
        })
        .unwrap_or_default()
}

fn x11_window_properties(payload: &str, active: bool) -> Option<RawWindow> {
    let pid = payload.lines().find_map(|line| {
        line.starts_with("_NET_WM_PID")
            .then(|| line.split_once('=')?.1.trim().parse::<u32>().ok())
            .flatten()
    })?;
    let title = payload
        .lines()
        .find(|line| line.starts_with("_NET_WM_NAME") || line.starts_with("WM_NAME"))
        .and_then(|line| line.split_once('=').map(|(_, value)| value.trim()))
        .unwrap_or_default()
        .trim_matches('"')
        .to_string();
    Some(RawWindow { pid, title, active })
}

fn x11_windows() -> Result<Vec<RawWindow>, String> {
    let ids = command_output("xprop", &["-root", "_NET_CLIENT_LIST"])?;
    let active = command_output("xprop", &["-root", "_NET_ACTIVE_WINDOW"])?;
    let active_id = x11_window_ids(&active).into_iter().next();
    Ok(x11_window_ids(&ids)
        .into_iter()
        .take(256)
        .filter_map(|id| {
            let properties = command_output(
                "xprop",
                &["-id", id, "_NET_WM_PID", "_NET_WM_NAME", "WM_NAME"],
            )
            .ok()?;
            x11_window_properties(&properties, active_id == Some(id))
        })
        .collect())
}

#[derive(Deserialize)]
struct KwinSnapshot {
    windows: Vec<RawWindow>,
    active_pid: Option<u32>,
}

fn kwin_script(bus_name: &str, token: &str, include_windows: bool) -> String {
    let bus_name = serde_json::to_string(bus_name).unwrap_or_default();
    let token = serde_json::to_string(token).unwrap_or_default();
    let windows = if include_windows {
        "workspace.windowList().filter(w => w.pid > 0 && !w.deleted && !w.popupWindow).map(w => ({pid: w.pid, title: w.caption}))"
    } else {
        "[]"
    };
    format!(
        r#"try {{
  const windows = {windows};
  const active_pid = workspace.activeWindow ? workspace.activeWindow.pid : null;
  callDBus({bus_name}, "/", "", "result", {token}, JSON.stringify({{windows, active_pid}}));
}} catch (error) {{
  callDBus({bus_name}, "/", "", "error", {token}, "Window query failed");
}}
"#
    )
}

fn kwin_snapshot(include_windows: bool) -> Result<KwinSnapshot, String> {
    let receiver = SyncConnection::new_session()
        .map_err(|_| "Could not connect to the desktop session bus.".to_string())?;
    let token = uuid::Uuid::new_v4().to_string();
    let script = kwin_script(&receiver.unique_name().to_string(), &token, include_windows);
    let (sender, response) = mpsc::channel();
    receiver.start_receive(
        MatchRule::new_method_call(),
        Box::new(move |message, _| {
            if let Ok((received_token, payload)) = message.read2::<String, String>() {
                let member = message.member().map(|name| name.to_string());
                if received_token == token {
                    if let Some(member) = member.filter(|name| name == "result" || name == "error")
                    {
                        let _ = sender.send((member, payload));
                    }
                }
            }
            true
        }),
    );

    let mut script_file = tempfile::NamedTempFile::new()
        .map_err(|_| "Could not prepare the KWin window query.".to_string())?;
    script_file
        .write_all(script.as_bytes())
        .map_err(|_| "Could not prepare the KWin window query.".to_string())?;
    let connection = Connection::new_session()
        .map_err(|_| "Could not connect to the desktop session bus.".to_string())?;
    let scripting = connection.with_proxy("org.kde.KWin", "/Scripting", Duration::from_secs(3));
    let script_name = format!("kolboo-window-{}", uuid::Uuid::new_v4());
    let (script_id,): (i32,) = scripting
        .method_call(
            "org.kde.kwin.Scripting",
            "loadScript",
            (
                script_file.path().to_string_lossy().as_ref(),
                script_name.as_str(),
            ),
        )
        .map_err(|_| "KWin window access is unavailable in this session.".to_string())?;
    if script_id < 0 {
        return Err("KWin refused the window query.".into());
    }
    let script_proxy = connection.with_proxy(
        "org.kde.KWin",
        format!("/Scripting/Script{script_id}"),
        Duration::from_secs(3),
    );
    let run_result: Result<(), dbus::Error> =
        script_proxy.method_call("org.kde.kwin.Script", "run", ());
    let started = Instant::now();
    let mut result = Err("KWin did not respond to the window query.".to_string());
    if run_result.is_ok() {
        while started.elapsed() < Duration::from_secs(3) {
            if receiver.process(Duration::from_millis(50)).is_err() {
                break;
            }
            if let Ok((kind, payload)) = response.try_recv() {
                result = if kind == "result" {
                    serde_json::from_str(&payload)
                        .map_err(|_| "KWin returned invalid window data.".to_string())
                } else {
                    Err("KWin could not query open windows.".to_string())
                };
                break;
            }
        }
    }
    let _: Result<(), _> = script_proxy.method_call("org.kde.kwin.Script", "stop", ());
    let _: Result<(bool,), _> =
        scripting.method_call("org.kde.kwin.Scripting", "unloadScript", (script_name,));
    result
}

fn query_windows() -> Result<Vec<RawWindow>, String> {
    match current_backend() {
        Backend::Kwin => {
            let snapshot = kwin_snapshot(true)?;
            Ok(snapshot
                .windows
                .into_iter()
                .map(|mut window| {
                    window.active = Some(window.pid) == snapshot.active_pid;
                    window
                })
                .collect())
        }
        Backend::Gnome => gnome_windows(),
        Backend::Sway => sway_windows(&command_output("swaymsg", &["-r", "-t", "get_tree"])?),
        Backend::Hyprland => {
            let clients = command_output("hyprctl", &["-j", "clients"])?;
            let active = command_output("hyprctl", &["-j", "activewindow"])?;
            let active_pid = serde_json::from_str::<Value>(&active)
                .ok()
                .and_then(|value| value.get("pid")?.as_u64())
                .and_then(|pid| u32::try_from(pid).ok());
            hypr_windows(&clients, active_pid)
        }
        Backend::X11 => x11_windows(),
        Backend::Unsupported => Err(
            "This Linux desktop does not expose open programs to Kolboo. Add an executable path manually."
                .into(),
        ),
    }
}

pub fn list_open_windows(include_titles: bool) -> Result<Vec<OpenWindowInfo>, String> {
    Ok(visible_windows(query_windows()?, include_titles))
}

pub fn get_foreground_process_path() -> Option<String> {
    let pid = match current_backend() {
        Backend::Kwin => kwin_snapshot(false).ok()?.active_pid,
        Backend::Gnome => gnome_active_pid(),
        Backend::Sway => sway_windows(&command_output("swaymsg", &["-r", "-t", "get_tree"]).ok()?)
            .ok()?
            .into_iter()
            .find(|window| window.active)
            .map(|window| window.pid),
        Backend::Hyprland => {
            serde_json::from_str::<Value>(&command_output("hyprctl", &["-j", "activewindow"]).ok()?)
                .ok()?
                .get("pid")
                .and_then(Value::as_u64)
                .and_then(|pid| u32::try_from(pid).ok())
        }
        Backend::X11 => {
            let active = command_output("xprop", &["-root", "_NET_ACTIVE_WINDOW"]).ok()?;
            let id = *x11_window_ids(&active).first()?;
            let properties = command_output("xprop", &["-id", id, "_NET_WM_PID"]).ok()?;
            x11_window_properties(&properties, true).map(|window| window.pid)
        }
        Backend::Unsupported => None,
    }?;
    process_path(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
