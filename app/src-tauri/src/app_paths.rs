use std::path::{Path, PathBuf};

use tauri::AppHandle;
use tauri::Manager;

pub const APP_DATA_DIR_ENV: &str = "KOLBOO_APP_DATA_DIR";

pub fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(override_dir) = std::env::var(APP_DATA_DIR_ENV) {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    app.path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))
}

pub fn ensure_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|e| format!("Failed to create dir {}: {e}", path.display()))
}

#[allow(dead_code)]
#[cfg(feature = "local-whisper")]
pub fn app_data_subdir(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let base = app_data_dir(app)?;
    let dir = base.join(name);
    ensure_dir(&dir)?;
    Ok(dir)
}
