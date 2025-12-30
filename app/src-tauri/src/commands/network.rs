use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SystemProxyInfo {
    pub env_http_proxy: Option<String>,
    pub env_https_proxy: Option<String>,
    pub env_no_proxy: Option<String>,

    /// Best-effort OS proxy settings (Windows only for now).
    pub windows_internet_settings: Option<WindowsInternetProxySettings>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WindowsInternetProxySettings {
    pub proxy_enable: Option<bool>,
    pub proxy_server: Option<String>,
    pub proxy_override: Option<String>,
    pub auto_config_url: Option<String>,
}

fn get_env_proxy_value(names: &[&str]) -> Option<String> {
    for name in names {
        if let Ok(v) = std::env::var(name) {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn to_wide(s: &str) -> Vec<u16> {
    use std::os::windows::prelude::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(target_os = "windows")]
fn reg_get_string(hkey: windows::Win32::System::Registry::HKEY, subkey: &str, value: &str) -> Option<String> {
    use windows::Win32::System::Registry::{RegGetValueW, RRF_RT_REG_SZ};
    use windows::core::PCWSTR;

    let subkey_w = to_wide(subkey);
    let value_w = to_wide(value);

    let mut data_len: u32 = 0;
    let status = unsafe {
        RegGetValueW(
            hkey,
            PCWSTR(subkey_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut data_len),
        )
    };

    if status.0 != 0 || data_len == 0 {
        return None;
    }

    let mut buf: Vec<u16> = vec![0; (data_len as usize + 1) / 2];
    let status = unsafe {
        RegGetValueW(
            hkey,
            PCWSTR(subkey_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut std::ffi::c_void),
            Some(&mut data_len),
        )
    };

    if status.0 != 0 {
        return None;
    }

    let len_u16 = (data_len as usize) / 2;
    let s = String::from_utf16_lossy(&buf[..len_u16]);
    let s = s.trim_end_matches('\0').trim();
    if s.is_empty() { None } else { Some(s.to_string()) }
}

#[cfg(target_os = "windows")]
fn reg_get_dword(hkey: windows::Win32::System::Registry::HKEY, subkey: &str, value: &str) -> Option<u32> {
    use windows::Win32::System::Registry::{RegGetValueW, RRF_RT_REG_DWORD};
    use windows::core::PCWSTR;

    let subkey_w = to_wide(subkey);
    let value_w = to_wide(value);

    let mut out: u32 = 0;
    let mut data_len: u32 = std::mem::size_of::<u32>() as u32;

    let status = unsafe {
        RegGetValueW(
            hkey,
            PCWSTR(subkey_w.as_ptr()),
            PCWSTR(value_w.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut out as *mut u32) as *mut std::ffi::c_void),
            Some(&mut data_len),
        )
    };

    if status.0 != 0 {
        return None;
    }

    Some(out)
}

#[cfg(target_os = "windows")]
fn get_windows_internet_proxy_settings() -> Option<WindowsInternetProxySettings> {
    use windows::Win32::System::Registry::HKEY_CURRENT_USER;

    const SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings";

    let proxy_enable = reg_get_dword(HKEY_CURRENT_USER, SUBKEY, "ProxyEnable")
        .map(|v| v != 0);
    let proxy_server = reg_get_string(HKEY_CURRENT_USER, SUBKEY, "ProxyServer");
    let proxy_override = reg_get_string(HKEY_CURRENT_USER, SUBKEY, "ProxyOverride");
    let auto_config_url = reg_get_string(HKEY_CURRENT_USER, SUBKEY, "AutoConfigURL");

    if proxy_enable.is_none()
        && proxy_server.is_none()
        && proxy_override.is_none()
        && auto_config_url.is_none()
    {
        return None;
    }

    Some(WindowsInternetProxySettings {
        proxy_enable,
        proxy_server,
        proxy_override,
        auto_config_url,
    })
}

#[cfg(not(target_os = "windows"))]
fn get_windows_internet_proxy_settings() -> Option<WindowsInternetProxySettings> {
    None
}

#[tauri::command]
pub fn get_system_proxy_info() -> SystemProxyInfo {
    // Best-effort env lookup (both cases; common on Unix and Windows).
    // Reqwest supports env vars like HTTP_PROXY/HTTPS_PROXY/NO_PROXY.
    let env_http_proxy = get_env_proxy_value(&["HTTP_PROXY", "http_proxy"]);
    let env_https_proxy = get_env_proxy_value(&["HTTPS_PROXY", "https_proxy"]);
    let env_no_proxy = get_env_proxy_value(&["NO_PROXY", "no_proxy"]);

    SystemProxyInfo {
        env_http_proxy,
        env_https_proxy,
        env_no_proxy,
        windows_internet_settings: get_windows_internet_proxy_settings(),
    }
}
