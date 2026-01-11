use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::{Mutex, OnceLock};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tauri::AppHandle;

/// Delay after clipboard operations to ensure system stability
const CLIPBOARD_STABILIZATION_DELAY_MS: u64 = 50;

/// How many times we try to confirm the clipboard contains our injected text.
///
/// This mitigates a race where we press Ctrl+V before the clipboard update is fully visible
/// to the target application (or before Windows has finished committing the clipboard write).
const CLIPBOARD_VERIFY_ATTEMPTS: u32 = 10;

/// Delay between clipboard verification attempts.
const CLIPBOARD_VERIFY_DELAY_MS: u64 = 20;

/// Delay between keyboard key press and release events
const KEY_EVENT_DELAY_MS: u64 = 50;

/// Delay before restoring previous clipboard content
const CLIPBOARD_RESTORE_DELAY_MS: u64 = 100;

/// Extra delay after issuing the paste keystroke before we attempt to restore the clipboard.
///
/// Some target apps fetch clipboard contents *after* receiving the paste shortcut (lazy paste).
/// If we restore too soon, they can end up pasting the previous clipboard instead.
#[cfg(target_os = "windows")]
const CLIPBOARD_POST_PASTE_DELAY_MS: u64 = 450;

#[cfg(not(target_os = "windows"))]
const CLIPBOARD_POST_PASTE_DELAY_MS: u64 = 250;

const SERVER_URL: &str = "http://127.0.0.1:8765";

/// Global lock to ensure we never run multiple output injections concurrently.
///
/// Without this, two overlapping "type/paste" operations can interleave key events and
/// produce dropped/mangled text in target applications.
static OUTPUT_INJECTION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn output_injection_lock() -> &'static Mutex<()> {
    OUTPUT_INJECTION_LOCK.get_or_init(|| Mutex::new(()))
}

fn maybe_hit_enter(enigo: &mut Enigo, hit_enter: bool) -> Result<(), String> {
    if !hit_enter {
        return Ok(());
    }

    // Small delay to avoid racing the paste keystroke.
    thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));

    enigo
        .key(Key::Return, Direction::Click)
        .map_err(|e| e.to_string())?;

    Ok(())
}
fn set_clipboard_text_with_barrier(
    clipboard: &mut Clipboard,
    text: &str,
    exclude_from_clipboard_history: bool,
) -> Result<(), String> {
    set_clipboard_text_platform(clipboard, text, exclude_from_clipboard_history)?;

    // Try to confirm the clipboard reflects the new text before we issue Ctrl+V.
    // If reading fails (clipboard is busy), keep retrying briefly.
    for _ in 0..CLIPBOARD_VERIFY_ATTEMPTS {
        thread::sleep(Duration::from_millis(CLIPBOARD_VERIFY_DELAY_MS));
        match clipboard.get_text() {
            Ok(current) if current == text => return Ok(()),
            Ok(_) => continue,
            Err(_) => continue,
        }
    }

    // Fall back to a small stabilization delay. Even if verification failed,
    // the clipboard write may still succeed; this avoids making failure worse.
    thread::sleep(Duration::from_millis(CLIPBOARD_STABILIZATION_DELAY_MS));
    log::debug!(
        "Clipboard barrier: could not confirm clipboard contents after {} attempts; proceeding",
        CLIPBOARD_VERIFY_ATTEMPTS
    );
    Ok(())
}

fn set_clipboard_text_platform(
    clipboard: &mut Clipboard,
    text: &str,
    exclude_from_clipboard_history: bool,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if exclude_from_clipboard_history {
            return set_clipboard_text_windows_excluding_history(text);
        }
    }

    clipboard.set_text(text).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn set_clipboard_text_windows_excluding_history(text: &str) -> Result<(), String> {
    use windows::ApplicationModel::DataTransfer::{Clipboard, ClipboardContentOptions, DataPackage};
    use windows::core::HSTRING;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

    // WinRT clipboard APIs require COM initialization on the current thread.
    // Best-effort: tolerate "already initialized" / "wrong apartment" scenarios.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let package = DataPackage::new().map_err(|e| e.to_string())?;
    package
        .SetText(&HSTRING::from(text))
        .map_err(|e| e.to_string())?;

    let options = ClipboardContentOptions::new().map_err(|e| e.to_string())?;
    options
        .SetIsAllowedInHistory(false)
        .map_err(|e| e.to_string())?;
    options
        .SetIsRoamable(false)
        .map_err(|e| e.to_string())?;

    Clipboard::SetContentWithOptions(&package, &options).map_err(|e| e.to_string())?;

    // Encourage the system to commit the content promptly.
    let _ = Clipboard::Flush();

    Ok(())
}

/// Output mode for transcribed text
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum OutputMode {
    /// Copy to clipboard and simulate Ctrl+V/Cmd+V, then restore clipboard
    #[default]
    Paste,
    /// Paste and keep in clipboard (no restore)
    PasteAndClipboard,
    /// Just copy to clipboard (no paste)
    Clipboard,
    // NOTE: Keystrokes mode was removed/disabled due to reliability issues across targets.
}

impl OutputMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "paste" => OutputMode::Paste,
            "paste_and_clipboard" => OutputMode::PasteAndClipboard,
            "clipboard" => OutputMode::Clipboard,
            // Legacy/disabled values: map to paste so existing settings.json doesn't break.
            "keystrokes" => OutputMode::Paste,
            "keystrokes_and_clipboard" => OutputMode::Paste,
            // Handle legacy value
            "auto_paste" => OutputMode::Paste,
            _ => OutputMode::Paste,
        }
    }
}

#[tauri::command]
pub async fn get_server_url() -> String {
    SERVER_URL.to_string()
}

#[tauri::command]
pub async fn type_text(app: AppHandle, text: String) -> Result<(), String> {
    // macOS HIToolbox APIs (used by enigo) must run on the main thread
    // Use a channel to get the result back from the main thread
    let (tx, rx) = mpsc::channel::<Result<(), String>>();

    app.run_on_main_thread(move || {
        // Serialize output across all modes to avoid interleaving key events.
        let _guard = match output_injection_lock().lock() {
            Ok(g) => g,
            Err(_) => {
                let _ = tx.send(Err("Output lock poisoned".to_string()));
                return;
            }
        };

        let result = type_text_blocking(&text, false);
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;

    // Wait for result from main thread
    rx.recv().map_err(|e| e.to_string())?
}

/// Output text based on the specified mode
pub fn output_text_with_mode(text: &str, mode: OutputMode, hit_enter: bool) -> Result<(), String> {
    let _guard = output_injection_lock()
        .lock()
        .map_err(|_| "Output lock poisoned".to_string())?;

    match mode {
        OutputMode::Paste => type_text_blocking(text, hit_enter),
        OutputMode::PasteAndClipboard => paste_and_keep_clipboard(text, hit_enter),
        OutputMode::Clipboard => copy_to_clipboard(text),
    }
}

/// Copy text to clipboard and paste, keeping text in clipboard (no restore)
pub fn paste_and_keep_clipboard(text: &str, hit_enter: bool) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;

    // Set new text and wait for it to become visible to readers (best-effort).
    // Even when we keep the text on the clipboard, avoid adding it to Win+V history.
    set_clipboard_text_with_barrier(&mut clipboard, text, true)?;

    // Simulate Ctrl+V / Cmd+V
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    enigo
        .key(modifier, Direction::Press)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));
    enigo
        .key(modifier, Direction::Release)
        .map_err(|e| e.to_string())?;

    maybe_hit_enter(&mut enigo, hit_enter)?;

    // Don't restore clipboard - keep the text there
    log::info!("Pasted {} chars (kept in clipboard)", text.len());
    Ok(())
}

/// Copy text to clipboard only (no paste)
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    // Avoid adding our output to Win+V history.
    set_clipboard_text_with_barrier(&mut clipboard, text, true)?;
    log::info!("Copied {} chars to clipboard", text.len());
    Ok(())
}

// Keystrokes mode intentionally disabled.
// (Kept as a stub in case any legacy call sites remain in downstream forks.)
#[allow(dead_code)]
pub fn type_as_keystrokes(_text: &str) -> Result<(), String> {
    Err("Keystrokes output mode is disabled".to_string())
}

/// Type text using clipboard and paste. Used internally by shortcut handlers.
pub fn type_text_blocking(text: &str, hit_enter: bool) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;

    // Save previous clipboard content (text only). If the previous clipboard isn't text,
    // don't try to "restore" it as an empty string.
    let previous: Option<String> = clipboard.get_text().ok();

    // Set new text and wait for it to become visible to readers (best-effort).
    // In the default "Paste" mode, we restore the clipboard afterwards, so on Windows we also
    // try to exclude the injected text from the OS clipboard history.
    set_clipboard_text_with_barrier(&mut clipboard, text, true)?;

    // Simulate Ctrl+V / Cmd+V
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    enigo
        .key(modifier, Direction::Press)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));
    enigo
        .key(modifier, Direction::Release)
        .map_err(|e| e.to_string())?;

    maybe_hit_enter(&mut enigo, hit_enter)?;

    // Give the target app time to fetch clipboard contents after the paste shortcut.
    // (Some apps fetch lazily, and restoring too soon causes the *previous* clipboard to paste.)
    thread::sleep(Duration::from_millis(CLIPBOARD_POST_PASTE_DELAY_MS));

    // Restore previous clipboard after a delay
    thread::sleep(Duration::from_millis(CLIPBOARD_RESTORE_DELAY_MS));

    // Only restore if:
    // 1) we actually captured a previous text value, and
    // 2) the clipboard still contains our injected text (avoid clobbering user changes).
    if let Some(previous) = previous {
        let should_restore = clipboard
            .get_text()
            .ok()
            .is_some_and(|current| current == text);

        if should_restore {
            // Avoid creating a duplicate clipboard-history entry for the restored content.
            let _ = set_clipboard_text_platform(&mut clipboard, &previous, true);
        } else {
            log::debug!("Clipboard restore skipped (clipboard changed after paste)");
        }
    } else {
        log::debug!("Clipboard restore skipped (previous clipboard was not text)");
    }

    Ok(())
}

/// Best-effort: attempt to capture highlighted/selected text from the currently focused app.
///
/// Strategy:
/// - (Optionally) save the current clipboard text (text-only)
/// - send Ctrl+C / Cmd+C to copy selection
/// - read clipboard text
/// - restore the previous clipboard text when safe
///
/// Notes / tradeoffs:
/// - This is inherently best-effort; not all apps expose copyable selection.
/// - We only preserve/restore *text* clipboard content (same limitation as paste mode).
/// - On Windows, we exclude our transient clipboard writes from Win+V history.
pub fn probe_selected_text_via_copy() -> Result<Option<String>, String> {
    // Serialize with output injections so key events can't interleave.
    let _guard = output_injection_lock()
        .lock()
        .map_err(|_| "Output lock poisoned".to_string())?;

    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;

    // Save previous clipboard content (text only).
    let previous: Option<String> = clipboard.get_text().ok();

    // Simulate Ctrl+C / Cmd+C.
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    enigo
        .key(modifier, Direction::Press)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));
    enigo
        .key(Key::Unicode('c'), Direction::Click)
        .map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));
    enigo
        .key(modifier, Direction::Release)
        .map_err(|e| e.to_string())?;

    // Wait briefly for clipboard to update and then read it.
    let mut captured: Option<String> = None;
    for _ in 0..CLIPBOARD_VERIFY_ATTEMPTS {
        thread::sleep(Duration::from_millis(CLIPBOARD_VERIFY_DELAY_MS));
        if let Ok(current) = clipboard.get_text() {
            if current.trim().is_empty() {
                continue;
            }
            captured = Some(current);
            break;
        }
    }

    // On Windows, the copy action is performed by the *target application* and may be recorded
    // by Win+V clipboard history. We can't directly control that, but we can try to reduce
    // history pollution by immediately rewriting the captured content using WinRT clipboard
    // options with history disabled.
    #[cfg(target_os = "windows")]
    if let Some(captured_text) = captured.as_deref() {
        // Best-effort: if this fails, we still proceed with restoration below.
        let _ = set_clipboard_text_platform(&mut clipboard, captured_text, true);
    }

    // Restore previous clipboard text (best-effort) if we captured something.
    // If we didn't capture anything, restoring is unnecessary and can be risky
    // (could clobber user clipboard changes).
    if let (Some(prev), Some(captured_text)) = (previous, captured.as_ref()) {
        let should_restore = clipboard
            .get_text()
            .ok()
            .map(|current| current == captured_text.as_str())
            .unwrap_or(false);

        if should_restore {
            // Avoid creating a duplicate clipboard-history entry for the restored content.
            let _ = set_clipboard_text_platform(&mut clipboard, &prev, true);
        } else {
            log::debug!("Quick replace probe: clipboard changed; skipping restore");
        }
    }

    Ok(captured)
}
