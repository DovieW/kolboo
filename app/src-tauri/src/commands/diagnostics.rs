//! Diagnostics export: bundles typed runtime records, safe settings, and system info
//! into a zip file saved to the user's Downloads folder.

use std::io::Write;
use std::path::PathBuf;

use chrono::Local;
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

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

    let temporary = tempfile::NamedTempFile::new_in(&download_dir).map_err(private_io_error)?;
    let mut temporary = temporary;
    let mut zip = ZipWriter::new(temporary.as_file_mut());
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(6));

    // Legacy text logs may contain arbitrary provider errors. Never export them.
    let directory = crate::app_paths::app_data_dir(app)?.join("diagnostics");
    zip.start_file("runtime-events.jsonl", options)
        .map_err(private_zip_error)?;
    crate::runtime_diagnostics::export_records_to(&directory, &mut zip)
        .map_err(private_io_error)?;

    // ── 2. Sanitized settings ───────────────────────────────────────────
    let settings_json = read_sanitized_settings(app);
    if let Some(json) = settings_json {
        zip.start_file("settings.json", options)
            .map_err(private_zip_error)?;
        zip.write_all(json.as_bytes()).map_err(private_io_error)?;
    }

    // ── 3. System info ──────────────────────────────────────────────────
    let info = collect_system_info(app);
    zip.start_file("system-info.txt", options)
        .map_err(private_zip_error)?;
    zip.write_all(info.as_bytes()).map_err(private_io_error)?;

    zip.finish().map_err(private_zip_error)?;
    temporary
        .persist_noclobber(&zip_path)
        .map_err(private_persist_error)?;

    log::info!("Private diagnostics archive exported");

    Ok(zip_path)
}

// Filesystem/ZIP errors can include private paths or payloads. Never expose
// their arbitrary Display implementations through IPC or diagnostic logs.
fn private_io_error(_: std::io::Error) -> String {
    "Could not write private diagnostics archive".into()
}
fn private_zip_error(_: zip::result::ZipError) -> String {
    "Could not assemble private diagnostics archive".into()
}
fn private_persist_error(_: tempfile::PersistError) -> String {
    "Could not save diagnostics archive; existing files were not overwritten".into()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

    let value = sanitized_settings(&serde_json::Value::Object(map));

    serde_json::to_string_pretty(&value).ok()
}

/// Positive, typed projection: unknown/nested/string settings are excluded.
fn sanitized_settings(value: &serde_json::Value) -> serde_json::Value {
    let mut safe = serde_json::Map::new();
    for key in [
        "mic_auto_recover_enabled",
        "hot_mic_enabled",
        "main_window_show_on_launch",
    ] {
        if let Some(value) = value.get(key).and_then(serde_json::Value::as_bool) {
            safe.insert(key.into(), value.into());
        }
    }
    serde_json::Value::Object(safe)
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
pub(crate) mod tests {
    use super::*;
    #[test]
    fn archive_errors_never_expose_paths_or_overwrite_existing_evidence() {
        assert!(!private_io_error(std::io::Error::other("PRIVATE/path/token")).contains("PRIVATE"));
        assert!(
            !private_zip_error(zip::result::ZipError::Io(std::io::Error::other("PRIVATE")))
                .contains("PRIVATE")
        );
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("existing.zip");
        std::fs::write(&destination, b"existing evidence").unwrap();
        let temporary = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
        let error = temporary.persist_noclobber(&destination).unwrap_err();
        assert!(!private_persist_error(error).contains("PRIVATE"));
        assert_eq!(std::fs::read(destination).unwrap(), b"existing evidence");
    }
    #[cfg(target_os = "linux")]
    pub(crate) fn native_private_archive(app: &AppHandle) {
        use std::io::Read;
        let fixture = PathBuf::from(
            std::env::var_os("KOLBOO_NATIVE_WINDOW_DIRECTORY").expect("isolated fixture"),
        );
        let downloads = app.path().download_dir().unwrap();
        assert!(
            downloads.starts_with(&fixture),
            "never export into the operator's Downloads"
        );
        let store = app.store("settings.json").unwrap();
        store.set("proxy_password", serde_json::json!("PRIVATE"));
        store.set("arbitrary_prompt", serde_json::json!("PRIVATE"));
        store.set("mic_auto_recover_enabled", serde_json::json!(true));
        store.save().unwrap();
        let path = export_diagnostics_zip(app).unwrap();
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        assert_eq!(archive.len(), 3);
        for name in ["runtime-events.jsonl", "settings.json", "system-info.txt"] {
            let mut text = String::new();
            archive
                .by_name(name)
                .unwrap()
                .read_to_string(&mut text)
                .unwrap();
            assert!(!text.contains("PRIVATE"));
            assert!(!text.contains("proxy_password"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn export_projects_only_known_boolean_preferences() {
        let raw = serde_json::json!({"mic_auto_recover_enabled":true,"hot_mic_enabled":false,
            "main_window_show_on_launch":"PRIVATE", "start_at_login":null,
            "proxy_password":"PRIVATE", "api_key":"PRIVATE", "nested":{"token":"PRIVATE"},
            "prompt":"PRIVATE", "email":"PRIVATE"});
        assert_eq!(
            sanitized_settings(&raw),
            serde_json::json!({"mic_auto_recover_enabled":true,"hot_mic_enabled":false})
        );
        assert_eq!(
            sanitized_settings(&serde_json::Value::Null),
            serde_json::json!({})
        );
        assert_eq!(
            sanitized_settings(
                &serde_json::json!({"main_window_show_on_launch":true,"start_at_login":false})
            ),
            serde_json::json!({"main_window_show_on_launch":true})
        );
    }
}
