//! One owner for background and user-requested signed updates. Downloads never
//! interrupt work; installation happens on explicit Quit or an idle UI request.
use schemars::JsonSchema;
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Manager, RunEvent, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio_util::sync::CancellationToken;

mod installation;
use installation::InstallTarget;

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const NETWORK_TIMEOUT: Duration = Duration::from_secs(120);
const DISABLED: &str = "This build uses manual downloads.";
const CHECK_FAILED: &str =
    "Couldn't check or download a verified update. Check your connection and try again.";
const INSTALL_FAILED: &str = "Couldn't install the update. Check installation-folder permissions and try again, or download the release.";

#[derive(Clone, Copy, Debug, Default, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    #[default]
    Idle,
    Checking,
    Downloading,
    Ready,
    Installing,
    Error,
}

#[derive(Clone, Debug, Default, Serialize, JsonSchema)]
pub struct UpdateStatus {
    pub enabled: bool,
    pub checked: bool,
    pub phase: UpdatePhase,
    pub version: Option<String>,
    pub error: Option<String>,
    pub hint: String,
    pub can_install: bool,
}

#[derive(Default)]
struct PendingUpdate {
    download: Mutex<Option<(Update, Vec<u8>)>>,
    status: Mutex<UpdateStatus>,
    target: Mutex<InstallTarget>,
    shutdown: CancellationToken,
    operation: Arc<AtomicBool>,
    // Some(quit_requested) owns installation; completion and Quit synchronize
    // under this short-lived lock, never across filesystem or GUI calls.
    installing: Mutex<Option<bool>>,
}

// Held across I/O, including cancelled futures. Only one check or installation
// owns an update at a time, independent of how many webviews are open.
struct OperationPermit(Arc<AtomicBool>);
impl OperationPermit {
    fn acquire(pending: &PendingUpdate) -> Option<Self> {
        pending
            .operation
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self(pending.operation.clone()))
    }
}
impl Drop for OperationPermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

// Uses the pipeline's existing exclusive-operation lease: checking Idle alone
// would race F3/import/retry between the check and the native installer.
struct InstallLease(crate::pipeline::SharedPipeline);
impl InstallLease {
    fn acquire(pipeline: crate::pipeline::SharedPipeline) -> Result<Self, String> {
        pipeline.begin_recovery().map_err(|_| {
            "Finish recording or transcription before installing an update.".to_string()
        })?;
        Ok(Self(pipeline))
    }
}
impl Drop for InstallLease {
    fn drop(&mut self) {
        self.0.end_recovery();
    }
}

pub(crate) fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("automatic-updates")
        .setup(|app, _| {
            app.manage(PendingUpdate::default());
            Ok(())
        })
        .on_event(on_event)
        .build()
}

pub(crate) fn setup<R: Runtime>(app: &AppHandle<R>, enabled: bool) {
    let target = installation::detect_target(
        std::env::consts::OS,
        tauri::utils::platform::bundle_type(),
        &std::env::current_exe().unwrap_or_default(),
    );
    start(app, enabled && !crate::cli::is_cli_invocation(), target);
}

fn start<R: Runtime>(app: &AppHandle<R>, enabled: bool, target: InstallTarget) {
    let pending = app.state::<PendingUpdate>();
    let enabled = enabled && target != InstallTarget::Unsupported;
    *pending.status.lock().unwrap() = UpdateStatus {
        enabled,
        hint: if enabled { target.hint() } else { DISABLED }.into(),
        ..Default::default()
    };
    *pending.target.lock().unwrap() = target;
    if enabled {
        tauri::async_runtime::spawn(run(app.clone()));
    }
}

async fn run<R: Runtime>(app: AppHandle<R>) {
    let shutdown = app.state::<PendingUpdate>().shutdown.clone();
    loop {
        tokio::select! { _ = shutdown.cancelled() => return, _ = check_once(&app) => {} }
        tokio::select! { _ = shutdown.cancelled() => return, _ = tokio::time::sleep(CHECK_INTERVAL) => {} }
    }
}

fn phase(pending: &PendingUpdate, phase: UpdatePhase, error: Option<&str>) {
    let version = pending
        .download
        .lock()
        .unwrap()
        .as_ref()
        .map(|(update, _)| update.version.clone());
    let mut status = pending.status.lock().unwrap();
    status.phase = phase;
    status.version = version;
    status.error = error.map(str::to_string);
}

async fn check_once<R: Runtime>(app: &AppHandle<R>) {
    if let Some(permit) = OperationPermit::acquire(&app.state::<PendingUpdate>()) {
        phase(&app.state::<PendingUpdate>(), UpdatePhase::Checking, None);
        check_owned(app, permit).await;
    }
}

async fn check_owned<R: Runtime>(app: &AppHandle<R>, _permit: OperationPermit) {
    let pending = app.state::<PendingUpdate>();
    let result = tokio::select! {
        biased; // A shutdown already in progress must never start another request.
        _ = pending.shutdown.cancelled() => {
            let ready = pending.download.lock().unwrap().is_some();
            phase(&pending, if ready { UpdatePhase::Ready } else { UpdatePhase::Idle }, None);
            return;
        }
        result = stage(app) => result,
    };
    if result.is_err() {
        phase(
            &app.state::<PendingUpdate>(),
            UpdatePhase::Error,
            Some(CHECK_FAILED),
        );
        log::warn!("Automatic update check/download failed; will retry later");
    }
}

async fn stage<R: Runtime>(app: &AppHandle<R>) -> tauri_plugin_updater::Result<()> {
    let updater = app.updater_builder().timeout(NETWORK_TIMEOUT).build()?;
    stage_update(updater, &app.state::<PendingUpdate>()).await
}

async fn stage_update(
    updater: tauri_plugin_updater::Updater,
    pending: &PendingUpdate,
) -> tauri_plugin_updater::Result<()> {
    let found = updater.check().await?;
    pending.status.lock().unwrap().checked = true;
    let Some(mut update) = found else {
        *pending.download.lock().unwrap() = None;
        phase(pending, UpdatePhase::Idle, None);
        return Ok(());
    };
    let already_downloaded = pending
        .download
        .lock()
        .unwrap()
        .as_ref()
        .map(|(u, _)| &u.version)
        == Some(&update.version);
    if !already_downloaded {
        phase(pending, UpdatePhase::Downloading, None);
        update.timeout = Some(NETWORK_TIMEOUT);
        // Tauri verifies our public-key signature before returning any bytes.
        let bytes = update.download(ignore_chunk, download_finished).await?;
        *pending.download.lock().unwrap() = Some((update, bytes));
        log::info!("Signed update downloaded; will install when Kolboo is quit");
    }
    phase(pending, UpdatePhase::Ready, None);
    Ok(())
}
fn ignore_chunk(_: usize, _: Option<u64>) {}
fn download_finished() {}

fn status<R: Runtime>(app: &AppHandle<R>) -> UpdateStatus {
    let pending = app.state::<PendingUpdate>();
    let mut status = pending.status.lock().unwrap().clone();
    status.can_install = status.enabled
        && pending.download.lock().unwrap().is_some()
        && !pending.operation.load(Ordering::SeqCst)
        && app
            .try_state::<crate::pipeline::SharedPipeline>()
            .is_some_and(|pipeline| {
                !pipeline.is_recovering() && pipeline.state().can_start_recording()
            });
    status
}

#[tauri::command]
pub(crate) fn get_update_status(app: AppHandle) -> UpdateStatus {
    status(&app)
}

fn request_check<R: Runtime>(app: &AppHandle<R>) -> Result<UpdateStatus, String> {
    if !status(app).enabled {
        return Err(DISABLED.into());
    }
    if let Some(permit) = OperationPermit::acquire(&app.state::<PendingUpdate>()) {
        phase(&app.state::<PendingUpdate>(), UpdatePhase::Checking, None);
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            check_owned(&app, permit).await;
        });
    }
    Ok(status(app))
}

#[tauri::command]
pub(crate) fn check_for_updates(app: AppHandle) -> Result<UpdateStatus, String> {
    request_check(&app)
}

#[tauri::command]
pub(crate) fn install_update(app: AppHandle) -> Result<(), String> {
    spawn_install(&app, false)
}

fn spawn_install(app: &AppHandle, quitting: bool) -> Result<(), String> {
    let pending = app.state::<PendingUpdate>();
    if !status(app).enabled {
        return Err(DISABLED.into());
    }
    let permit =
        OperationPermit::acquire(&pending).ok_or("An update operation is already in progress.")?;
    let lease = if quitting {
        None
    } else {
        let pipeline = app
            .try_state::<crate::pipeline::SharedPipeline>()
            .ok_or("Recording status is unavailable. Try again later.")?;
        Some(InstallLease::acquire(pipeline.inner().clone())?)
    };
    let (update, bytes) = pending
        .download
        .lock()
        .unwrap()
        .take()
        .ok_or("No verified update is ready to install.")?;
    let target = pending.target.lock().unwrap().clone();
    *pending.installing.lock().unwrap() = Some(quitting);
    phase(&pending, UpdatePhase::Installing, None);
    pending.status.lock().unwrap().version = Some(update.version.clone());
    let app = app.clone();
    // Never hold a mutex or block the GUI thread across native installation.
    tauri::async_runtime::spawn_blocking(move || {
        let failed = target.install(&update, &bytes).is_err();
        let pending = app.state::<PendingUpdate>();
        // Release ownership atomically with reading any deferred Quit request.
        let quit = pending.installing.lock().unwrap().take().unwrap();
        if failed && !quit {
            *pending.download.lock().unwrap() = Some((update, bytes));
            phase(&pending, UpdatePhase::Ready, Some(INSTALL_FAILED));
        } else {
            phase(&pending, UpdatePhase::Idle, None);
        }
        drop(lease);
        drop(permit);
        // Windows' updater hands off to an installer/restart. On Linux/Mac,
        // successful installation closes Kolboo; the next launch is updated.
        if !failed || quit {
            app.exit(0);
        }
    });
    Ok(())
}

fn on_event(app: &AppHandle, event: &RunEvent) {
    if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
        app.state::<PendingUpdate>().shutdown.cancel();
    }
    if let RunEvent::ExitRequested {
        code: Some(0), api, ..
    } = event
    {
        let pending = app.state::<PendingUpdate>();
        let mut installing = pending.installing.lock().unwrap();
        if let Some(quit) = installing.as_mut() {
            *quit = true;
            api.prevent_exit();
        } else {
            drop(installing);
            if spawn_install(app, true).is_ok() {
                api.prevent_exit();
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
