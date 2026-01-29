// Windows foreground process + window enumeration helpers.
//
// These are used for per-program prompt profiles.

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct ForegroundWindowCaptureTarget {
    /// The chosen window handle (HWND) to capture, stored as a raw pointer value (usize)
    /// so it can safely cross threads.
    #[serde(skip_serializing)]
    pub hwnd_raw: usize,
    pub process_path: String,
    /// True if the true foreground was Kolboo and we had to pick an external window.
    pub used_external_fallback: bool,
}

#[cfg(target_os = "windows")]
mod imp {
    use schemars::JsonSchema;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use windows::core::{BOOL, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId, IsWindowVisible,
    };

    use super::ForegroundWindowCaptureTarget;

    #[derive(Debug, Clone, serde::Serialize, JsonSchema)]
    pub struct OpenWindowInfo {
        pub title: String,
        pub process_path: String,
    }

    #[derive(Debug)]
    struct EnumWindowsState {
        include_titles: bool,
        windows: *mut Vec<OpenWindowInfo>,
    }

    fn query_process_path(pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

            // Large buffer to avoid truncation.
            let mut buf: Vec<u16> = vec![0; 4096];
            let mut size: u32 = buf.len() as u32;

            let ok = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut size,
            )
            .is_ok();

            let _ = CloseHandle(handle);

            if !ok || size == 0 {
                return None;
            }

            Some(String::from_utf16_lossy(&buf[..size as usize]))
        }
    }

    // When our always-on-top overlay windows are visible, Windows can briefly report our own
    // process as the foreground window. That breaks per-program profile matching.
    // We keep a short-lived memory of the last non-Kolboo foreground process and use it as a
    // fallback when the current foreground belongs to our process.
    fn last_external_foreground_cell() -> &'static Mutex<Option<(String, Instant)>> {
        static CELL: OnceLock<Mutex<Option<(String, Instant)>>> = OnceLock::new();
        CELL.get_or_init(|| Mutex::new(None))
    }

    fn record_external_foreground(path: &str) {
        let mut guard = last_external_foreground_cell().lock().unwrap();
        *guard = Some((path.to_string(), Instant::now()));
    }

    fn get_recent_external_foreground(max_age: Duration) -> Option<String> {
        let guard = last_external_foreground_cell().lock().unwrap();
        let (path, at) = guard.as_ref()?;
        if at.elapsed() <= max_age {
            Some(path.clone())
        } else {
            None
        }
    }

    fn find_external_process_path_by_z_order(current_pid: u32) -> Option<String> {
        #[derive(Debug)]
        struct State {
            current_pid: u32,
            found: Option<String>,
        }

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            // Safety: caller passes a valid mutable State pointer via LPARAM.
            let state = unsafe { &mut *(lparam.0 as *mut State) };

            unsafe {
                if !IsWindowVisible(hwnd).as_bool() {
                    return BOOL(1);
                }

                let title_len = GetWindowTextLengthW(hwnd);
                if title_len == 0 {
                    return BOOL(1);
                }

                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                if pid == 0 {
                    return BOOL(1);
                }
                if pid == state.current_pid {
                    return BOOL(1);
                }

                let Some(process_path) = query_process_path(pid) else {
                    return BOOL(1);
                };

                state.found = Some(process_path);
                // Stop enumeration once we found the first plausible external window.
                BOOL(0)
            }
        }

        let mut state = State {
            current_pid,
            found: None,
        };

        unsafe {
            let _ = EnumWindows(Some(enum_proc), LPARAM((&mut state as *mut _) as isize));
        }

        state.found
    }

    fn find_external_window_by_z_order(current_pid: u32) -> Option<(HWND, String)> {
        #[derive(Debug)]
        struct State {
            current_pid: u32,
            found: Option<(HWND, String)>,
        }

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            // Safety: caller passes a valid mutable State pointer via LPARAM.
            let state = unsafe { &mut *(lparam.0 as *mut State) };

            unsafe {
                if !IsWindowVisible(hwnd).as_bool() {
                    return BOOL(1);
                }

                let title_len = GetWindowTextLengthW(hwnd);
                if title_len == 0 {
                    return BOOL(1);
                }

                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                if pid == 0 {
                    return BOOL(1);
                }
                if pid == state.current_pid {
                    return BOOL(1);
                }

                let Some(process_path) = query_process_path(pid) else {
                    return BOOL(1);
                };

                state.found = Some((hwnd, process_path));
                // Stop enumeration once we found the first plausible external window.
                BOOL(0)
            }
        }

        let mut state = State {
            current_pid,
            found: None,
        };

        unsafe {
            let _ = EnumWindows(Some(enum_proc), LPARAM((&mut state as *mut _) as isize));
        }

        state.found
    }

    pub fn get_foreground_window_capture_target() -> Option<ForegroundWindowCaptureTarget> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }

            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return None;
            }

            let current_pid = GetCurrentProcessId();
            if pid == current_pid {
                // If Kolboo is foreground, pick a plausible external window by z-order.
                let (external_hwnd, external_path) = find_external_window_by_z_order(current_pid)?;
                record_external_foreground(&external_path);
                return Some(ForegroundWindowCaptureTarget {
                    hwnd_raw: external_hwnd.0 as usize,
                    process_path: external_path,
                    used_external_fallback: true,
                });
            }

            let path = query_process_path(pid)?;
            record_external_foreground(&path);
            Some(ForegroundWindowCaptureTarget {
                hwnd_raw: hwnd.0 as usize,
                process_path: path,
                used_external_fallback: false,
            })
        }
    }

    pub fn get_foreground_process_path() -> Option<String> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }

            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return None;
            }

            let current_pid = GetCurrentProcessId();
            if pid == current_pid {
                // Prefer the most recent external foreground.
                if let Some(path) = get_recent_external_foreground(Duration::from_secs(5)) {
                    log::debug!(
                        "[windows_apps] Foreground is Kolboo pid {}; using recent external foreground: {}",
                        current_pid,
                        crate::app_shared::basename_for_log(&path)
                    );
                    return Some(path);
                }

                // If we don't have a recent external sample yet, try to recover a plausible
                // external "active" app by scanning top-level windows (best-effort).
                let path = find_external_process_path_by_z_order(current_pid)?;
                log::debug!(
                    "[windows_apps] Foreground is Kolboo pid {}; recovered external foreground from z-order: {}",
                    current_pid,
                    crate::app_shared::basename_for_log(&path)
                );
                record_external_foreground(&path);
                return Some(path);
            }

            let path = query_process_path(pid)?;
            record_external_foreground(&path);
            Some(path)
        }
    }

    pub fn list_open_windows(include_titles: bool) -> Vec<OpenWindowInfo> {
        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            // Safety: caller passes a valid mutable EnumWindowsState pointer via LPARAM.
            let state = unsafe { &mut *(lparam.0 as *mut EnumWindowsState) };
            let windows = unsafe { &mut *state.windows };

            unsafe {
                if !IsWindowVisible(hwnd).as_bool() {
                    return BOOL(1);
                }

                let title_len = GetWindowTextLengthW(hwnd);
                if title_len == 0 {
                    return BOOL(1);
                }

                // Title collection can expose sensitive info (doc names, URLs, chat previews).
                // For a good privacy default, allow callers to opt out.
                let title = if state.include_titles {
                    let mut title_buf: Vec<u16> = vec![0; (title_len as usize) + 1];
                    let copied = GetWindowTextW(hwnd, &mut title_buf);
                    if copied == 0 {
                        return BOOL(1);
                    }

                    let title = String::from_utf16_lossy(&title_buf[..copied as usize])
                        .trim()
                        .to_string();
                    if title.is_empty() {
                        return BOOL(1);
                    }
                    title
                } else {
                    String::new()
                };

                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                if pid == 0 {
                    return BOOL(1);
                }

                let Some(process_path) = query_process_path(pid) else {
                    return BOOL(1);
                };

                windows.push(OpenWindowInfo {
                    title,
                    process_path,
                });
                BOOL(1)
            }
        }

        let mut windows: Vec<OpenWindowInfo> = Vec::new();
        let mut state = EnumWindowsState {
            include_titles,
            windows: (&mut windows as *mut Vec<OpenWindowInfo>),
        };

        unsafe {
            let _ = EnumWindows(Some(enum_proc), LPARAM((&mut state as *mut _) as isize));
        }

        windows
    }
}

#[cfg(target_os = "windows")]
pub use imp::{
    get_foreground_process_path, get_foreground_window_capture_target, list_open_windows,
    OpenWindowInfo,
};

#[cfg(not(target_os = "windows"))]
mod imp_stub {
    use schemars::JsonSchema;

    #[derive(Debug, Clone, serde::Serialize, JsonSchema)]
    pub struct OpenWindowInfo {
        pub title: String,
        pub process_path: String,
    }

    pub fn get_foreground_process_path() -> Option<String> {
        None
    }

    pub fn list_open_windows(_include_titles: bool) -> Vec<OpenWindowInfo> {
        Vec::new()
    }
}

#[cfg(not(target_os = "windows"))]
pub use imp_stub::{get_foreground_process_path, list_open_windows, OpenWindowInfo};
