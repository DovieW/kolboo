//! Exercise the actual ashpd/D-Bus adapter against an isolated local bus. No
//! real desktop permissions, accounts, input devices or application content.
use super::*;
use ashpd::zbus::{
    self,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::Mutex as StdMutex,
};

type Dict = HashMap<String, OwnedValue>;
fn value(value: impl Into<Value<'static>>) -> OwnedValue {
    OwnedValue::try_from(value.into()).unwrap()
}

#[derive(Default)]
struct Observed {
    app_ids: Vec<String>,
    peers: std::collections::HashSet<String>,
    selects: Vec<(u32, u32, Option<String>)>,
    events: Vec<(i32, u32)>,
    starts: usize,
    closes: usize,
    failure: &'static str,
    fail_events: Vec<usize>,
    version: u32,
    devices: u32,
    token: Option<String>,
    session_path: Option<OwnedObjectPath>,
}
type State = Arc<StdMutex<Observed>>;
struct Registry(State);
#[zbus::interface(name = "org.freedesktop.host.portal.Registry", crate = "ashpd::zbus")]
impl Registry {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }
    fn register(
        &self,
        app_id: &str,
        _options: Dict,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        let mut state = self.0.lock().unwrap();
        if state.failure == "registry" {
            return Err(zbus::fdo::Error::Failed("synthetic failure".into()));
        }
        if !state
            .peers
            .insert(header.sender().unwrap().as_str().to_string())
        {
            return Err(zbus::fdo::Error::Failed("peer already registered".into()));
        }
        state.app_ids.push(app_id.into());
        Ok(())
    }
}
struct MockSession(State);
#[zbus::interface(name = "org.freedesktop.portal.Session", crate = "ashpd::zbus")]
impl MockSession {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }
    fn close(&self) {
        self.0.lock().unwrap().closes += 1;
    }
}
struct MockRequest;
#[zbus::interface(name = "org.freedesktop.portal.Request", crate = "ashpd::zbus")]
impl MockRequest {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }
    fn close(&self) {}
}
struct Portal(State);

async fn reply(
    connection: &Connection,
    header: Header<'_>,
    options: Dict,
    code: u32,
    results: Dict,
) -> zbus::fdo::Result<OwnedObjectPath> {
    let sender = header.sender().unwrap().as_str();
    let token = <&str>::try_from(options.get("handle_token").unwrap()).unwrap();
    let path = OwnedObjectPath::try_from(format!(
        "/org/freedesktop/portal/desktop/request/{}/{}",
        sender.trim_start_matches(':').replace('.', "_"),
        token
    ))
    .unwrap();
    connection
        .object_server()
        .at(path.clone(), MockRequest)
        .await
        .unwrap();
    connection
        .emit_signal(
            Some(sender),
            path.clone(),
            "org.freedesktop.portal.Request",
            "Response",
            &(code, results),
        )
        .await
        .unwrap();
    Ok(path)
}

#[zbus::interface(name = "org.freedesktop.portal.RemoteDesktop", crate = "ashpd::zbus")]
impl Portal {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        self.0.lock().unwrap().version
    }
    #[zbus(property)]
    fn available_device_types(&self) -> zbus::fdo::Result<u32> {
        let state = self.0.lock().unwrap();
        if state.failure == "capabilities" {
            return Err(zbus::fdo::Error::Failed("synthetic failure".into()));
        }
        Ok(state.devices)
    }
    async fn create_session(
        &self,
        options: Dict,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        if self.0.lock().unwrap().failure == "create" {
            return Err(zbus::fdo::Error::Failed("synthetic failure".into()));
        }
        let sender = header.sender().unwrap().as_str();
        let token = <&str>::try_from(options.get("session_handle_token").unwrap()).unwrap();
        let path = OwnedObjectPath::try_from(format!(
            "/org/freedesktop/portal/desktop/session/{}/{}",
            sender.trim_start_matches(':').replace('.', "_"),
            token
        ))
        .unwrap();
        connection
            .object_server()
            .at(path.clone(), MockSession(self.0.clone()))
            .await
            .unwrap();
        self.0.lock().unwrap().session_path = Some(path.clone());
        reply(
            connection,
            header,
            options,
            0,
            HashMap::from([("session_handle".into(), value(path.to_string()))]),
        )
        .await
    }
    async fn select_devices(
        &self,
        _session: OwnedObjectPath,
        options: Dict,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let (failure, code) = {
            let mut state = self.0.lock().unwrap();
            state.selects.push((
                u32::try_from(options.get("types").unwrap()).unwrap(),
                u32::try_from(options.get("persist_mode").unwrap()).unwrap(),
                options
                    .get("restore_token")
                    .map(|v| <&str>::try_from(v).unwrap().to_string()),
            ));
            (
                state.failure,
                if state.failure == "select-denied" {
                    1
                } else {
                    0
                },
            )
        };
        if failure == "select" {
            return Err(zbus::fdo::Error::Failed("synthetic failure".into()));
        }
        reply(connection, header, options, code, Dict::new()).await
    }
    async fn start(
        &self,
        _session: OwnedObjectPath,
        _parent: &str,
        options: Dict,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let (failure, devices, token) = {
            let mut state = self.0.lock().unwrap();
            state.starts += 1;
            (state.failure, state.devices, state.token.clone())
        };
        if failure == "start" {
            return Err(zbus::fdo::Error::Failed("synthetic failure".into()));
        }
        let mut results = HashMap::from([(
            "devices".into(),
            value(if failure == "no-keyboard" {
                2u32
            } else {
                devices
            }),
        )]);
        if let Some(token) = token {
            results.insert("restore_token".into(), value(token));
        }
        reply(
            connection,
            header,
            options,
            if failure == "denied" { 1 } else { 0 },
            results,
        )
        .await
    }
    fn notify_keyboard_keysym(
        &self,
        _session: OwnedObjectPath,
        _options: Dict,
        key: i32,
        state: u32,
    ) -> zbus::fdo::Result<()> {
        let mut observed = self.0.lock().unwrap();
        observed.events.push((key, state));
        if observed.fail_events.contains(&observed.events.len()) {
            return Err(zbus::fdo::Error::Failed("synthetic failure".into()));
        }
        Ok(())
    }
}

#[derive(Default)]
struct Store {
    token: StdMutex<Option<String>>,
    saves: StdMutex<Vec<Option<String>>>,
    fail: bool,
}
impl RestoreTokenStore for Store {
    fn load(&self) -> futures_util::future::BoxFuture<'_, Option<String>> {
        Box::pin(async { self.token.lock().unwrap().clone() })
    }
    fn save<'a>(
        &'a self,
        token: Option<&'a str>,
    ) -> futures_util::future::BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            if self.fail {
                return Err("synthetic storage failure".into());
            }
            let token = token.map(str::to_string);
            self.saves.lock().unwrap().push(token.clone());
            *self.token.lock().unwrap() = token;
            Ok(())
        })
    }
}

struct Bus {
    child: Child,
    address: String,
    client: Connection,
    server: Connection,
    observed: State,
}
impl Bus {
    async fn new() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("isolated D-Bus fixture requires dbus-daemon");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let observed = Arc::new(StdMutex::new(Observed {
            version: 2,
            devices: 1,
            token: Some("synthetic-rotated-token".into()),
            ..Default::default()
        }));
        let server = zbus::connection::Builder::address(address.trim())
            .unwrap()
            .name("org.freedesktop.portal.Desktop")
            .unwrap()
            .serve_at("/org/freedesktop/portal/desktop", Portal(observed.clone()))
            .unwrap()
            .serve_at(
                "/org/freedesktop/portal/desktop",
                Registry(observed.clone()),
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        let client = zbus::connection::Builder::address(address.trim())
            .unwrap()
            .build()
            .await
            .unwrap();
        Self {
            child,
            address,
            client,
            server,
            observed,
        }
    }
    async fn paste(
        &self,
        controller: &mut InputController,
        store: &Store,
        shortcut: PasteShortcut,
        enter: bool,
    ) -> Result<(), String> {
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            controller.paste(
                async {
                    zbus::connection::Builder::address(self.address.trim())
                        .unwrap()
                        .build()
                        .await
                        .map_err(|_| "synthetic bus connection failure".into())
                },
                "com.kolboo.app",
                store,
                (InputShortcut::Paste(shortcut), enter),
                || Ok(()),
                |_| Box::pin(async {}),
            ),
        )
        .await
        .expect("portal fixture hung")
    }
}
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test]
async fn clipboard_is_prepared_after_approval_and_copy_media_share_the_same_grant() {
    let bus = Bus::new().await;
    let store = Store::default();
    let mut controller = InputController::default();
    assert!(controller
        .paste(
            async { panic!("media must not connect or prompt") },
            "com.kolboo.app",
            &store,
            (InputShortcut::MediaPlayPause, false),
            || panic!("no approval"),
            |_| Box::pin(async {})
        )
        .await
        .is_err());
    assert!(bus.observed.lock().unwrap().app_ids.is_empty());
    for (shortcut, expected) in [
        (
            InputShortcut::Copy,
            vec![(0xffe3, 1), (0x63, 1), (0x63, 0), (0xffe3, 0)],
        ),
        (
            InputShortcut::MediaPlayPause,
            vec![(0x1008ff14, 1), (0x1008ff14, 0)],
        ),
    ] {
        bus.observed.lock().unwrap().events.clear();
        controller
            .paste(
                async { Ok(bus.client.clone()) },
                "com.kolboo.app",
                &store,
                (shortcut, false),
                || {
                    let state = bus.observed.lock().unwrap();
                    assert_eq!(state.starts, 1);
                    assert!(state.events.is_empty());
                    Ok(())
                },
                |_| Box::pin(async {}),
            )
            .await
            .unwrap();
        assert_eq!(bus.observed.lock().unwrap().events, expected);
    }
    bus.observed.lock().unwrap().events.clear();
    assert_eq!(
        controller
            .paste(
                async { Ok(bus.client.clone()) },
                "com.kolboo.app",
                &store,
                (InputShortcut::Paste(PasteShortcut::System), false),
                || Err("synthetic clipboard failure".into()),
                |_| Box::pin(async {})
            )
            .await,
        Err("synthetic clipboard failure".into())
    );
    assert!(bus.observed.lock().unwrap().events.is_empty());
    assert_eq!(bus.observed.lock().unwrap().starts, 1);
    // A clipboard error doesn't destroy valid OS approval.
    bus.paste(&mut controller, &store, PasteShortcut::System, false)
        .await
        .unwrap();
    assert_eq!(bus.observed.lock().unwrap().starts, 1);
}

#[tokio::test]
async fn keyboard_only_grant_is_identified_rotated_and_reused_for_all_paste_chords() {
    let bus = Bus::new().await;
    let store = Store::default();
    *store.token.lock().unwrap() = Some("synthetic-old-token".into());
    let mut controller = InputController::default();
    // Expected keysyms are independent of the adapter's mapping: a wrong key
    // must fail this protocol test rather than redefine its own expectation.
    for (shortcut, modifiers, key) in [
        (PasteShortcut::System, &[0xffe3][..], 0x76),
        (PasteShortcut::CtrlV, &[0xffe3][..], 0x76),
        (PasteShortcut::CtrlShiftV, &[0xffe3, 0xffe1][..], 0x76),
        (PasteShortcut::ShiftInsert, &[0xffe1][..], 0xff63),
        (PasteShortcut::CmdV, &[0xffeb][..], 0x76),
    ] {
        let before = bus.observed.lock().unwrap().events.len();
        bus.paste(&mut controller, &store, shortcut, true)
            .await
            .unwrap();
        let mut expected: Vec<_> = modifiers.iter().map(|key| (*key, 1)).collect();
        expected.extend([(key, 1), (key, 0)]);
        expected.extend(modifiers.iter().rev().map(|key| (*key, 0)));
        expected.extend([(0xff0d, 1), (0xff0d, 0)]);
        assert_eq!(&bus.observed.lock().unwrap().events[before..], expected);
    }
    let observed = bus.observed.lock().unwrap();
    assert_eq!(observed.app_ids, ["com.kolboo.app"]);
    assert_eq!(
        observed.selects,
        [(1, 2, Some("synthetic-old-token".into()))]
    );
    assert_eq!(observed.starts, 1);
    assert_eq!(
        *store.token.lock().unwrap(),
        Some("synthetic-rotated-token".into())
    );
}

#[tokio::test]
async fn denied_or_unsupported_portals_never_repeat_the_prompt_or_send_keys() {
    for failure in [
        "registry",
        "capabilities",
        "create",
        "select",
        "select-denied",
        "start",
        "denied",
        "no-keyboard",
        "old",
        "no-device",
    ] {
        let bus = Bus::new().await;
        {
            let mut state = bus.observed.lock().unwrap();
            state.failure = failure;
            if failure == "old" {
                state.version = 1;
            }
            if failure == "no-device" {
                state.devices = 2;
            }
        }
        let mut controller = InputController::default();
        let store = Store::default();
        assert!(
            bus.paste(&mut controller, &store, PasteShortcut::System, false)
                .await
                .is_err(),
            "{failure}"
        );
        let starts = bus.observed.lock().unwrap().starts;
        assert!(bus
            .paste(&mut controller, &store, PasteShortcut::System, false)
            .await
            .is_err());
        let state = bus.observed.lock().unwrap();
        assert_eq!(state.starts, starts);
        assert!(state.events.is_empty());
        assert!(store.saves.lock().unwrap().is_empty());
        if matches!(
            failure,
            "select" | "select-denied" | "start" | "denied" | "no-keyboard"
        ) {
            assert_eq!(state.closes, 1);
        }
    }
    let mut controller = InputController::default();
    assert!(controller
        .paste(
            async { Err("synthetic bus failure".into()) },
            "com.kolboo.app",
            &Store::default(),
            (InputShortcut::Paste(PasteShortcut::System), false),
            || Ok(()),
            |_| Box::pin(async {})
        )
        .await
        .is_err());
}

#[tokio::test]
async fn partial_key_failures_clean_up_without_replaying_the_paste() {
    for failures in (1..=8).map(|event| vec![event]).chain([vec![3, 4]]) {
        let fail_at = failures[0];
        let bus = Bus::new().await;
        let mut controller = InputController::default();
        let store = Store::default();
        bus.observed.lock().unwrap().fail_events = failures;
        assert!(bus
            .paste(&mut controller, &store, PasteShortcut::CtrlShiftV, true)
            .await
            .is_err());
        {
            let state = bus.observed.lock().unwrap();
            assert_eq!(state.closes, 1);
            assert_eq!(state.starts, 1);
            assert_eq!(
                state
                    .events
                    .iter()
                    .filter(|event| **event == (0x76, 1))
                    .count(),
                usize::from(fail_at > 2)
            );
            assert!(state.events.contains(&(0xffe3, 0)));
            assert!(state.events.contains(&(0xffe1, 0)));
            if fail_at >= 3 {
                assert!(
                    state.events.contains(&(0x76, 0)),
                    "primary key release after failed delivery"
                );
            }
            if fail_at >= 7 {
                assert!(
                    state.events.contains(&(0xff0d, 0)),
                    "Enter release after failed delivery"
                );
            }
        }
        bus.observed.lock().unwrap().fail_events.clear();
        bus.paste(&mut controller, &store, PasteShortcut::System, false)
            .await
            .unwrap();
        assert_eq!(
            bus.observed.lock().unwrap().selects[1].2,
            Some("synthetic-rotated-token".into())
        );
    }
}

#[test]
fn packaged_linux_desktop_identity_matches_portal_registration_without_duplicate_menu_items() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../../../tauri.conf.json")).unwrap();
    let identifier = config["identifier"].as_str().unwrap();
    assert_eq!(identifier, "com.kolboo.app");
    for package in ["deb", "rpm", "appimage"] {
        assert_eq!(
            config["bundle"]["linux"][package]["files"]
                [format!("/usr/share/applications/{identifier}.desktop")],
            "linux/com.kolboo.app.desktop"
        );
    }
    let entry = include_str!("../../../linux/com.kolboo.app.desktop");
    for field in [
        "Name=Kolboo",
        "Exec=kolboo",
        "NoDisplay=true",
        "Type=Application",
    ] {
        assert!(entry.lines().any(|line| line == field));
    }
}

#[tokio::test]
async fn missing_persistence_clears_obsolete_tokens_and_storage_failure_keeps_live_session() {
    for fail in [false, true] {
        let bus = Bus::new().await;
        bus.observed.lock().unwrap().token = None;
        let store = Store {
            fail,
            ..Default::default()
        };
        *store.token.lock().unwrap() = Some("synthetic-old-token".into());
        let mut controller = InputController::default();
        bus.paste(&mut controller, &store, PasteShortcut::System, false)
            .await
            .unwrap();
        bus.paste(&mut controller, &store, PasteShortcut::System, false)
            .await
            .unwrap();
        assert_eq!(bus.observed.lock().unwrap().starts, 1);
        assert_eq!(store.saves.lock().unwrap().len(), usize::from(!fail));
        if !fail {
            assert!(store.token.lock().unwrap().is_none());
        }
    }
}

#[tokio::test]
async fn closed_sessions_restore_rotated_token_and_invalid_identity_fails_before_requests() {
    let bus = Bus::new().await;
    let store = Store::default();
    let mut controller = InputController::default();
    bus.paste(&mut controller, &store, PasteShortcut::System, false)
        .await
        .unwrap();
    let path = bus.observed.lock().unwrap().session_path.clone().unwrap();
    bus.server
        .emit_signal(
            None::<&str>,
            path,
            "org.freedesktop.portal.Session",
            "Closed",
            &(Dict::new(),),
        )
        .await
        .unwrap();
    // Synchronize with the real closed listener, not a timing sleep.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !controller
            .active
            .as_ref()
            .unwrap()
            .closed
            .load(Ordering::Acquire)
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    bus.paste(&mut controller, &store, PasteShortcut::System, false)
        .await
        .unwrap();
    assert_eq!(bus.observed.lock().unwrap().starts, 2);
    assert_eq!(
        bus.observed.lock().unwrap().selects[1].2,
        Some("synthetic-rotated-token".into())
    );
    let mut controller = InputController::default();
    assert!(controller
        .paste(
            async { Ok(bus.client.clone()) },
            "invalid",
            &store,
            (InputShortcut::Paste(PasteShortcut::System), false),
            || Ok(()),
            |_| Box::pin(async {})
        )
        .await
        .unwrap_err()
        .contains("identity"));
}
