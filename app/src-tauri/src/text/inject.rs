use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use crate::text::clipboard::{set_clipboard_text_with_barrier, ClipboardRestoreGuard};

/// Delay between keyboard key press and release events
const KEY_EVENT_DELAY_MS: u64 = 50;

/// Global lock to ensure we never run multiple output injections concurrently.
///
/// Without this, two overlapping "type/paste" operations can interleave key events and
/// produce dropped/mangled text in target applications.
static OUTPUT_INJECTION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn output_injection_lock() -> &'static Mutex<()> {
    OUTPUT_INJECTION_LOCK.get_or_init(|| Mutex::new(()))
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

pub fn output_injection_lock_handle() -> &'static Mutex<()> {
    output_injection_lock()
}

pub fn run_with_output_injection_lock<T>(
    work: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _guard = output_injection_lock()
        .lock()
        .map_err(|_| "Output lock poisoned".to_string())?;
    work()
}
