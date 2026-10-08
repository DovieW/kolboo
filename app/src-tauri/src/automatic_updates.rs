//! Signed release updates are staged by Rust, independently of webview lifetime.
//! Installation only takes place at an explicit Quit, never on an idle timer or
//! when closing the main window to the tray. OS shutdown is not delayed.
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Manager, RunEvent, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio_util::sync::CancellationToken;

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const NETWORK_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Default)]
struct PendingUpdate {
    download: Mutex<Option<(Update, Vec<u8>)>>,
    shutdown: CancellationToken,
    installing: AtomicBool,
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
    if !enabled || crate::cli::is_cli_invocation() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(run(app));
}

async fn run<R: Runtime>(app: AppHandle<R>) {
    let shutdown = app.state::<PendingUpdate>().shutdown.clone();
    loop {
        let result = tokio::select! {
            _ = shutdown.cancelled() => return,
            result = stage(&app) => result,
        };
        if result.is_err() {
            // Connectivity and signature failures must not break BYOK or
            // leak URLs, response bodies, file paths, or upstream errors.
            log::warn!("Automatic update check/download failed; will retry later");
        }
        tokio::select! {
            _ = shutdown.cancelled() => return,
            _ = tokio::time::sleep(CHECK_INTERVAL) => {}
        }
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
    let Some(mut update) = updater.check().await? else {
        return Ok(());
    };
    if pending
        .download
        .lock()
        .unwrap()
        .as_ref()
        .map(|(u, _)| &u.version)
        == Some(&update.version)
    {
        return Ok(());
    }
    update.timeout = Some(NETWORK_TIMEOUT);
    // The plugin verifies the committed public-key signature before returning
    // bytes. Only verified downloads may replace a previously staged update.
    let bytes = update.download(ignore_chunk, download_finished).await?;
    *pending.download.lock().unwrap() = Some((update, bytes));
    log::info!("Signed update downloaded; will install when Kolboo is quit");
    Ok(())
}

fn ignore_chunk(_: usize, _: Option<u64>) {}
fn download_finished() {}

fn on_event(app: &AppHandle, event: &RunEvent) {
    // Cancellation covers real quit, OS shutdown and runtime teardown. Closing
    // a webview is not an exit request, so checks continue while tray-only.
    if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
        app.state::<PendingUpdate>().shutdown.cancel();
    }
    if let RunEvent::ExitRequested {
        code: Some(0), api, ..
    } = event
    {
        if app
            .state::<PendingUpdate>()
            .installing
            .load(Ordering::SeqCst)
        {
            api.prevent_exit();
            return;
        }
        let pending = app.state::<PendingUpdate>().download.lock().unwrap().take();
        if let Some((update, bytes)) = pending {
            app.state::<PendingUpdate>()
                .installing
                .store(true, Ordering::SeqCst);
            api.prevent_exit();
            let app = app.clone();
            // Native installation can synchronously marshal GUI operations.
            // Never hold our mutex or block the event-loop thread across it.
            tauri::async_runtime::spawn_blocking(move || {
                if update.install(bytes).is_err() {
                    log::warn!(
                        "Automatic update installation failed; will retry on the next launch"
                    );
                }
                // Windows' installer exits/restarts the process itself. Other
                // platforms and failed installs complete the original Quit.
                app.state::<PendingUpdate>()
                    .installing
                    .store(false, Ordering::SeqCst);
                app.exit(0);
            });
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
