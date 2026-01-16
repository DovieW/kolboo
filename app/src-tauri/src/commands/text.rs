use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tauri::AppHandle;
use uuid::Uuid;

#[cfg(desktop)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextGrabMethod {
    /// Disable highlighted-selection capture entirely.
    ///
    /// When set, Kolboo will not inject any copy shortcut (and will not touch the clipboard)
    /// for Quick Ask / Quick Replace context capture.
    None,
    /// Standard copy shortcut (Cmd+C on macOS, Ctrl+C elsewhere).
    CtrlC,
    /// Terminal-friendly copy shortcut on Windows (Ctrl+Shift+C).
    ///
    /// This is opt-in via settings; Kolboo will not auto-detect.
    CtrlShiftC,
    /// Alternative copy shortcut on Windows (Ctrl+Insert).
    ///
    /// Some console hosts treat Ctrl+Shift+C as a Ctrl+C cancel event. Ctrl+Insert is
    /// commonly bound to copy-selection and avoids the Ctrl+C cancel semantics.
    CtrlInsert,

    /// Clipboard-only selection capture.
    ///
    /// This mode injects no keys at all and simply reads the current clipboard text,
    /// returning it only when it *changed* since the last clipboard-only probe.
    ///
    /// Intended for apps/terminals that support "copy on select".
    ClipboardOnly,
}

/// Delay after clipboard operations to ensure system stability
const CLIPBOARD_STABILIZATION_DELAY_MS: u64 = 50;

/// How many times we try to confirm the clipboard contains our injected text.
///
/// This mitigates a race where we press Ctrl+V before the clipboard update is fully visible
/// to the target application (or before Windows has finished committing the clipboard write).
const CLIPBOARD_VERIFY_ATTEMPTS: u32 = 10;

/// Delay between clipboard verification attempts.
const CLIPBOARD_VERIFY_DELAY_MS: u64 = 20;

/// How long we wait for the *target application* to update the clipboard after we inject a copy
/// shortcut during highlighted-selection probing.
///
/// This is intentionally longer than `CLIPBOARD_VERIFY_*` which is used as a short barrier for
/// *our own* clipboard writes during output injection.
const SELECTION_PROBE_VERIFY_ATTEMPTS: u32 = 40;

/// Delay between selection-probe clipboard reads.
const SELECTION_PROBE_VERIFY_DELAY_MS: u64 = 25;

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

/// Snapshot used by `ContextGrabMethod::ClipboardOnly` to avoid repeatedly treating an
/// unchanged clipboard as a selection capture.
#[cfg(desktop)]
static CLIPBOARD_ONLY_LAST_TEXT: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn output_injection_lock() -> &'static Mutex<()> {
    OUTPUT_INJECTION_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(desktop)]
fn clipboard_only_last_text_lock() -> &'static Mutex<Option<String>> {
    CLIPBOARD_ONLY_LAST_TEXT.get_or_init(|| Mutex::new(None))
}

fn basename_for_log(path: &str) -> &str {
    let trimmed = path.trim().trim_matches('"');
    trimmed.rsplit(['\\', '/']).next().unwrap_or(trimmed)
}

fn with_pressed_key<T>(
    enigo: &mut Enigo,
    key: Key,
    work: impl FnOnce(&mut Enigo) -> Result<T, String>,
) -> Result<T, String> {
    enigo
        .key(key, Direction::Press)
        .map_err(|e| e.to_string())?;

    // Ensure we always release, even if `work` fails.
    let result = work(enigo);
    let _ = enigo.key(key, Direction::Release);
    result
}

fn release_common_modifiers_best_effort(enigo: &mut Enigo) {
    // If we ever miss a key-up (or a release fails), users can experience "stuck" modifiers.
    // Best-effort attempt to reset common modifiers.
    let _ = enigo.key(Key::Shift, Direction::Release);
    let _ = enigo.key(Key::Control, Direction::Release);
    let _ = enigo.key(Key::Alt, Direction::Release);
    let _ = enigo.key(Key::Meta, Direction::Release);
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
    _exclude_from_clipboard_history: bool,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if _exclude_from_clipboard_history {
            return set_clipboard_text_windows_excluding_history(text);
        }
    }

    clipboard.set_text(text).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn set_clipboard_text_windows_excluding_history(text: &str) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::ApplicationModel::DataTransfer::{
        Clipboard, ClipboardContentOptions, DataPackage,
    };
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
    options.SetIsRoamable(false).map_err(|e| e.to_string())?;

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
#[allow(dead_code)]
pub fn output_text_with_mode(text: &str, mode: OutputMode, hit_enter: bool) -> Result<(), String> {
    output_text_with_mode_options(text, mode, hit_enter, true)
}

/// Output text with explicit control over whether we read/restore the previous clipboard.
///
/// - When `preserve_clipboard` is true, Paste mode will save+restore the previous *text* clipboard
///   value when safe.
/// - When false, Paste mode will never read the clipboard and will not attempt any restore.
pub fn output_text_with_mode_options(
    text: &str,
    mode: OutputMode,
    hit_enter: bool,
    preserve_clipboard: bool,
) -> Result<(), String> {
    let _guard = output_injection_lock()
        .lock()
        .map_err(|_| "Output lock poisoned".to_string())?;

    match mode {
        OutputMode::Paste => type_text_blocking_with_options(text, hit_enter, preserve_clipboard),
        OutputMode::PasteAndClipboard => paste_and_keep_clipboard(text, hit_enter),
        OutputMode::Clipboard => copy_to_clipboard(text),
    }
}

struct ClipboardRestoreGuard {
    enabled: bool,
    previous: Option<String>,
    injected: String,
    paste_sent: bool,
}

impl ClipboardRestoreGuard {
    fn new(previous: Option<String>, injected: &str, enabled: bool) -> Self {
        Self {
            enabled,
            previous,
            injected: injected.to_string(),
            paste_sent: false,
        }
    }

    fn mark_paste_sent(&mut self) {
        self.paste_sent = true;
    }
}

impl Drop for ClipboardRestoreGuard {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }

        let Some(previous) = self.previous.as_ref() else {
            // If we didn't capture previous text, do not overwrite a non-text clipboard.
            return;
        };

        // If we actually attempted to paste, give the target app time to fetch clipboard
        // contents (lazy paste). Then apply the normal restore delay.
        if self.paste_sent {
            thread::sleep(Duration::from_millis(CLIPBOARD_POST_PASTE_DELAY_MS));
        }
        thread::sleep(Duration::from_millis(CLIPBOARD_RESTORE_DELAY_MS));

        let Ok(mut clipboard) = Clipboard::new() else {
            return;
        };

        // Only restore if the clipboard still contains our injected text.
        let should_restore = clipboard
            .get_text()
            .ok()
            .is_some_and(|current| current == self.injected);

        if should_restore {
            // Avoid creating a duplicate clipboard-history entry for the restored content.
            let _ = set_clipboard_text_platform(&mut clipboard, previous, true);
        }
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

    let result = with_pressed_key(&mut enigo, modifier, |enigo| {
        thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));

        #[cfg(target_os = "windows")]
        {
            // Physical 'V' key (scancode set 1). More reliable for modifier shortcuts.
            const SCANCODE_V: u16 = 0x2F;
            enigo
                .raw(SCANCODE_V, Direction::Press)
                .map_err(|e| e.to_string())?;
            thread::sleep(Duration::from_millis(30));
            enigo
                .raw(SCANCODE_V, Direction::Release)
                .map_err(|e| e.to_string())?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            enigo
                .key(Key::Unicode('v'), Direction::Click)
                .map_err(|e| e.to_string())?;
        }

        thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));
        Ok(())
    });

    // Extra safety: try to ensure no modifiers remain logically held down.
    release_common_modifiers_best_effort(&mut enigo);
    result?;

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
    type_text_blocking_with_options(text, hit_enter, true)
}

pub fn type_text_blocking_with_options(
    text: &str,
    hit_enter: bool,
    preserve_clipboard: bool,
) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;

    // Save previous clipboard content (text only). If the previous clipboard isn't text,
    // don't try to "restore" it as an empty string.
    let previous: Option<String> = if preserve_clipboard {
        clipboard.get_text().ok()
    } else {
        None
    };

    // RAII restore guard so errors/early-returns still attempt to restore when safe.
    let mut restore_guard = ClipboardRestoreGuard::new(previous, text, preserve_clipboard);

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

    let result = with_pressed_key(&mut enigo, modifier, |enigo| {
        thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));

        #[cfg(target_os = "windows")]
        {
            // Physical 'V' key (scancode set 1). More reliable for modifier shortcuts.
            const SCANCODE_V: u16 = 0x2F;
            enigo
                .raw(SCANCODE_V, Direction::Press)
                .map_err(|e| e.to_string())?;
            thread::sleep(Duration::from_millis(30));
            enigo
                .raw(SCANCODE_V, Direction::Release)
                .map_err(|e| e.to_string())?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            enigo
                .key(Key::Unicode('v'), Direction::Click)
                .map_err(|e| e.to_string())?;
        }

        thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));
        Ok(())
    });

    // Extra safety: try to ensure no modifiers remain logically held down.
    release_common_modifiers_best_effort(&mut enigo);
    result?;

    restore_guard.mark_paste_sent();

    maybe_hit_enter(&mut enigo, hit_enter)?;

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
///
/// IMPORTANT: On macOS, Enigo (HIToolbox) operations must run on the main thread.
/// Use `probe_selected_text_via_copy_with_app(...)` when calling from background tasks.
#[cfg(desktop)]
pub fn probe_selected_text_via_copy(method: ContextGrabMethod) -> Result<Option<String>, String> {
    if method == ContextGrabMethod::None {
        log::debug!("Selection probe: disabled (method=None)");
        return Ok(None);
    }

    if method == ContextGrabMethod::ClipboardOnly {
        // Key-free probe: do not write a sentinel and do not inject any shortcuts.
        // Best-effort: only treat the clipboard as a "selection" when it changed since the last
        // clipboard-only probe.
        let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
        let current = clipboard
            .get_text()
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let Some(current) = current else {
            log::info!("Selection probe: clipboard_only => empty");
            return Ok(None);
        };

        let mut last = clipboard_only_last_text_lock()
            .lock()
            .map_err(|_| "Clipboard-only lock poisoned".to_string())?;

        if last.as_deref() == Some(current.as_str()) {
            log::info!("Selection probe: clipboard_only => unchanged");
            return Ok(None);
        }

        *last = Some(current.clone());
        log::info!(
            "Selection probe: clipboard_only => changed (len={})",
            current.chars().count()
        );
        return Ok(Some(current));
    }

    // Serialize with output injections so key events can't interleave.
    let _guard = output_injection_lock()
        .lock()
        .map_err(|_| "Output lock poisoned".to_string())?;

    log::info!("Selection probe: attempting copy (method={:?})", method);

    #[cfg(target_os = "windows")]
    {
        let fg = crate::windows_apps::get_foreground_process_path();
        log::info!(
            "Selection probe: foreground_process={}",
            fg.as_deref().map(basename_for_log).unwrap_or("<unknown>")
        );
    }

    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;

    // Save previous clipboard content (text only).
    // NOTE: If this is None, we avoid any technique that would overwrite a non-text clipboard.
    let previous: Option<String> = clipboard.get_text().ok();

    // Important: without any guard, if Ctrl+C doesn't actually copy (common in terminals),
    // reading the clipboard will just return whatever text was already there (stale).
    //
    // If we have a previous text clipboard, we temporarily write a unique sentinel value and
    // only accept a selection if the clipboard changes away from that sentinel.
    let mut sentinel: Option<String> = None;
    if previous.is_some() {
        let token = format!("__kolboo_selection_probe__{}", Uuid::new_v4());
        if set_clipboard_text_platform(&mut clipboard, &token, true).is_ok() {
            sentinel = Some(token);
        }
    }

    log::debug!(
        "Selection probe: sentinel_set={} (previous_clipboard_text={})",
        sentinel.is_some(),
        previous.is_some()
    );

    // Simulate a copy shortcut.
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    #[cfg(not(target_os = "windows"))]
    {
        let modifier = if cfg!(target_os = "macos") {
            Key::Meta
        } else {
            Key::Control
        };

        // Key sequence: press modifiers -> click 'c' -> release.

        // Give the OS a moment to finish processing any key-up events from the record-stop hotkey.
        // (Without this, the target app can miss the subsequent chord.)
        thread::sleep(Duration::from_millis(120));

        // Important: ensure we always release modifiers, even if something errors mid-injection.
        // Otherwise keys can appear "stuck" at the OS level.
        let injection_result: Result<(), String> =
            with_pressed_key(&mut enigo, modifier, |enigo| {
                enigo
                    .key(Key::Unicode('c'), Direction::Click)
                    .map_err(|e| e.to_string())?;
                thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS * 2));
                Ok(())
            });

        if let Err(e) = injection_result {
            log::warn!("Selection probe: key injection failed: {}", e);
            // Keep going: we'll still restore sentinel/clipboard state below.
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Key sequence: press modifiers -> click key -> release.

        // Give the OS a moment to finish processing any key-up events from the record-stop hotkey.
        // (Without this, the target app can miss the subsequent chord.)
        thread::sleep(Duration::from_millis(120));

        #[cfg(target_os = "windows")]
        match method {
            ContextGrabMethod::CtrlShiftC => log::info!("Selection probe: injecting Ctrl+Shift+C"),
            ContextGrabMethod::CtrlInsert => log::info!("Selection probe: injecting Ctrl+Insert"),
            _ => log::info!("Selection probe: injecting Ctrl+C"),
        }

        // Important: ensure we always release modifiers, even if something errors mid-injection.
        // Otherwise keys can appear "stuck" at the OS level.
        let injection_result: Result<(), String> = {
            // Scancode set 1:
            // - C: 0x2E
            const SCANCODE_C: u16 = 0x2E;

            match method {
                ContextGrabMethod::CtrlShiftC => {
                    with_pressed_key(&mut enigo, Key::Shift, |enigo| {
                        // Hold Shift first, then press Ctrl+<key>.
                        // NOTE: Some console hosts still treat this as a Ctrl+C cancel event; users
                        // can opt into Ctrl+Insert or None if this is unsafe in their shell.
                        with_pressed_key(enigo, Key::Control, |enigo| {
                            enigo
                                .raw(SCANCODE_C, Direction::Press)
                                .map_err(|e| e.to_string())?;
                            thread::sleep(Duration::from_millis(50));
                            enigo
                                .raw(SCANCODE_C, Direction::Release)
                                .map_err(|e| e.to_string())?;
                            thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS * 2));
                            Ok(())
                        })
                    })
                }
                ContextGrabMethod::CtrlInsert => {
                    with_pressed_key(&mut enigo, Key::Control, |enigo| {
                        // Use the semantic Insert key rather than a raw scancode here.
                        // Insert is an extended key on Windows; sending the wrong form can end up
                        // as a VT escape sequence in some terminals (e.g. showing up as `5~`).
                        enigo
                            .key(Key::Insert, Direction::Click)
                            .map_err(|e| e.to_string())?;
                        thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS * 2));
                        Ok(())
                    })
                }
                _ => with_pressed_key(&mut enigo, Key::Control, |enigo| {
                    enigo
                        .raw(SCANCODE_C, Direction::Press)
                        .map_err(|e| e.to_string())?;
                    thread::sleep(Duration::from_millis(50));
                    enigo
                        .raw(SCANCODE_C, Direction::Release)
                        .map_err(|e| e.to_string())?;
                    thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS * 2));
                    Ok(())
                }),
            }
        };

        {
            let fg = crate::windows_apps::get_foreground_process_path();
            log::debug!(
                "Selection probe: foreground_process_after_key={}",
                fg.as_deref().map(basename_for_log).unwrap_or("<unknown>")
            );
        }

        if let Err(e) = injection_result {
            log::warn!("Selection probe: key injection failed: {}", e);
            // Keep going: we'll still restore sentinel/clipboard state below.
        }
    }

    // Even on success, try to reset modifiers (best-effort) so we never leave keys "stuck".
    release_common_modifiers_best_effort(&mut enigo);

    // Wait briefly for clipboard to update and then read it.
    let mut clipboard_read_errors: u32 = 0;
    let mut saw_sentinel: u32 = 0;
    let mut saw_nonempty_text: u32 = 0;

    let mut poll_for_capture = |clipboard: &mut Clipboard| -> Option<String> {
        for _ in 0..SELECTION_PROBE_VERIFY_ATTEMPTS {
            thread::sleep(Duration::from_millis(SELECTION_PROBE_VERIFY_DELAY_MS));
            match clipboard.get_text() {
                Ok(current) => {
                    if current.trim().is_empty() {
                        continue;
                    }
                    saw_nonempty_text += 1;

                    // If we set a sentinel, only accept a capture if the clipboard changed away from it.
                    if sentinel.as_deref().is_some_and(|s| s == current) {
                        saw_sentinel += 1;
                        continue;
                    }

                    // If we didn't set a sentinel (e.g. previous clipboard wasn't readable as text),
                    // avoid treating an unchanged clipboard as a "capture".
                    if sentinel.is_none() && previous.as_deref().is_some_and(|p| p == current) {
                        continue;
                    }

                    return Some(current);
                }
                Err(_) => {
                    clipboard_read_errors += 1;
                    continue;
                }
            }
        }

        None
    };

    #[cfg(target_os = "windows")]
    let mut polls_run: u32 = 1;
    #[cfg(not(target_os = "windows"))]
    let polls_run: u32 = 1;
    #[cfg(target_os = "windows")]
    let mut captured: Option<String> = poll_for_capture(&mut clipboard);
    #[cfg(not(target_os = "windows"))]
    let captured: Option<String> = poll_for_capture(&mut clipboard);

    // Windows reliability: some apps respond to Ctrl+Insert more consistently than Ctrl+C.
    // Only do this fallback when the user selected Ctrl+C, to avoid surprising behavior.
    #[cfg(target_os = "windows")]
    if captured.is_none() && method == ContextGrabMethod::CtrlC {
        polls_run += 1;
        log::info!(
            "Selection probe: Ctrl+C produced no clipboard change; retrying with Ctrl+Insert"
        );

        let injection_result: Result<(), String> =
            with_pressed_key(&mut enigo, Key::Control, |enigo| {
                enigo
                    .key(Key::Insert, Direction::Click)
                    .map_err(|e| e.to_string())?;
                thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS * 2));
                Ok(())
            });

        if let Err(e) = injection_result {
            log::warn!("Selection probe: key injection failed: {}", e);
        }

        release_common_modifiers_best_effort(&mut enigo);
        captured = poll_for_capture(&mut clipboard);
    }

    match captured.as_deref() {
        Some(text) => {
            log::info!(
                "Selection probe: capture succeeded (len={}, waited_ms~{}, clipboard_read_errors={})",
                text.chars().count(),
                (polls_run as u64)
                    * (SELECTION_PROBE_VERIFY_ATTEMPTS as u64)
                    * SELECTION_PROBE_VERIFY_DELAY_MS,
                clipboard_read_errors
            );
        }
        None => {
            log::info!(
                "Selection probe: capture failed (no clipboard change detected; waited_ms~{}, sentinel_set={}, saw_sentinel_reads={}, saw_nonempty_reads={}, clipboard_read_errors={})",
                (polls_run as u64)
                    * (SELECTION_PROBE_VERIFY_ATTEMPTS as u64)
                    * SELECTION_PROBE_VERIFY_DELAY_MS,
                sentinel.is_some(),
                saw_sentinel,
                saw_nonempty_text,
                clipboard_read_errors
            );
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

    // Restore previous clipboard text (best-effort).
    //
    // - If we set a sentinel, we MUST attempt to restore (otherwise we leave the sentinel behind).
    // - Even when restoring, avoid clobbering if the user/app changed clipboard after our copy.
    if let Some(prev) = previous {
        let expected_current = captured.as_deref().or(sentinel.as_deref());

        let should_restore = match expected_current {
            Some(expected) => clipboard
                .get_text()
                .ok()
                .is_some_and(|current| current == expected),
            None => false,
        };

        if should_restore {
            // Avoid creating a duplicate clipboard-history entry for the restored content.
            let _ = set_clipboard_text_platform(&mut clipboard, &prev, true);
        } else if sentinel.is_some() {
            log::debug!("Selection probe: clipboard changed after copy; skipping restore");
        }
    }

    Ok(captured)
}

/// Probe selected text, ensuring platform constraints are respected.
///
/// - macOS: runs on the main thread via `AppHandle::run_on_main_thread`.
/// - other platforms: calls `probe_selected_text_via_copy` directly.
#[cfg(desktop)]
#[allow(dead_code)]
pub fn probe_selected_text_via_copy_with_app(
    _app: &AppHandle,
    method: ContextGrabMethod,
) -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = mpsc::channel::<Result<Option<String>, String>>();
        let app = _app.clone();

        app.run_on_main_thread(move || {
            let result = probe_selected_text_via_copy(method);
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;

        return rx.recv().map_err(|e| e.to_string())?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        probe_selected_text_via_copy(method)
    }
}
