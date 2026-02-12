//! Diagnostics export: bundles logs, sanitized settings, and system info
//! into a zip file saved to the user's Downloads folder.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Local;
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Keys in `settings.json` that may contain credentials or sensitive data.
/// Their values are replaced with `"[REDACTED]"` before export.
const REDACTED_SETTINGS_KEYS: &[&str] = &["proxy_username", "proxy_password"];

/// Create a diagnostics zip in the Downloads folder.
///
/// Returns the path to the created zip file on success.
pub fn export_diagnostics_zip(app: &AppHandle) -> Result<PathBuf, String> {
    let download_dir = app
        .path()
        .download_dir()
        .map_err(|e| format!("Cannot resolve Downloads folder: {e}"))?;

    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let zip_name = format!("kolboo-diagnostics-{timestamp}.zip");
    let zip_path = download_dir.join(&zip_name);

    let file = fs::File::create(&zip_path)
        .map_err(|e| format!("Failed to create {}: {e}", zip_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(6));

    // ── 1. Log files ────────────────────────────────────────────────────
    if let Some(log_dir) = crate::tracing_init::log_dir() {
        add_directory_to_zip(&mut zip, log_dir, "logs", options)?;
    }

    // ── 2. Sanitized settings ───────────────────────────────────────────
    let settings_json = read_sanitized_settings(app);
    if let Some(json) = settings_json {
        zip.start_file("settings.json", options)
            .map_err(|e| format!("zip error: {e}"))?;
        zip.write_all(json.as_bytes())
            .map_err(|e| format!("zip write error: {e}"))?;
    }

    // ── 3. System info ──────────────────────────────────────────────────
    let info = collect_system_info(app);
    zip.start_file("system-info.txt", options)
        .map_err(|e| format!("zip error: {e}"))?;
    zip.write_all(info.as_bytes())
        .map_err(|e| format!("zip write error: {e}"))?;

    zip.finish().map_err(|e| format!("zip finish error: {e}"))?;

    log::info!("Diagnostics exported to {}", zip_path.display());

    Ok(zip_path)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Recursively add all files from `dir` into the zip under `prefix/`.
fn add_directory_to_zip<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    dir: &Path,
    prefix: &str,
    options: SimpleFileOptions,
) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Cannot read {}: {e}", dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("dir entry error: {e}"))?;
        let path = entry.path();
        let name = entry.file_name();
        let archive_name = format!("{}/{}", prefix, name.to_string_lossy());

        if path.is_file() {
            let content =
                fs::read(&path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
            zip.start_file(&archive_name, options)
                .map_err(|e| format!("zip error: {e}"))?;
            zip.write_all(&content)
                .map_err(|e| format!("zip write error: {e}"))?;
        } else if path.is_dir() {
            add_directory_to_zip(zip, &path, &archive_name, options)?;
        }
    }
    Ok(())
}

/// Read settings.json from the Tauri store, then strip sensitive keys.
fn read_sanitized_settings(app: &AppHandle) -> Option<String> {
    let store = app.store("settings.json").ok()?;
    // Reload to get the freshest version on disk
    let _ = store.reload();

    // Build a serde_json::Value from all store entries
    let entries = store.entries();
    let mut map = serde_json::Map::new();
    for (key, value) in entries {
        map.insert(key, value);
    }

    let mut value = serde_json::Value::Object(map);
    redact_sensitive_keys(&mut value);

    serde_json::to_string_pretty(&value).ok()
}

/// Recursively walk JSON and redact any key whose name matches the deny-list.
fn redact_sensitive_keys(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if REDACTED_SETTINGS_KEYS.contains(&key.as_str()) {
                    if let serde_json::Value::String(s) = val {
                        if !s.is_empty() {
                            *val = serde_json::Value::String("[REDACTED]".to_string());
                        }
                    }
                } else {
                    redact_sensitive_keys(val);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_sensitive_keys(item);
            }
        }
        _ => {}
    }
}

/// Gather basic system information for bug reports.
fn collect_system_info(app: &AppHandle) -> String {
    let version = app.package_info().version.to_string();
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let mut lines = vec![
        format!("Kolboo version: {version}"),
        format!("OS: {os}"),
        format!("Arch: {arch}"),
    ];

    // Windows version from registry (detailed build info)
    #[cfg(target_os = "windows")]
    {
        if let Some(win_ver) = windows_version_string() {
            lines.push(format!("Windows version: {win_ver}"));
        }
    }

    // App data directory
    if let Ok(data_dir) = crate::app_paths::app_data_dir(app) {
        lines.push(format!("App data dir: {}", data_dir.display()));
    }

    // Log directory
    if let Some(log_dir) = crate::tracing_init::log_dir() {
        lines.push(format!("Log dir: {}", log_dir.display()));
    }

    lines.push(format!(
        "Export timestamp: {}",
        Local::now().format("%Y-%m-%d %H:%M:%S %z")
    ));

    lines.join("\n")
}

/// Build an accurate Windows version string from the registry.
///
/// The `ProductName` value is notoriously wrong on Windows 11 (it still says
/// "Windows 10 ..."). Instead we read `CurrentBuildNumber` (>= 22000 → Win 11),
/// `EditionID`, `DisplayVersion`, and `UBR` to construct a string like:
///
///   `Windows 11 Professional 24H2 (Build 26100.3194)`
#[cfg(target_os = "windows")]
fn windows_version_string() -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_DWORD, RRF_RT_REG_SZ,
    };

    const SUB_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

    /// Read a REG_SZ value from the CurrentVersion key.
    fn read_reg_sz(sub_key: &str, value_name: &str) -> Option<String> {
        let sub_key_w: Vec<u16> = sub_key.encode_utf16().chain(std::iter::once(0)).collect();
        let value_w: Vec<u16> = value_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut buf = [0u16; 256];
        let mut size = (buf.len() * 2) as u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(sub_key_w.as_ptr()),
                PCWSTR(value_w.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        if status.is_err() {
            return None;
        }
        let len = (size as usize / 2).saturating_sub(1); // drop null terminator
        Some(String::from_utf16_lossy(&buf[..len]))
    }

    /// Read a REG_DWORD value from the CurrentVersion key.
    fn read_reg_dword(sub_key: &str, value_name: &str) -> Option<u32> {
        let sub_key_w: Vec<u16> = sub_key.encode_utf16().chain(std::iter::once(0)).collect();
        let value_w: Vec<u16> = value_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut val: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(sub_key_w.as_ptr()),
                PCWSTR(value_w.as_ptr()),
                RRF_RT_DWORD,
                None,
                Some((&mut val as *mut u32).cast()),
                Some(&mut size),
            )
        };
        if status.is_err() {
            return None;
        }
        Some(val)
    }

    let build_str = read_reg_sz(SUB_KEY, "CurrentBuildNumber")?;
    let build_num: u32 = build_str.parse().unwrap_or(0);
    let edition = read_reg_sz(SUB_KEY, "EditionID").unwrap_or_default();
    let display_ver = read_reg_sz(SUB_KEY, "DisplayVersion").unwrap_or_default();
    let ubr = read_reg_dword(SUB_KEY, "UBR");

    // Build >= 22000 → Windows 11, otherwise Windows 10
    let win_ver = if build_num >= 22000 { "11" } else { "10" };

    let full_build = match ubr {
        Some(u) => format!("{build_str}.{u}"),
        None => build_str,
    };

    let mut parts = vec![format!("Windows {win_ver}")];
    if !edition.is_empty() {
        parts.push(edition);
    }
    if !display_ver.is_empty() {
        parts.push(display_ver);
    }
    parts.push(format!("(Build {full_build})"));

    Some(parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_replaces_non_empty_password() {
        let mut value = serde_json::json!({
            "proxy_password": "s3cret",
            "proxy_username": "admin",
            "some_other_key": "keep-me",
        });
        redact_sensitive_keys(&mut value);

        assert_eq!(value["proxy_password"], "[REDACTED]");
        assert_eq!(value["proxy_username"], "[REDACTED]");
        assert_eq!(value["some_other_key"], "keep-me");
    }

    #[test]
    fn redact_preserves_empty_password() {
        let mut value = serde_json::json!({
            "proxy_password": "",
            "proxy_username": "",
        });
        redact_sensitive_keys(&mut value);

        assert_eq!(value["proxy_password"], "");
        assert_eq!(value["proxy_username"], "");
    }

    #[test]
    fn redact_handles_nested_objects() {
        let mut value = serde_json::json!({
            "network": {
                "proxy_password": "oops",
                "safe_key": 42
            }
        });
        redact_sensitive_keys(&mut value);

        assert_eq!(value["network"]["proxy_password"], "[REDACTED]");
        assert_eq!(value["network"]["safe_key"], 42);
    }

    #[test]
    fn redact_ignores_non_string_values() {
        let mut value = serde_json::json!({
            "proxy_password": 12345,
            "proxy_username": null,
        });
        redact_sensitive_keys(&mut value);

        // Non-string values are left as-is (only string secrets get redacted)
        assert_eq!(value["proxy_password"], 12345);
        assert!(value["proxy_username"].is_null());
    }
}
