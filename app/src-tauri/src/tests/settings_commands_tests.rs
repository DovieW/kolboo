use crate::settings::HotkeyConfig;

// Tests for HotkeyConfig
#[test]
fn test_default_toggle_hotkey() {
    let hotkey = HotkeyConfig::default_toggle();
    #[cfg(target_os = "windows")]
    assert_eq!(hotkey.key, "AltRight");

    #[cfg(not(target_os = "windows"))]
    assert_eq!(hotkey.key, "F3");
    assert!(hotkey.modifiers.is_empty());
}

#[test]
fn test_default_hold_hotkey() {
    let hotkey = HotkeyConfig::default_hold();
    assert!(hotkey.is_none());
}

#[test]
fn test_default_paste_last_hotkey() {
    let hotkey = HotkeyConfig::default_paste_last();
    assert!(hotkey.is_none());
}

#[test]
fn test_default_retry_hotkey() {
    let hotkey = HotkeyConfig::default_retry();
    assert!(hotkey.is_none());
}

#[test]
fn test_default_quick_ask_hotkey() {
    let hotkey = HotkeyConfig::default_quick_ask();
    assert!(hotkey.is_none());
}

#[test]
fn test_to_shortcut_string() {
    let hotkey = HotkeyConfig {
        modifiers: vec!["Ctrl".to_string(), "Alt".to_string()],
        key: "Space".to_string(),
    };
    // Modifiers should be lowercased
    let result = hotkey.to_shortcut_string();
    assert!(result.contains("ctrl"));
    assert!(result.contains("alt"));
    assert!(result.contains("Space"));
}
