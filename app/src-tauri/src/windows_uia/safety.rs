use super::types::WindowsTextTargetSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertBlockReason {
    Password,
    Disabled,
    ReadOnly,
}

impl InsertBlockReason {
    pub fn as_str(self) -> &'static str {
        match self {
            InsertBlockReason::Password => "password_field",
            InsertBlockReason::Disabled => "disabled_control",
            InsertBlockReason::ReadOnly => "read_only_control",
        }
    }
}

/// Determine whether it is safe to capture context from the focused element.
///
/// We never capture from password/secure fields.
pub fn allow_context_capture(snapshot: &WindowsTextTargetSnapshot) -> bool {
    !matches!(snapshot.is_password, Some(true))
}

/// Determine whether it is safe to insert text into the focused element.
///
/// Block insertion into password fields and disabled/read-only controls.
pub fn allow_insert(snapshot: &WindowsTextTargetSnapshot) -> bool {
    insert_block_reason(snapshot).is_none()
}

/// Determine whether it is safe to insert text into the focused element, honoring
/// the smart paste protection setting.
pub fn allow_insert_with_protection(
    snapshot: &WindowsTextTargetSnapshot,
    protection_enabled: bool,
) -> bool {
    if !protection_enabled {
        return true;
    }

    allow_insert(snapshot)
}

/// If insertion is blocked by policy, return a specific reason.
///
/// This is safe to log (no user text). It is used to make logs more actionable.
pub fn insert_block_reason(snapshot: &WindowsTextTargetSnapshot) -> Option<InsertBlockReason> {
    if matches!(snapshot.is_password, Some(true)) {
        return Some(InsertBlockReason::Password);
    }

    if matches!(snapshot.is_enabled, Some(false)) {
        return Some(InsertBlockReason::Disabled);
    }

    if matches!(snapshot.is_read_only, Some(true)) {
        return Some(InsertBlockReason::ReadOnly);
    }

    None
}

/// If smart paste protection is enabled, return the block reason; otherwise allow insert.
pub fn insert_block_reason_with_protection(
    snapshot: &WindowsTextTargetSnapshot,
    protection_enabled: bool,
) -> Option<InsertBlockReason> {
    if !protection_enabled {
        return None;
    }

    insert_block_reason(snapshot)
}

#[cfg(test)]
mod tests {
    use super::{allow_context_capture, allow_insert, allow_insert_with_protection};
    use crate::windows_uia::types::WindowsTextTargetSnapshot;

    fn base_snapshot() -> WindowsTextTargetSnapshot {
        WindowsTextTargetSnapshot {
            captured_at_ms: 0,
            process_id: None,
            exe_path: None,
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
    fn blocks_context_capture_for_password() {
        let mut snapshot = base_snapshot();
        snapshot.is_password = Some(true);
        assert!(!allow_context_capture(&snapshot));
    }

    #[test]
    fn blocks_insert_for_password() {
        let mut snapshot = base_snapshot();
        snapshot.is_password = Some(true);
        assert!(!allow_insert(&snapshot));
    }

    #[test]
    fn blocks_insert_for_read_only() {
        let mut snapshot = base_snapshot();
        snapshot.is_read_only = Some(true);
        assert!(!allow_insert(&snapshot));
    }

    #[test]
    fn blocks_insert_for_disabled() {
        let mut snapshot = base_snapshot();
        snapshot.is_enabled = Some(false);
        assert!(!allow_insert(&snapshot));
    }

    #[test]
    fn allows_insert_when_unknown() {
        let snapshot = base_snapshot();
        assert!(allow_insert(&snapshot));
    }

    #[test]
    fn allows_insert_when_protection_disabled() {
        let mut snapshot = base_snapshot();
        snapshot.is_password = Some(true);
        assert!(allow_insert_with_protection(&snapshot, false));
    }

    #[test]
    fn blocks_insert_when_protection_enabled() {
        let mut snapshot = base_snapshot();
        snapshot.is_password = Some(true);
        assert!(!allow_insert_with_protection(&snapshot, true));
    }
}
