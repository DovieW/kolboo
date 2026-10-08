//! Platform installation boundaries. Downloads have already passed Tauri's
//! signature verification. Never collect sudo passwords inside Kolboo.
use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use tauri::utils::config::BundleType;
use tauri_plugin_updater::Update;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) enum InstallTarget {
    #[default]
    Unsupported,
    Tauri,
    Deb,
    MacBundle(PathBuf),
}

pub(super) fn detect_target(os: &str, bundle: Option<BundleType>, exe: &Path) -> InstallTarget {
    match (os, bundle) {
        ("windows", Some(BundleType::Nsis | BundleType::Msi))
        | ("linux", Some(BundleType::AppImage)) => InstallTarget::Tauri,
        ("linux", Some(BundleType::Deb)) => InstallTarget::Deb,
        ("macos", Some(BundleType::App)) => {
            let bundle = exe.parent().and_then(Path::parent).and_then(Path::parent);
            match bundle {
                Some(bundle)
                    if bundle.extension().is_some_and(|ext| ext == "app")
                        && exe.ends_with("Contents/MacOS/kolboo") =>
                {
                    InstallTarget::MacBundle(bundle.into())
                }
                _ => InstallTarget::Unsupported,
            }
        }
        _ => InstallTarget::Unsupported,
    }
}

impl InstallTarget {
    pub(super) fn hint(&self) -> &'static str {
        match self {
            Self::Unsupported => "This build uses manual downloads.",
            Self::Tauri => "Updates download automatically and install when you quit. Installing now closes Kolboo.",
            Self::Deb => "Updates download automatically. Installation may ask for administrator approval. Installing now closes Kolboo.",
            Self::MacBundle(_) => "Updates download automatically. Kolboo must be in a writable Applications folder, not the DMG. Installing now closes Kolboo; reopen it to use the update.",
        }
    }
    pub(super) fn install(&self, update: &Update, bytes: &[u8]) -> Result<(), String> {
        match self {
            Self::Unsupported => Err("Unsupported installation".into()),
            Self::Tauri => update
                .install(bytes)
                .map_err(|_| "Installation failed".into()),
            Self::Deb => install_deb(bytes, Path::new("/usr/bin/pkexec"))
                .map_err(|_| "Administrator approval or package installation failed".into()),
            Self::MacBundle(bundle) => install_mac_bundle(bytes, bundle)
                .map_err(|_| "Application replacement failed".into()),
        }
    }
}

fn install_deb(bytes: &[u8], authorization: &Path) -> io::Result<()> {
    if !bytes.starts_with(b"!<arch>\n") {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let directory = tempfile::tempdir()?;
    let package = directory.path().join("kolboo.deb");
    std::fs::write(&package, bytes)?;
    // A single OS-owned authentication prompt. Cancellation is final: do not
    // fall through to zenity/kdialog password collection or terminal sudo.
    let result = Command::new(authorization)
        .arg("/usr/bin/dpkg")
        .arg("--install")
        .arg(&package)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if result.success() {
        Ok(())
    } else {
        Err(io::ErrorKind::PermissionDenied.into())
    }
}

fn install_mac_bundle(bytes: &[u8], target: &Path) -> io::Result<()> {
    // All staging/backup paths are private siblings on the same filesystem.
    // Failure to obtain write access happens before touching the current app.
    let parent = target.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let directory = tempfile::tempdir_in(parent)?;
    let staging = directory.path().join("staging");
    std::fs::create_dir(&staging)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    archive.unpack(&staging)?; // tar confines extracted paths to staging.
    let roots = std::fs::read_dir(&staging)?.collect::<Result<Vec<_>, _>>()?;
    if roots.len() != 1 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let updated = roots[0].path();
    if !roots[0].file_type()?.is_dir()
        || updated.extension().is_none_or(|ext| ext != "app")
        || !updated.join("Contents/MacOS/kolboo").is_file()
        || !target.is_dir()
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    replace_bundle(directory, &updated, target)
}

fn replace_bundle(directory: tempfile::TempDir, updated: &Path, target: &Path) -> io::Result<()> {
    let backup = directory.path().join("previous.app");
    std::fs::rename(target, &backup)?;
    if let Err(error) = std::fs::rename(updated, target) {
        // Do not delete the backup if a filesystem fault also prevents rollback.
        restore_backup(directory, &backup, target);
        return Err(error);
    }
    Ok(())
}

fn restore_backup(directory: tempfile::TempDir, backup: &Path, target: &Path) {
    if std::fs::rename(backup, target).is_err() {
        let _ = directory.keep();
    }
}

#[cfg(test)]
mod tests;
