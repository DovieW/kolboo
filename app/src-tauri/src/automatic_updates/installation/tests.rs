use super::*;

fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut archive = tar::Builder::new(encoder);
    for (path, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        archive.append_data(&mut header, path, *bytes).unwrap();
    }
    archive.into_inner().unwrap().finish().unwrap()
}

#[test]
fn recognizes_only_supported_installed_packages() {
    use BundleType::*;
    for (os, bundle, expected) in [
        ("windows", Some(Nsis), InstallTarget::Tauri),
        ("windows", Some(Msi), InstallTarget::Tauri),
        ("linux", Some(AppImage), InstallTarget::Tauri),
        ("linux", Some(Deb), InstallTarget::Deb),
        ("linux", Some(Rpm), InstallTarget::Unsupported),
        ("windows", None, InstallTarget::Unsupported),
        (
            "macos",
            Some(App),
            InstallTarget::MacBundle("/Applications/Kolboo.app".into()),
        ),
    ] {
        assert_eq!(
            detect_target(
                os,
                bundle,
                Path::new("/Applications/Kolboo.app/Contents/MacOS/kolboo")
            ),
            expected
        );
        assert!(!expected.hint().is_empty());
    }
    for path in [
        "/kolboo",
        "/Applications/other/Contents/MacOS/kolboo",
        "/Applications/Kolboo.app/Contents/MacOS/other",
    ] {
        assert_eq!(
            detect_target("macos", Some(App), Path::new(path)),
            InstallTarget::Unsupported
        );
    }
}

#[test]
fn mac_replacement_preserves_old_app_on_invalid_archives_and_updates_in_place() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("Kolboo.app");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("old"), b"original").unwrap();
    for bytes in [
        b"not gzip".to_vec(),
        archive(&[]),
        archive(&[("One.app/a", b"1"), ("Two.app/a", b"2")]),
        archive(&[("Other/a", b"1")]),
        archive(&[("Kolboo.app/Contents/a", b"1")]),
    ] {
        assert!(install_mac_bundle(&bytes, &target).is_err());
        assert_eq!(std::fs::read(target.join("old")).unwrap(), b"original");
    }
    let bytes = archive(&[("Kolboo.app/Contents/MacOS/kolboo", b"new app")]);
    assert!(install_mac_bundle(&bytes, &root.path().join("Missing.app")).is_err());
    assert!(install_mac_bundle(&bytes, Path::new("/")).is_err());
    install_mac_bundle(&bytes, &target).unwrap();
    assert_eq!(
        std::fs::read(target.join("Contents/MacOS/kolboo")).unwrap(),
        b"new app"
    );
    assert!(!target.join("old").exists());
    assert_eq!(
        std::fs::read_dir(root.path()).unwrap().count(),
        1,
        "no old bundle or staging leak"
    );
}

#[test]
fn filesystem_transaction_rolls_back_and_keeps_backup_if_rollback_is_blocked() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("Kolboo.app");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("old"), b"original").unwrap();
    let stage = tempfile::tempdir_in(root.path()).unwrap();
    assert!(replace_bundle(stage, &root.path().join("missing"), &target).is_err());
    assert_eq!(std::fs::read(target.join("old")).unwrap(), b"original");
    let stage = tempfile::tempdir_in(root.path()).unwrap();
    let retained = stage.path().to_path_buf();
    let backup = stage.path().join("previous.app");
    std::fs::create_dir(&backup).unwrap();
    std::fs::write(backup.join("old"), b"original").unwrap();
    restore_backup(stage, &backup, &target);
    assert_eq!(std::fs::read(backup.join("old")).unwrap(), b"original");
    assert!(
        retained.exists(),
        "failed rollback must not delete the recoverable original"
    );
}

#[cfg(unix)]
#[test]
fn deb_authentication_is_os_owned_single_attempt_and_cancel_is_final() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let authorization = root.path().join("authorize");
    // A private process fixture observes the real argument boundary. No root
    // command runs, and no system package or authentication agent is changed.
    for exit in [0, 1] {
        let script = format!("#!/bin/sh\n[ \"$1\" = /usr/bin/dpkg ] || exit 2\n[ \"$2\" = --install ] || exit 3\n[ \"$(head -c 7 \"$3\")\" = '!<arch>' ] || exit 4\nprintf '%s' \"$3\" > '{}'/observed\nexit {exit}\n", root.path().display());
        std::fs::write(&authorization, script).unwrap();
        std::fs::set_permissions(&authorization, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            install_deb(b"!<arch>\nfixture", &authorization).is_ok(),
            exit == 0
        );
        let temporary_package = std::fs::read_to_string(root.path().join("observed")).unwrap();
        assert!(
            !Path::new(&temporary_package).exists(),
            "temporary package is removed after approval or cancellation"
        );
    }
    assert!(install_deb(b"invalid", &authorization).is_err());
    assert!(install_deb(b"!<arch>\nfixture", &root.path().join("missing")).is_err());
}
