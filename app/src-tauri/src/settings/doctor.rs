use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{HotkeyConfig, ProxySettings, RewriteProgramPromptProfile, VadSettings};

pub const SETTINGS_DOCTOR_KEYS: &[&str] = &[
    "settings_version",
    "proxy_settings",
    "vad_settings",
    "rewrite_program_prompt_profiles",
    "toggle_hotkey",
    "hold_hotkey",
    "paste_last_hotkey",
    "retry_hotkey",
    "quick_ask_hotkey",
    "quick_ask_hold_hotkey",
    "quick_ask_toggle_hotkey",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettingsDoctorIssue {
    pub key: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SettingsDoctorReport {
    pub issues: Vec<SettingsDoctorIssue>,
}

impl SettingsDoctorReport {
    fn push_issue(&mut self, key: &str, message: String) {
        self.issues.push(SettingsDoctorIssue {
            key: key.to_string(),
            message,
        });
    }
}

pub fn validate_settings_map(values: &Map<String, Value>) -> SettingsDoctorReport {
    let mut report = SettingsDoctorReport::default();

    validate_value::<u32>(
        &mut report,
        "settings_version",
        "u32",
        values.get("settings_version"),
        false,
    );
    validate_value::<ProxySettings>(
        &mut report,
        "proxy_settings",
        "ProxySettings",
        values.get("proxy_settings"),
        false,
    );
    validate_value::<VadSettings>(
        &mut report,
        "vad_settings",
        "VadSettings",
        values.get("vad_settings"),
        false,
    );
    validate_value::<Vec<RewriteProgramPromptProfile>>(
        &mut report,
        "rewrite_program_prompt_profiles",
        "RewriteProgramPromptProfile[]",
        values.get("rewrite_program_prompt_profiles"),
        false,
    );

    for key in [
        "toggle_hotkey",
        "hold_hotkey",
        "paste_last_hotkey",
        "retry_hotkey",
        "quick_ask_hotkey",
        "quick_ask_hold_hotkey",
        "quick_ask_toggle_hotkey",
    ] {
        validate_value::<HotkeyConfig>(&mut report, key, "HotkeyConfig", values.get(key), true);
    }

    report
}

fn validate_value<T: DeserializeOwned>(
    report: &mut SettingsDoctorReport,
    key: &str,
    expected: &str,
    value: Option<&Value>,
    allow_null: bool,
) {
    let Some(value) = value else {
        return;
    };

    if value.is_null() {
        if allow_null {
            return;
        }

        report.push_issue(key, format!("Expected {expected} but found null"));
        return;
    }

    if let Err(err) = serde_json::from_value::<T>(value.clone()) {
        report.push_issue(
            key,
            format!("Expected {expected}; failed to parse value: {err}"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reports_invalid_settings_version() {
        let mut map = Map::new();
        map.insert("settings_version".to_string(), json!("oops"));

        let report = validate_settings_map(&map);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].key, "settings_version");
    }

    #[test]
    fn allows_null_hotkeys() {
        let mut map = Map::new();
        map.insert("toggle_hotkey".to_string(), Value::Null);

        let report = validate_settings_map(&map);
        assert!(report.issues.is_empty());
    }

    #[test]
    fn reports_invalid_profiles() {
        let mut map = Map::new();
        map.insert(
            "rewrite_program_prompt_profiles".to_string(),
            json!("not-an-array"),
        );

        let report = validate_settings_map(&map);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].key, "rewrite_program_prompt_profiles");
    }
}
