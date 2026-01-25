use serde_json::json;

use crate::settings::{filter_enabled_rewrite_profiles, RewriteProgramPromptProfile};

#[test]
fn filter_enabled_rewrite_profiles_excludes_disabled() {
    let enabled: RewriteProgramPromptProfile =
        serde_json::from_value(json!({"id": "enabled", "name": "Enabled"}))
            .expect("valid enabled profile");
    let disabled: RewriteProgramPromptProfile =
        serde_json::from_value(json!({"id": "disabled", "name": "Disabled", "disabled": true}))
            .expect("valid disabled profile");

    let filtered = filter_enabled_rewrite_profiles(vec![enabled, disabled]);

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "enabled");
}
