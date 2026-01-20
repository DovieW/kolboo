use crate::shortcuts::normalize_shortcut_string;

#[test]
fn test_normalize_ctrl_to_control() {
    assert_eq!(normalize_shortcut_string("Ctrl+Space"), "control+space");
}

#[test]
fn test_normalize_uppercase_ctrl() {
    // Canonical form puts modifiers first.
    assert_eq!(normalize_shortcut_string("CTRL+A"), "control+a");
}

#[test]
fn test_normalize_cmd_to_super() {
    // Canonical form puts modifiers first.
    assert_eq!(normalize_shortcut_string("cmd+shift+a"), "shift+super+a");
}

#[test]
fn test_normalize_win_to_super() {
    // Canonical form puts modifiers first.
    assert_eq!(normalize_shortcut_string("WIN+a"), "super+a");
}

#[test]
fn test_normalize_meta_to_super() {
    // Canonical form puts modifiers first.
    assert_eq!(normalize_shortcut_string("Meta+b"), "super+b");
}

#[test]
fn test_normalize_multiple_replacements() {
    // ctrl+meta should become control+super
    assert_eq!(normalize_shortcut_string("ctrl+meta+x"), "control+super+x");
}

#[test]
fn test_normalize_already_normalized() {
    assert_eq!(
        normalize_shortcut_string("control+alt+space"),
        "alt+control+space"
    );
}

#[test]
fn test_normalize_is_order_insensitive() {
    assert_eq!(
        normalize_shortcut_string("ctrl+shift+F3"),
        normalize_shortcut_string("shift+control+f3")
    );
}

#[test]
fn test_normalize_preserves_non_modifier_parts() {
    assert_eq!(
        normalize_shortcut_string("ctrl+Backquote"),
        // Canonical form puts modifiers first.
        "control+backquote"
    );
}

#[test]
fn test_normalize_empty_string() {
    assert_eq!(normalize_shortcut_string(""), "");
}

#[test]
fn test_normalize_single_key() {
    assert_eq!(normalize_shortcut_string("Space"), "space");
}
