use arboard::Clipboard;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

/// Delay after clipboard operations to ensure system stability
const CLIPBOARD_STABILIZATION_DELAY_MS: u64 = 50;

/// How many times we try to confirm the clipboard contains our injected text.
///
/// This mitigates a race where we press Ctrl+V before the clipboard update is fully visible
/// to the target application (or before Windows has finished committing the clipboard write).
const CLIPBOARD_VERIFY_ATTEMPTS: u32 = 10;

/// Delay between clipboard verification attempts.
const CLIPBOARD_VERIFY_DELAY_MS: u64 = 20;

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

/// Snapshot used by `ContextGrabMethod::ClipboardOnly` to avoid repeatedly treating an
/// unchanged clipboard as a selection capture.
#[cfg(desktop)]
static CLIPBOARD_ONLY_LAST_TEXT: OnceLock<Mutex<Option<String>>> = OnceLock::new();

#[cfg(desktop)]
pub fn clipboard_only_last_text_lock() -> &'static Mutex<Option<String>> {
    CLIPBOARD_ONLY_LAST_TEXT.get_or_init(|| Mutex::new(None))
}

pub fn set_clipboard_text_with_barrier(
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

pub fn set_clipboard_text_platform(
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

pub struct ClipboardRestoreGuard {
    enabled: bool,
    previous: Option<String>,
    injected: String,
    paste_sent: bool,
}

impl ClipboardRestoreGuard {
    pub fn new(previous: Option<String>, injected: &str, enabled: bool) -> Self {
        Self {
            enabled,
            previous,
            injected: injected.to_string(),
            paste_sent: false,
        }
    }

    pub fn mark_paste_sent(&mut self) {
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
