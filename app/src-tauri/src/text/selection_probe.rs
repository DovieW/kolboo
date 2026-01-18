use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tauri::AppHandle;
use uuid::Uuid;

use crate::text::clipboard::{clipboard_only_last_text_lock, set_clipboard_text_platform};
use crate::text::inject::run_with_output_injection_lock;

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

    run_with_output_injection_lock(|| {
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
    })
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
