//! Keyboard-only, app-identified Wayland input. Never fall back to XWayland
//! simulation: it cannot restore Kolboo's grant and can repeatedly prompt.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};

use ashpd::{
    desktop::{
        remote_desktop::{DeviceType, KeyState, RemoteDesktop, SelectDevicesOptions},
        PersistMode, Session,
    },
    zbus::Connection,
    AppID,
};
use futures_util::StreamExt;
use tokio::sync::Mutex;

use super::key_inject::PasteShortcut;

#[derive(Clone, Copy)]
pub(crate) enum InputShortcut {
    Paste(PasteShortcut),
    Copy,
    MediaPlayPause,
}

pub(crate) trait RestoreTokenStore: Send + Sync {
    fn load(&self) -> futures_util::future::BoxFuture<'_, Option<String>>;
    fn save<'a>(
        &'a self,
        token: Option<&'a str>,
    ) -> futures_util::future::BoxFuture<'a, Result<(), String>>;
}

struct ActiveSession {
    portal: RemoteDesktop,
    session: Arc<Session<RemoteDesktop>>,
    closed: Arc<AtomicBool>,
    closed_task: tokio::task::JoinHandle<()>,
}

impl Drop for ActiveSession {
    fn drop(&mut self) {
        self.closed_task.abort();
    }
}

#[derive(Default)]
pub(crate) struct InputController {
    active: Option<ActiveSession>,
    // A rejected/unsupported request must not show another dialog for every
    // recording. A new app launch is an explicit opportunity to try again.
    unavailable: bool,
}

impl InputController {
    async fn connect(
        &mut self,
        connection: Connection,
        app_id: &str,
        store: &dyn RestoreTokenStore,
    ) -> Result<(), String> {
        let id = AppID::try_from(app_id).map_err(|_| "Invalid desktop application identity")?;
        // Fail closed if the host cannot establish its own identity. Do not
        // acquire an anonymous, shared permission on older desktop stacks.
        ashpd::register_host_app_with_connection(connection.clone(), id)
            .await
            .map_err(|_| "Desktop application identification is unavailable")?;
        let portal = RemoteDesktop::with_connection(connection)
            .await
            .map_err(|_| "Desktop input portal is unavailable")?;
        if portal.version() < 2
            || !portal
                .available_device_types()
                .await
                .map_err(|_| "Desktop input capabilities are unavailable")?
                .contains(DeviceType::Keyboard)
        {
            return Err("Desktop does not support persistent keyboard approval".into());
        }
        let session = Arc::new(
            portal
                .create_session(Default::default())
                .await
                .map_err(|_| "Could not create desktop input session")?,
        );
        let setup = async {
            let token = store.load().await;
            portal
                .select_devices(
                    &session,
                    SelectDevicesOptions::default()
                        .set_devices(Some(DeviceType::Keyboard.into()))
                        .set_persist_mode(PersistMode::ExplicitlyRevoked)
                        .set_restore_token(token.as_deref()),
                )
                .await
                .map_err(|_| "Could not select desktop keyboard")?
                .response()
                .map_err(|_| "Desktop keyboard selection was rejected")?;
            let response = portal
                .start(&session, None, Default::default())
                .await
                .map_err(|_| "Could not request desktop input approval")?
                .response()
                .map_err(|_| "Desktop input approval was declined")?;
            if !response.devices().contains(DeviceType::Keyboard) {
                return Err("Desktop keyboard approval was not granted".to_string());
            }
            // Restore tokens are single use and rotate on every successful
            // Start. Clear obsolete tokens when persistence was not granted.
            // Storage failure must not discard a usable session or repeat the
            // approval during this run. The user still controls revocation.
            if store.save(response.restore_token()).await.is_err() {
                log::warn!("Desktop input approval could not be saved securely; it will last for this app session only");
            }
            let closed = Arc::new(AtomicBool::new(false));
            let signal = closed.clone();
            let observed_session = session.clone();
            let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
            let closed_task = tokio::spawn(async move {
                let events = observed_session.receive_closed().await;
                let Ok(mut events) = events else {
                    let _ = ready_tx.send(false);
                    return;
                };
                let _ = ready_tx.send(true);
                let _ = events.next().await;
                signal.store(true, Ordering::Release);
            });
            if ready_rx.await != Ok(true) {
                closed_task.abort();
                return Err("Could not observe desktop input session closure".into());
            }
            Ok((closed, closed_task))
        }.await;
        match setup {
            Ok((closed, closed_task)) => {
                self.active = Some(ActiveSession {
                    portal,
                    session,
                    closed,
                    closed_task,
                });
                Ok(())
            }
            Err(error) => {
                let _ = session.close().await;
                Err(error)
            }
        }
    }

    /// A single user-triggered paste, with no replay after partial delivery.
    /// `connection` is lazy: repeated pastes reuse the live grant and connection.
    pub(crate) async fn paste(
        &mut self,
        connection: impl std::future::Future<Output = Result<Connection, String>>,
        app_id: &str,
        store: &dyn RestoreTokenStore,
        input: (InputShortcut, bool),
        before_input: impl FnOnce() -> Result<(), String>,
        wait: impl Fn(u64) -> futures_util::future::BoxFuture<'static, ()>,
    ) -> Result<(), String> {
        let (shortcut, hit_enter) = input;
        if self.unavailable {
            return Err("Desktop input is unavailable for this app session".into());
        }
        if self
            .active
            .as_ref()
            .is_some_and(|session| session.closed.load(Ordering::Acquire))
        {
            self.active.take();
        }
        if self.active.is_none() {
            // Optional recording-side media effects must not interrupt capture
            // with a permission dialog. Only an explicit paste/copy may ask.
            if matches!(shortcut, InputShortcut::MediaPlayPause) {
                return Err("Desktop keyboard approval is required before media control".into());
            }
            let setup = match connection.await {
                Ok(connection) => self.connect(connection, app_id, store).await,
                Err(error) => Err(error),
            };
            if let Err(error) = setup {
                self.unavailable = true;
                return Err(error);
            }
        }
        // First-use approval may take minutes. Prepare the clipboard only once
        // it completes, not before the dialog while other apps can change it.
        before_input()?;
        let active = self
            .active
            .as_ref()
            .ok_or("Desktop input session is missing")?;
        let (modifiers, key) = paste_keys(shortcut);
        let mut release_key = false;
        let result = async {
            for modifier in modifiers {
                notify(active, *modifier, KeyState::Pressed).await?;
            }
            wait(50).await;
            release_key = true;
            notify(active, key, KeyState::Pressed).await?;
            notify(active, key, KeyState::Released).await?;
            release_key = false;
            wait(50).await;
            Ok::<_, String>(())
        }
        .await;
        let mut cleanup_error = None;
        if release_key {
            // A failed D-Bus reply may still have delivered the key press.
            if let Err(error) = notify(active, key, KeyState::Released).await {
                cleanup_error = Some(error);
            }
        }
        for modifier in modifiers.iter().rev() {
            if let Err(error) = notify(active, *modifier, KeyState::Released).await {
                cleanup_error = Some(error);
            }
        }
        let result = result.and(cleanup_error.map_or(Ok(()), Err));
        let result = match result {
            Ok(()) if hit_enter => {
                wait(50).await;
                let press = notify(active, 0xff0d, KeyState::Pressed).await;
                let release = notify(active, 0xff0d, KeyState::Released).await;
                if release.is_err() {
                    let _ = notify(active, 0xff0d, KeyState::Released).await;
                }
                press.and(release)
            }
            result => result,
        };
        if result.is_err() {
            // No automatic re-paste: the compositor may already have delivered
            // part of this chord. Subsequent user actions may restore the grant.
            let _ = active.session.close().await;
            self.active.take();
        }
        result
    }
}

async fn notify(active: &ActiveSession, key: i32, state: KeyState) -> Result<(), String> {
    active
        .portal
        .notify_keyboard_keysym(&active.session, key, state, Default::default())
        .await
        .map_err(|_| "Desktop keyboard event was rejected".into())
}

fn paste_keys(shortcut: InputShortcut) -> (&'static [i32], i32) {
    match shortcut {
        InputShortcut::Paste(PasteShortcut::System | PasteShortcut::CtrlV) => (&[0xffe3], 0x76),
        InputShortcut::Paste(PasteShortcut::CtrlShiftV) => (&[0xffe3, 0xffe1], 0x76),
        InputShortcut::Paste(PasteShortcut::ShiftInsert) => (&[0xffe1], 0xff63),
        InputShortcut::Paste(PasteShortcut::CmdV) => (&[0xffeb], 0x76),
        InputShortcut::Copy => (&[0xffe3], 0x63),
        InputShortcut::MediaPlayPause => (&[], 0x1008ff14),
    }
}

pub(crate) async fn send_shortcut(
    app: &tauri::AppHandle,
    shortcut: InputShortcut,
    hit_enter: bool,
    before_input: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    struct SecureStore;
    impl RestoreTokenStore for SecureStore {
        fn load(&self) -> futures_util::future::BoxFuture<'_, Option<String>> {
            Box::pin(async {
                tauri::async_runtime::spawn_blocking(crate::secrets::load_desktop_input_token)
                    .await
                    .ok()
                    .flatten()
            })
        }
        fn save<'a>(
            &'a self,
            token: Option<&'a str>,
        ) -> futures_util::future::BoxFuture<'a, Result<(), String>> {
            let token = token.map(str::to_owned);
            Box::pin(async move {
                tauri::async_runtime::spawn_blocking(move || {
                    crate::secrets::save_desktop_input_token(token.as_deref())
                })
                .await
                .map_err(|_| "Desktop input secure-storage worker failed".to_string())?
            })
        }
    }
    static CONTROLLER: OnceLock<Mutex<InputController>> = OnceLock::new();
    let controller = CONTROLLER.get_or_init(|| Mutex::new(InputController::default()));
    let mut controller = if matches!(shortcut, InputShortcut::MediaPlayPause) {
        controller.try_lock().map_err(|_| "Desktop input is busy")?
    } else {
        controller.lock().await
    };
    controller
        .paste(
            async {
                Connection::session()
                    .await
                    .map_err(|_| "Desktop session bus is unavailable".into())
            },
            &app.config().identifier,
            &SecureStore,
            (shortcut, hit_enter),
            before_input,
            |ms| Box::pin(tokio::time::sleep(std::time::Duration::from_millis(ms))),
        )
        .await
}

#[cfg(test)]
#[path = "tests/wayland_input.rs"]
mod tests;
