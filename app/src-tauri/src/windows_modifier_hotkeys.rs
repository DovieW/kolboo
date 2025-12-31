// Windows-only support for modifier-only hotkeys (e.g. Right Alt by itself).
//
// Why this exists:
// - Tauri global shortcuts (and OS-level hotkey APIs) generally require a non-modifier key.
// - Users may want a single modifier key (like Right Alt) as the entire hotkey.
//
// Implementation:
// - Install a low-level keyboard hook (WH_KEYBOARD_LL).
// - Detect Right Alt (VK_RMENU) key down/up.
// - Forward events to the app's handler (crate::handle_modifier_key_event).
// - Allow temporarily disabling while the UI is recording a new hotkey.

#![cfg(target_os = "windows")]

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::AppHandle;

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
    KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

// Virtual key codes (avoid adding extra windows crate feature flags)
const VK_RMENU: u32 = 0xA5; // Right Alt

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(true);

/// Enable/disable modifier-only hotkeys.
///
/// This is used to prevent interference while the UI is capturing a new hotkey.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::SeqCst);
}

/// Start the low-level keyboard hook thread.
/// Safe to call multiple times; only starts once.
pub fn init(app: AppHandle) {
    if APP_HANDLE.set(app).is_err() {
        // Already initialized.
        return;
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
                    return;
                }
            };

            log::info!(
                "Installed modifier-only keyboard hook (WH_KEYBOARD_LL): {:?}",
                hook
            );

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

        if vk == VK_RMENU {
            if let Some(app) = APP_HANDLE.get() {
                let app = app.clone();
                // Keep the hook callback fast: defer work to the async runtime.
                tauri::async_runtime::spawn(async move {
                    crate::handle_modifier_key_event(&app, "AltRight", is_down);
                });
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}
