// Windows-only support for modifier-only hotkeys (e.g. Right Alt by itself).
//
// Why this exists:
// - Tauri global shortcuts (and OS-level hotkey APIs) generally require a non-modifier key.
// - Users may want a single modifier key (like Right Alt) as the entire hotkey.
//
// Implementation:
// - Install a low-level keyboard hook (WH_KEYBOARD_LL).
// - Detect Right Alt (VK_RMENU) key down/up.
// - Forward events to the app's handler (crate::shortcuts::handle_modifier_key_event).
// - Allow temporarily disabling while the UI is recording a new hotkey.

use std::sync::atomic::AtomicU32;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use tauri::AppHandle;
use tauri::Emitter;

use crate::events;
use crate::SystemEvent;

use tauri_plugin_store::StoreExt;

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
    KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

// Virtual key codes (avoid adding extra windows crate feature flags)
const VK_MENU: u32 = 0x12; // Alt (generic)
const VK_SHIFT: u32 = 0x10;
const VK_CONTROL: u32 = 0x11;
const VK_LSHIFT: u32 = 0xA0;
const VK_RSHIFT: u32 = 0xA1;
const VK_LCONTROL: u32 = 0xA2;
const VK_RCONTROL: u32 = 0xA3;
const VK_LMENU: u32 = 0xA4; // Left Alt
const VK_RMENU: u32 = 0xA5; // Right Alt
const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;

// Low-level hook flags (KBDLLHOOKSTRUCT.flags)
const LLKHF_EXTENDED: u32 = 0x01;
const LLKHF_INJECTED: u32 = 0x10;
const LLKHF_ALTDOWN: u32 = 0x20;
const LLKHF_UP: u32 = 0x80;

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(true);

// Runtime flag: whether Copilot hotkey interception is enabled.
//
// When enabled, we attempt to interpret certain Win-key chords as the physical Copilot key
// (e.g. Win+C or Win+Shift+F23, depending on keyboard/firmware) and suppress OS behavior.
static COPILOT_HOTKEY_ENABLED: AtomicBool = AtomicBool::new(false);

// Tracks whether we're currently handling a Win+C chord as "Copilot".
// Used to suppress the Win key-up event to avoid triggering the Start menu.
static COPILOT_CHORD_ACTIVE: AtomicBool = AtomicBool::new(false);

// Whether we should suppress the Win key-up event for the current Copilot chord.
// We only do this for Win+C to avoid the Start menu opening when C is swallowed.
static COPILOT_SUPPRESS_WIN_UP: AtomicBool = AtomicBool::new(false);

// Timestamp (KBDLLHOOKSTRUCT.time, milliseconds) of the most recent Win key-down.
static LAST_WIN_DOWN_TIME: AtomicU32 = AtomicU32::new(0);

// Runtime flag: whether to emit hotkey debug events.
// IMPORTANT: Do not read settings.json on every keypress; the store plugin can
// have a stale in-memory view relative to the JS side. The frontend updates
// this flag via a command when the user toggles the setting.
static HOTKEY_DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

// Runtime flag: whether RightAlt should be treated as an app hotkey and swallowed so
// other apps don't see it (e.g. to prevent menu-focus behavior in browsers).
static ALT_RIGHT_HOTKEY_ENABLED: AtomicBool = AtomicBool::new(false);

// Tracks whether Right Alt is currently held (for AltGr / pressed-alone gating).
static ALT_RIGHT_HELD: AtomicBool = AtomicBool::new(false);
// Tracks whether any non-modifier key was pressed while Right Alt was held.
static ALT_RIGHT_USED_WITH_OTHER_KEY: AtomicBool = AtomicBool::new(false);

static LWIN_HELD: AtomicBool = AtomicBool::new(false);
static RWIN_HELD: AtomicBool = AtomicBool::new(false);

fn win_held() -> bool {
    LWIN_HELD.load(Ordering::Relaxed) || RWIN_HELD.load(Ordering::Relaxed)
}

fn recent_win_activity(now_ms: u32) -> bool {
    let last = LAST_WIN_DOWN_TIME.load(Ordering::Relaxed);
    last != 0 && now_ms.saturating_sub(last) <= 300
}

fn is_modifier_vk(vk: u32) -> bool {
    matches!(
        vk,
        VK_MENU
            | VK_LMENU
            | VK_RMENU
            | VK_CONTROL
            | VK_LCONTROL
            | VK_RCONTROL
            | VK_SHIFT
            | VK_LSHIFT
            | VK_RSHIFT
            | VK_LWIN
            | VK_RWIN
    )
}

/// Best-effort detection of physical Right Alt.
///
/// Notes:
/// - On most systems, Right Alt comes through as VK_RMENU.
/// - On some systems and input stacks, it can come through as VK_MENU (generic Alt)
///   with LLKHF_EXTENDED set, which corresponds to the right-side Alt key.
fn classify_right_alt(kb: &KBDLLHOOKSTRUCT) -> Option<&'static str> {
    let vk = kb.vkCode;
    if vk == VK_RMENU {
        return Some("VK_RMENU");
    }

    let flags: u32 = kb.flags.0;
    if vk == VK_MENU && (flags & LLKHF_EXTENDED) != 0 {
        return Some("VK_MENU+LLKHF_EXTENDED");
    }

    None
}

fn hotkey_debug_enabled(_app: &AppHandle) -> bool {
    HOTKEY_DEBUG_ENABLED.load(Ordering::Relaxed)
}

/// Update runtime hotkey debug flag.
///
/// This is called from a Tauri command when the user toggles the setting.
pub fn set_hotkey_debug_enabled(enabled: bool) {
    HOTKEY_DEBUG_ENABLED.store(enabled, Ordering::SeqCst);
}

/// Enable/disable AltRight hotkey interception.
///
/// When enabled, the hook will swallow the RightAlt key events (down/up) so foreground
/// apps won't treat Alt as a menu accelerator.
#[allow(dead_code)]
pub fn set_alt_right_hotkey_enabled(enabled: bool) {
    ALT_RIGHT_HOTKEY_ENABLED.store(enabled, Ordering::SeqCst);

    if let Some(app) = APP_HANDLE.get() {
        if hotkey_debug_enabled(app) {
            emit_hotkey_debug_event(
                app,
                "AltRight hotkey interception enabled changed",
                Some(format!("{enabled}")),
            );
        }
    }
}

/// Enable/disable Copilot hotkey interception.
///
/// When enabled, the hook treats the Win+C chord as a single "Copilot" hotkey and
/// suppresses the OS Copilot behavior.
pub fn set_copilot_hotkey_enabled(enabled: bool) {
    COPILOT_HOTKEY_ENABLED.store(enabled, Ordering::SeqCst);

    // Reset chord tracking when turning off.
    if !enabled {
        COPILOT_CHORD_ACTIVE.store(false, Ordering::SeqCst);
        COPILOT_SUPPRESS_WIN_UP.store(false, Ordering::SeqCst);
    }

    log::debug!(
        "Copilot hotkey interception enabled changed: {}",
        if enabled { "true" } else { "false" }
    );

    if let Some(app) = APP_HANDLE.get() {
        if hotkey_debug_enabled(app) {
            emit_hotkey_debug_event(
                app,
                "Copilot hotkey interception enabled changed",
                Some(format!("{enabled}")),
            );
        }
    }
}

/// Read the current runtime hotkey debug flag.
///
/// This is used by other backend code paths (e.g. the modifier-only handler in
/// `lib.rs`) to gate debug `system-event`s consistently.
#[allow(dead_code)]
pub fn hotkey_debug_runtime_enabled() -> bool {
    HOTKEY_DEBUG_ENABLED.load(Ordering::Relaxed)
}

fn emit_hotkey_debug_event(app: &AppHandle, message: &str, details: Option<String>) {
    let event = SystemEvent {
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: "debug".to_string(),
        message: message.to_string(),
        details,
    };

    let _ = app.emit(events::EVENT_SYSTEM_EVENT, event);
}

/// Enable/disable modifier-only hotkeys.
///
/// This is used to prevent interference while the UI is capturing a new hotkey.
pub fn set_enabled(enabled: bool) {
    let prev = ENABLED.swap(enabled, Ordering::SeqCst);

    if prev != enabled {
        if let Some(app) = APP_HANDLE.get() {
            if hotkey_debug_enabled(app) {
                emit_hotkey_debug_event(
                    app,
                    "Modifier-only hotkey hook enabled changed",
                    Some(format!("{prev} -> {enabled}")),
                );
            }
        }
    }
}

/// Start the low-level keyboard hook thread.
/// Safe to call multiple times; only starts once.
pub fn init(app: AppHandle) {
    // Best-effort: seed the runtime flag from persisted settings once.
    // (Subsequent updates come via set_hotkey_debug_enabled.)
    let initial_debug = app
        .store("settings.json")
        .ok()
        .and_then(|s| s.get("hotkey_debug_enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    HOTKEY_DEBUG_ENABLED.store(initial_debug, Ordering::SeqCst);

    if APP_HANDLE.set(app).is_err() {
        // Already initialized.
        return;
    }

    // One-time marker to confirm the hook thread startup path is reached.
    if let Some(app) = APP_HANDLE.get() {
        if hotkey_debug_enabled(app) {
            emit_hotkey_debug_event(app, "Starting modifier-only hotkey hook thread", None);
        }
    }

    std::thread::Builder::new()
        .name("kolboo-modifier-hotkeys".to_string())
        .spawn(|| unsafe {
            let hinstance: Option<HINSTANCE> = GetModuleHandleW(None)
                .ok()
                .map(|hmodule| HINSTANCE(hmodule.0));

            let hook = match SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(low_level_keyboard_proc),
                hinstance,
                0,
            ) {
                Ok(h) => h,
                Err(e) => {
                    log::warn!(
                        "Failed to install WH_KEYBOARD_LL hook for modifier-only hotkeys: {e}"
                    );

                    if let Some(app) = APP_HANDLE.get() {
                        if hotkey_debug_enabled(app) {
                            emit_hotkey_debug_event(
                                app,
                                "Failed to install modifier-only hotkey hook (WH_KEYBOARD_LL)",
                                Some(format!("{e}")),
                            );
                        }
                    }
                    return;
                }
            };

            log::info!(
                "Installed modifier-only keyboard hook (WH_KEYBOARD_LL): {:?}",
                hook
            );

            if let Some(app) = APP_HANDLE.get() {
                if hotkey_debug_enabled(app) {
                    emit_hotkey_debug_event(
                        app,
                        "Installed modifier-only hotkey hook (WH_KEYBOARD_LL)",
                        Some(format!("hook={hook:?}")),
                    );
                }
            }

            // Message loop required for the hook to stay alive.
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        })
        .map_err(|e| log::warn!("Failed to start modifier hotkey thread: {e}"))
        .ok();
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    if !ENABLED.load(Ordering::Relaxed) {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let msg = wparam.0 as u32;
    let is_down = matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN);
    let is_up = matches!(msg, WM_KEYUP | WM_SYSKEYUP);

    if is_down || is_up {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = kb.vkCode;
        let now_ms = kb.time;
        let flags: u32 = kb.flags.0;

        // Track Win key state (used for Win+C -> Copilot chord detection).
        if vk == VK_LWIN {
            LWIN_HELD.store(is_down, Ordering::Relaxed);
            if is_down {
                LAST_WIN_DOWN_TIME.store(now_ms, Ordering::Relaxed);
            }
        } else if vk == VK_RWIN {
            RWIN_HELD.store(is_down, Ordering::Relaxed);
            if is_down {
                LAST_WIN_DOWN_TIME.store(now_ms, Ordering::Relaxed);
            }
        }

        // If we previously handled a Copilot chord, we may need to suppress the Win key release
        // to avoid the Start menu opening. Only do this for mappings where we swallowed the
        // non-modifier key (e.g. Win+C). For Win+Shift+F23, Shift is still delivered and should
        // prevent Start-menu behavior, so swallowing Win-up can cause "stuck Win" in apps.
        if COPILOT_CHORD_ACTIVE.load(Ordering::Relaxed) && is_up && (vk == VK_LWIN || vk == VK_RWIN)
        {
            let suppress = COPILOT_SUPPRESS_WIN_UP.load(Ordering::Relaxed);

            // Update chord state now that Win has been released.
            if !win_held() {
                COPILOT_CHORD_ACTIVE.store(false, Ordering::Relaxed);
                COPILOT_SUPPRESS_WIN_UP.store(false, Ordering::Relaxed);
            }

            if suppress {
                return LRESULT(1);
            }
        }

        // Heuristic Copilot key handling (Windows).
        //
        // Different keyboards/firmware map the physical Copilot key differently. We support:
        // - Win+C (common software shortcut)
        // - Win+Shift+F23 (observed on at least one Copilot-key keyboard)
        //
        // IMPORTANT: This is gated by a runtime flag so we don't steal system shortcuts unless
        // the user configured a Copilot hotkey.
        const VK_C: u32 = 0x43;
        const VK_F23: u32 = 0x86;
        let is_copilot_vk = matches!(vk, VK_C | VK_F23);
        if COPILOT_HOTKEY_ENABLED.load(Ordering::Relaxed)
            && is_copilot_vk
            && (win_held() || recent_win_activity(now_ms))
        {
            if is_down {
                COPILOT_CHORD_ACTIVE.store(true, Ordering::Relaxed);
                // Only suppress Win-up for Win+C. For Win+Shift+F23, do not suppress Win-up.
                COPILOT_SUPPRESS_WIN_UP.store(vk == VK_C, Ordering::Relaxed);
            }

            if let Some(app) = APP_HANDLE.get() {
                let app = app.clone();

                if hotkey_debug_enabled(&app) {
                    let kind = if vk == VK_C {
                        "Win+C"
                    } else {
                        // Observed on at least one Copilot-key keyboard
                        "Win+Shift+F23"
                    };
                    emit_hotkey_debug_event(
                        &app,
                        if is_down {
                            "Hotkey debug: Copilot down"
                        } else {
                            "Hotkey debug: Copilot up"
                        },
                        Some(format!(
                            "kind={kind} msg=0x{msg:X} vk=0x{vk:X} scan=0x{:X} flags=0x{flags:X} lwinHeld={} rwinHeld={} chordActive={} suppressWinUp={}",
                            kb.scanCode,
                            LWIN_HELD.load(Ordering::Relaxed),
                            RWIN_HELD.load(Ordering::Relaxed),
                            COPILOT_CHORD_ACTIVE.load(Ordering::Relaxed),
                            COPILOT_SUPPRESS_WIN_UP.load(Ordering::Relaxed)
                        )),
                    );
                }

                tauri::async_runtime::spawn(async move {
                    crate::shortcuts::handle_modifier_key_event(&app, "Copilot", is_down, false);
                });
            }

            // Swallow the key event so Windows Copilot (and/or other Win+C handlers) don't fire.
            return LRESULT(1);
        }

        // When debugging, emit raw Alt key observations even if we don't classify them
        // as Right Alt, so we can diagnose weird keyboard layouts / drivers.
        if let Some(app) = APP_HANDLE.get() {
            if hotkey_debug_enabled(app) && (vk == VK_MENU || vk == VK_RMENU) {
                let flags: u32 = kb.flags.0;
                let kind = if is_down { "down" } else { "up" };
                emit_hotkey_debug_event(
                    app,
                    "Raw Alt key event observed",
                    Some(format!(
                        "vk=0x{vk:02X} kind={kind} flags=0x{flags:02X} extended={} injected={} altdown={} upFlag={} scanCode=0x{:X} time={}",
                        (flags & LLKHF_EXTENDED) != 0,
                        (flags & LLKHF_INJECTED) != 0,
                        (flags & LLKHF_ALTDOWN) != 0,
                        (flags & LLKHF_UP) != 0,
                        kb.scanCode,
                        kb.time
                    )),
                );
            }
        }

        // If Right Alt is held and we see a non-modifier key press, consider this an AltGr-style
        // key chord (typing) and suppress release-triggered actions.
        if is_down && ALT_RIGHT_HELD.load(Ordering::Relaxed) && !is_modifier_vk(vk) {
            ALT_RIGHT_USED_WITH_OTHER_KEY.store(true, Ordering::Relaxed);
        }

        if let Some(classification) = classify_right_alt(kb) {
            // Track held state for pressed-alone gating.
            if is_down {
                ALT_RIGHT_HELD.store(true, Ordering::Relaxed);
                ALT_RIGHT_USED_WITH_OTHER_KEY.store(false, Ordering::Relaxed);
            }

            let suppressed = if is_up {
                let used_with_other = ALT_RIGHT_USED_WITH_OTHER_KEY.load(Ordering::Relaxed);
                ALT_RIGHT_HELD.store(false, Ordering::Relaxed);
                used_with_other
            } else {
                false
            };

            if let Some(app) = APP_HANDLE.get() {
                let app = app.clone();

                // Optional diagnostics in final builds: emit a compact event to the in-app
                // System Events panel.
                if hotkey_debug_enabled(&app) {
                    let flags: u32 = kb.flags.0;
                    let details = format!(
                        "msg=0x{:X} vk=0x{:X} scan=0x{:X} flags=0x{:X} extended={} injected={} altDown={} upFlag={} held={} usedWithOther={} classification={} suppressed={}",
                        msg,
                        vk,
                        kb.scanCode,
                        flags,
                        (flags & LLKHF_EXTENDED) != 0,
                        (flags & LLKHF_INJECTED) != 0,
                        (flags & LLKHF_ALTDOWN) != 0,
                        (flags & LLKHF_UP) != 0,
                        ALT_RIGHT_HELD.load(Ordering::Relaxed),
                        ALT_RIGHT_USED_WITH_OTHER_KEY.load(Ordering::Relaxed),
                        classification,
                        suppressed
                    );
                    emit_hotkey_debug_event(
                        &app,
                        if is_down {
                            "Hotkey debug: RightAlt down"
                        } else {
                            "Hotkey debug: RightAlt up"
                        },
                        Some(details),
                    );
                }

                // Keep the hook callback fast: defer work to the async runtime.
                tauri::async_runtime::spawn(async move {
                    crate::shortcuts::handle_modifier_key_event(
                        &app, "AltRight", is_down, suppressed,
                    );
                });
            }

            // If AltRight is configured as a hotkey, swallow the key events so other apps
            // (e.g. Chrome) don't treat Alt as a menu accelerator / focus-stealer.
            if ALT_RIGHT_HOTKEY_ENABLED.load(Ordering::Relaxed) {
                return LRESULT(1);
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}
