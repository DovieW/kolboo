use super::types::WindowsTextTargetSnapshot;

pub fn target_matches(
    snapshot: &WindowsTextTargetSnapshot,
    current: &WindowsTextTargetSnapshot,
) -> bool {
    if let (Some(snapshot_pid), Some(current_pid)) = (snapshot.process_id, current.process_id) {
        if snapshot_pid != current_pid {
            return false;
        }
    }

    if let (Some(snapshot_path), Some(current_path)) =
        (snapshot.exe_path.as_deref(), current.exe_path.as_deref())
    {
        if !snapshot_path.eq_ignore_ascii_case(current_path) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(pid: Option<u32>, exe: Option<&str>) -> WindowsTextTargetSnapshot {
        WindowsTextTargetSnapshot {
            captured_at_ms: 0,
            process_id: pid,
            exe_path: exe.map(|v| v.to_string()),
            window_title: None,
            uia_runtime_id: None,
            is_password: None,
            is_enabled: None,
            is_read_only: None,
            supports_text_pattern: false,
            supports_value_pattern: false,
        }
    }

    #[test]
    fn matches_when_identifiers_align() {
        let a = snapshot(Some(100), Some("C:/Apps/Note.exe"));
        let b = snapshot(Some(100), Some("c:/apps/note.exe"));
        assert!(target_matches(&a, &b));
    }

    #[test]
    fn rejects_on_process_mismatch() {
        let a = snapshot(Some(100), None);
        let b = snapshot(Some(101), None);
        assert!(!target_matches(&a, &b));
    }

    #[test]
    fn rejects_on_exe_mismatch() {
        let a = snapshot(None, Some("C:/Apps/Note.exe"));
        let b = snapshot(None, Some("C:/Apps/Other.exe"));
        assert!(!target_matches(&a, &b));
    }
}
