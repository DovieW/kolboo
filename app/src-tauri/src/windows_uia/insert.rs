use super::insert_plan::build_insert_plan;
use super::target_match::target_matches;
use super::types::{WindowsInsertMethod, WindowsTextTargetSnapshot};
use super::verify::{verify_or_fallback, VerificationInput};
use crate::windows_uia::app_identity::app_identity_key;
use crate::windows_uia::capability_memory::{
    load_capability_memory, record_insertion_result, save_capability_memory,
};
use crate::windows_uia::safety::insert_block_reason;

#[cfg(target_os = "windows")]
use crate::request_log::{LogLevel, RequestLogStore};
#[cfg(target_os = "windows")]
use tauri::Manager;

#[cfg(target_os = "windows")]
use crate::text::inject::{
    copy_to_clipboard_and_notify, type_text_as_keystrokes, type_text_blocking_with_options,
};
#[cfg(target_os = "windows")]
use crate::windows_uia::client::UiaClient;
#[cfg(target_os = "windows")]
use crate::windows_uia::com::initialize_com_mta;
#[cfg(target_os = "windows")]
use crate::windows_uia::snapshot::capture_snapshot;

#[cfg(target_os = "windows")]
use std::time::SystemTime;

#[cfg(target_os = "windows")]
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(target_os = "windows")]
fn capture_focused_snapshot() -> Result<WindowsTextTargetSnapshot, String> {
    let _guard = initialize_com_mta()?;
    let client = UiaClient::new()?;
    let element = client.get_focused_element_with_retry(2, 40)?;
    capture_snapshot(&element, now_ms())
}

#[cfg(target_os = "windows")]
fn log_request_entry(app: &tauri::AppHandle, level: LogLevel, message: impl Into<String>) {
    let message = message.into();
    if let Some(store) = app.try_state::<RequestLogStore>() {
        let _ = store.with_current(|log| log.log(level, message, None));
    }
}

#[cfg(target_os = "windows")]
pub fn insert_text_with_snapshot(
    app: &tauri::AppHandle,
    text: &str,
    initial_snapshot: Option<WindowsTextTargetSnapshot>,
    allow_paste: bool,
    allow_typing: bool,
) -> Result<WindowsInsertMethod, String> {
    let snapshot = if let Some(snapshot) = initial_snapshot {
        snapshot
    } else {
        capture_focused_snapshot()?
    };

    let plan = build_insert_plan(snapshot.supports_value_pattern, allow_paste, allow_typing);
    log::info!(
        "UIA insert: plan method={:?} allowed={} (paste={}, typing={})",
        plan.method,
        plan.allowed,
        allow_paste,
        allow_typing
    );
    log_request_entry(
        app,
        LogLevel::Info,
        format!(
            "UIA insert plan: method={:?} allowed={} (paste={}, typing={})",
            plan.method, plan.allowed, allow_paste, allow_typing
        ),
    );
    if !plan.allowed {
        log::info!("UIA insert: plan not allowed, using safe fallback");
        log_request_entry(
            app,
            LogLevel::Warn,
            "UIA insert: plan not allowed, using safe fallback".to_string(),
        );
        copy_to_clipboard_and_notify(app, text)?;
        return Ok(WindowsInsertMethod::None);
    }

    // Capture a fresh snapshot for safety checks - the initial snapshot from recording stop may
    // be stale (focus may not have settled on the editable field yet, causing false read_only).
    let current_snapshot = capture_focused_snapshot()?;
    let target_matches_now = target_matches(&snapshot, &current_snapshot);

    // For password fields, block if EITHER snapshot showed password (security-first).
    // For enabled/read_only, use the current snapshot since those can be transient during focus.
    let safety_snapshot = WindowsTextTargetSnapshot {
        is_password: match (snapshot.is_password, current_snapshot.is_password) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            _ => current_snapshot.is_password,
        },
        is_enabled: current_snapshot.is_enabled,
        is_read_only: current_snapshot.is_read_only,
        ..current_snapshot.clone()
    };

    if let Some(reason) = insert_block_reason(&safety_snapshot) {
        log::info!(
            "UIA insert: blocked by safety policy (reason={}, is_password={:?}, is_enabled={:?}, is_read_only={:?}), using safe fallback",
            reason.as_str(),
            safety_snapshot.is_password,
            safety_snapshot.is_enabled,
            safety_snapshot.is_read_only
        );
        log_request_entry(
            app,
            LogLevel::Warn,
            format!(
                "UIA insert blocked by safety policy (reason={})",
                reason.as_str()
            ),
        );
        copy_to_clipboard_and_notify(app, text)?;
        return Ok(WindowsInsertMethod::None);
    }

    let mut method_error: Option<String> = None;
    let mut clipboard_restored: Option<bool> = None;

    let method_used = match plan.method {
        WindowsInsertMethod::UiaValuePattern => {
            method_error = Some("UIA ValuePattern insertion not implemented yet".to_string());
            WindowsInsertMethod::UiaValuePattern
        }
        WindowsInsertMethod::Paste => {
            if let Err(err) = type_text_blocking_with_options(text, false, true) {
                method_error = Some(err);
            } else {
                clipboard_restored = Some(true);
            }
            WindowsInsertMethod::Paste
        }
        WindowsInsertMethod::Typing => {
            if let Err(err) = type_text_as_keystrokes(text) {
                method_error = Some(err);
            }
            WindowsInsertMethod::Typing
        }
        WindowsInsertMethod::None => {
            method_error = Some("No insertion method available".to_string());
            WindowsInsertMethod::None
        }
    };

    let method_error_present = method_error.is_some();

    let verification = verify_or_fallback(VerificationInput {
        method_error,
        target_matches: target_matches_now,
        timed_out: false,
        clipboard_restored,
    });
    log::debug!(
        "UIA insert: attempted method={:?} target_match={} method_error={} clipboard_restored={:?}",
        method_used,
        target_matches_now,
        method_error_present,
        clipboard_restored
    );

    if verification.is_ok() {
        log::info!("UIA insert: verification ok (method={:?})", method_used);
        log_request_entry(
            app,
            LogLevel::Info,
            format!("UIA insert: verification ok (method={:?})", method_used),
        );
        if let Some(app_key) = app_identity_key(snapshot.exe_path.as_deref()) {
            if let Ok(mut memory) = load_capability_memory(app) {
                record_insertion_result(&mut memory, &app_key, method_used.clone(), true, now_ms());
                let _ = save_capability_memory(app, &memory);
            }
        }
        return Ok(method_used);
    }
    log::info!(
        "UIA insert: verification failed, falling back (method={:?})",
        method_used
    );
    log_request_entry(
        app,
        LogLevel::Warn,
        format!(
            "UIA insert: verification failed, falling back (method={:?})",
            method_used
        ),
    );

    // Fallback ladder: attempt paste then typing before safe fallback.
    if method_used != WindowsInsertMethod::Paste && allow_paste {
        if type_text_blocking_with_options(text, false, true).is_ok() {
            log::info!("UIA insert: fallback paste succeeded");
            log_request_entry(
                app,
                LogLevel::Info,
                "UIA insert: fallback paste succeeded".to_string(),
            );
            if let Some(app_key) = app_identity_key(snapshot.exe_path.as_deref()) {
                if let Ok(mut memory) = load_capability_memory(app) {
                    record_insertion_result(
                        &mut memory,
                        &app_key,
                        WindowsInsertMethod::Paste,
                        true,
                        now_ms(),
                    );
                    let _ = save_capability_memory(app, &memory);
                }
            }
            return Ok(WindowsInsertMethod::Paste);
        }
    }

    if method_used != WindowsInsertMethod::Typing && allow_typing {
        if type_text_as_keystrokes(text).is_ok() {
            log::info!("UIA insert: fallback typing succeeded");
            log_request_entry(
                app,
                LogLevel::Info,
                "UIA insert: fallback typing succeeded".to_string(),
            );
            if let Some(app_key) = app_identity_key(snapshot.exe_path.as_deref()) {
                if let Ok(mut memory) = load_capability_memory(app) {
                    record_insertion_result(
                        &mut memory,
                        &app_key,
                        WindowsInsertMethod::Typing,
                        true,
                        now_ms(),
                    );
                    let _ = save_capability_memory(app, &memory);
                }
            }
            return Ok(WindowsInsertMethod::Typing);
        }
    }

    if let Some(app_key) = app_identity_key(snapshot.exe_path.as_deref()) {
        if let Ok(mut memory) = load_capability_memory(app) {
            record_insertion_result(&mut memory, &app_key, method_used.clone(), false, now_ms());
            let _ = save_capability_memory(app, &memory);
        }
    }

    log::info!("UIA insert: all methods failed, using safe fallback");
    log_request_entry(
        app,
        LogLevel::Warn,
        "UIA insert: all methods failed, using safe fallback".to_string(),
    );
    copy_to_clipboard_and_notify(app, text)?;
    Ok(WindowsInsertMethod::None)
}

#[cfg(not(target_os = "windows"))]
pub fn insert_text_with_snapshot(
    _app: &tauri::AppHandle,
    _text: &str,
    _initial_snapshot: Option<WindowsTextTargetSnapshot>,
    _allow_paste: bool,
    _allow_typing: bool,
) -> Result<WindowsInsertMethod, String> {
    Ok(WindowsInsertMethod::None)
}
