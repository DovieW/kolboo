//! A model's payment source is part of its identity, never inferred from access.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ModelAction {
    Transcription,
    Rewrite,
    QuickAsk,
    QuickReplace,
    IntentRouter,
    HistoryAnalysis,
    PromptTools,
}

impl ModelAction {
    pub const ALL: [Self; 7] = [
        Self::Transcription,
        Self::Rewrite,
        Self::QuickAsk,
        Self::QuickReplace,
        Self::IntentRouter,
        Self::HistoryAnalysis,
        Self::PromptTools,
    ];

    pub fn settings_key(self) -> &'static str {
        match self {
            Self::Transcription => "model_selection_transcription",
            Self::Rewrite => "model_selection_rewrite",
            Self::QuickAsk => "model_selection_quick_ask",
            Self::QuickReplace => "model_selection_quick_replace",
            Self::IntentRouter => "model_selection_intent_router",
            Self::HistoryAnalysis => "model_selection_history_analysis",
            Self::PromptTools => "model_selection_prompt_tools",
        }
    }
}

pub type ModelSelections = BTreeMap<ModelAction, ModelSelection>;

pub fn deserialize_selections<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<ModelSelections, D::Error> {
    Ok(Option::<ModelSelections>::deserialize(deserializer)?.unwrap_or_default())
}

/// Independent action keys avoid a read/merge/write race between UI consumers.
/// Invalid explicit choices must not turn into legacy routing.
#[derive(Default, Serialize, JsonSchema)]
pub struct LoadedModelSettings {
    #[serde(rename = "model_selections")]
    pub selections: ModelSelections,
    pub profiles: Vec<crate::settings::RewriteProgramPromptProfile>,
    pub stt_input_mode: crate::settings::SttInputMode,
    pub error: Option<String>,
}

pub fn scoped_selection_key(
    profile_id: &str,
    preset_id: Option<&str>,
    action: ModelAction,
) -> String {
    format!(
        "model_selection_scope:{}",
        serde_json::json!([preset_id.is_none().then_some(profile_id), preset_id, action])
    )
}

pub fn parse_scoped_selection_key(
    key: &str,
) -> Result<(Option<String>, Option<String>, ModelAction), String> {
    let raw = key
        .strip_prefix("model_selection_scope:")
        .ok_or("Invalid model scope")?;
    let scope: (Option<String>, Option<String>, ModelAction) =
        serde_json::from_str(raw).map_err(|_| "Invalid model scope")?;
    if scope.0.as_ref().is_some_and(|id| id.is_empty())
        || scope.1.as_ref().is_some_and(|id| id.is_empty())
        || scope.0.is_some() == scope.1.is_some()
        || scope.0.as_deref() == Some("default")
    {
        return Err("Use global settings for the Default model selection".into());
    }
    if scoped_selection_key(
        scope.0.as_deref().unwrap_or_default(),
        scope.1.as_deref(),
        scope.2,
    ) != key
    {
        return Err("Invalid model scope".into());
    }
    Ok(scope)
}

#[cfg(desktop)]
pub fn read_model_settings<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> LoadedModelSettings {
    read_model_settings_locked(app, &crate::settings::patch::WRITE_LOCK)
}

#[cfg(desktop)]
pub(crate) fn read_model_settings_locked<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    write_lock: &std::sync::Mutex<()>,
) -> LoadedModelSettings {
    use tauri_plugin_store::StoreExt;
    let Ok(_guard) = write_lock.lock() else {
        return LoadedModelSettings {
            error: Some("Model settings are unavailable".into()),
            ..Default::default()
        };
    };
    match app.store("settings.json") {
        Ok(store) => load_global_selections(&|key| store.get(key)),
        Err(_) => LoadedModelSettings {
            error: Some("Model settings could not be read safely".into()),
            ..Default::default()
        },
    }
}

pub fn load_global_selections(
    get: &dyn Fn(&str) -> Option<serde_json::Value>,
) -> LoadedModelSettings {
    let mut selections = ModelSelections::new();
    let mut cleared = std::collections::BTreeSet::new();
    for action in ModelAction::ALL {
        let value = get(action.settings_key());
        if value.as_ref().is_some_and(|value| value.is_null()) {
            cleared.insert(action);
        }
        if let Some(value) = value.filter(|value| !value.is_null()) {
            let parsed = serde_json::from_value::<ModelSelection>(value)
                .map_err(|_| {
                    "A saved model selection is invalid. Choose its model again".to_string()
                })
                .and_then(|selection| selection.validate().map(|_| selection));
            match parsed {
                Ok(selection) => {
                    selections.insert(action, selection);
                }
                Err(error) => {
                    return LoadedModelSettings {
                        error: Some(error),
                        ..Default::default()
                    };
                }
            }
        }
    }
    let parsed = get("rewrite_program_prompt_profiles")
        .filter(|value| !value.is_null())
        .map(|value| {
            serde_json::from_value::<Vec<crate::settings::RewriteProgramPromptProfile>>(value)
                .map_err(|_| {
                    "Saved model profiles are invalid. Repair their settings before recording"
                        .to_string()
                })
        })
        .unwrap_or_else(|| Ok(Vec::new()));
    let mut stt_input_mode = crate::settings::SttInputMode::default();
    match parsed.and_then(|mut profiles| {
        if let Some(value) = get("stt_input_mode").filter(|value| !value.is_null()) {
            stt_input_mode = serde_json::from_value(value)
                .map_err(|_| "The saved transcription input mode is invalid".to_string())?;
        }
        for profile in &mut profiles {
            for action in ModelAction::ALL {
                if let Some(value) = get(&scoped_selection_key(&profile.id, None, action)) {
                    if value.is_null() {
                        profile.model_selections.remove(&action);
                        clear_profile_legacy_choice(profile, action);
                    } else {
                        let choice = serde_json::from_value::<ModelSelection>(value)
                            .map_err(|_| "Invalid scoped model selection".to_string())?;
                        choice.validate()?;
                        profile.model_selections.insert(action, choice);
                    }
                }
                for preset in &mut profile.presets {
                    if let Some(value) =
                        get(&scoped_selection_key(&profile.id, Some(&preset.id), action))
                    {
                        if value.is_null() {
                            preset.model_selections.remove(&action);
                            match action {
                                ModelAction::Transcription => {
                                    preset.stt_provider = None;
                                    preset.stt_model = None;
                                }
                                ModelAction::Rewrite => {
                                    preset.llm_provider = None;
                                    preset.llm_model = None;
                                }
                                _ => {}
                            }
                        } else {
                            let choice = serde_json::from_value::<ModelSelection>(value)
                                .map_err(|_| "Invalid preset model selection".to_string())?;
                            choice.validate()?;
                            preset.model_selections.insert(action, choice);
                        }
                    }
                }
            }
            if profile.id == "default" {
                for action in ModelAction::ALL {
                    if let Some(choice) = selections.get(&action) {
                        profile.model_selections.insert(action, choice.clone());
                    } else if cleared.contains(&action) {
                        profile.model_selections.remove(&action);
                        clear_profile_legacy_choice(profile, action);
                    } else if let Some(choice) = profile.model_selections.get(&action) {
                        selections.insert(action, choice.clone());
                    }
                }
            }
        }
        validate_profile_selections(&profiles)?;
        validate_selection_policy(&selections, &profiles, get)?;
        Ok(profiles)
    }) {
        Ok(profiles) => LoadedModelSettings {
            selections,
            profiles,
            stt_input_mode,
            error: None,
        },
        Err(error) => LoadedModelSettings {
            error: Some(error),
            ..Default::default()
        },
    }
}

/// Organization constraints must not be bypassed by a newly introduced route
/// field. Conflicts remain visible; never rewrite a saved payment source.
fn validate_selection_policy(
    selections: &ModelSelections,
    profiles: &[crate::settings::RewriteProgramPromptProfile],
    get: &dyn Fn(&str) -> Option<serde_json::Value>,
) -> Result<(), String> {
    let policy = crate::policy::normalize_policy_state(get("policy_state"), chrono::Utc::now());
    if policy.source == crate::policy::PolicySource::None || !policy.is_valid {
        return Ok(());
    }
    let values = get("policy_effective_values").unwrap_or_default();
    for (action, choice) in selections.iter().chain(profiles.iter().flat_map(|profile| {
        profile.model_selections.iter().chain(
            profile
                .presets
                .iter()
                .flat_map(|preset| preset.model_selections.iter()),
        )
    })) {
        let paths = match action {
            ModelAction::Transcription => ["stt_provider", "stt_model"],
            ModelAction::QuickAsk => ["quick_ask_provider", "quick_ask_model"],
            ModelAction::QuickReplace => ["quick_replace_provider", "quick_replace_model"],
            _ => ["llm_provider", "llm_model"],
        };
        for (path, selected) in paths
            .into_iter()
            .zip([&choice.provider_id, &choice.model_id])
        {
            if policy
                .enforced_fields
                .iter()
                .any(|field| field.path == path)
                && values
                    .get(path)
                    .is_some_and(|value| value.as_str() != Some(selected.as_str()))
            {
                return Err("A saved model choice conflicts with organization policy. Your choice has not been changed".into());
            }
        }
    }
    Ok(())
}

fn clear_profile_legacy_choice(
    profile: &mut crate::settings::RewriteProgramPromptProfile,
    action: ModelAction,
) {
    match action {
        ModelAction::Transcription => {
            profile.stt_provider = None;
            profile.stt_model = None;
        }
        ModelAction::Rewrite => {
            profile.llm_provider = None;
            profile.llm_model = None;
        }
        ModelAction::QuickAsk => {
            profile.quick_ask_provider = None;
            profile.quick_ask_model = None;
        }
        ModelAction::QuickReplace => {
            profile.quick_replace_provider = None;
            profile.quick_replace_model = None;
        }
        ModelAction::IntentRouter => {
            if let Some(router) = profile.router.as_mut() {
                router.llm_provider = None;
                router.llm_model = None;
            }
        }
        _ => {}
    }
}

pub fn validate_profile_selections(
    profiles: &[crate::settings::RewriteProgramPromptProfile],
) -> Result<(), String> {
    for profile in profiles {
        for selection in profile.model_selections.values().chain(
            profile
                .presets
                .iter()
                .flat_map(|preset| preset.model_selections.values()),
        ) {
            selection.validate()?;
        }
    }
    Ok(())
}

/// Atomic choices win within a layer. Legacy provider/model overrides retain
/// an inherited explicit source rather than guessing another payment route.
pub fn inherit_model_selection(
    base: Option<&ModelSelection>,
    layers: &[(Option<&ModelSelection>, Option<&str>, Option<&str>)],
) -> Option<ModelSelection> {
    let mut selection = base.cloned();
    for &(explicit, provider, model) in layers {
        if let Some(explicit) = explicit {
            selection = Some(explicit.clone());
        } else if let Some(selection) = selection.as_mut() {
            if let Some(provider) = provider.filter(|id| !id.trim().is_empty()) {
                selection.provider_id = provider.to_string();
            }
            if let Some(model) = model.filter(|id| !id.trim().is_empty()) {
                selection.model_id = model.to_string();
            }
        }
    }
    selection
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelSource {
    Managed,
    Byok,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelSelection {
    pub source: ModelSource,
    pub provider_id: String,
    pub model_id: String,
}

impl ModelSelection {
    pub fn validate(&self) -> Result<(), String> {
        if self.provider_id.trim().is_empty()
            || self.model_id.trim().is_empty()
            || self.provider_id.len() > 100
            || self.model_id.len() > 256
            || self
                .provider_id
                .chars()
                .chain(self.model_id.chars())
                .any(char::is_control)
        {
            return Err("Select a valid provider and model".into());
        }
        Ok(())
    }
}

impl From<&crate::recordings::options::MeetingModel> for ModelSelection {
    fn from(model: &crate::recordings::options::MeetingModel) -> Self {
        Self {
            source: if model.use_managed {
                ModelSource::Managed
            } else {
                ModelSource::Byok
            },
            provider_id: model.provider.clone(),
            model_id: model.model.clone(),
        }
    }
}

#[cfg(test)]
#[path = "model_selection/tests.rs"]
mod routing_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_is_explicit_and_legacy_meeting_choices_retain_it() {
        for (flag, expected) in [(false, ModelSource::Byok), (true, ModelSource::Managed)] {
            let legacy = crate::recordings::options::MeetingModel {
                provider: "provider".into(),
                model: "raw-id".into(),
                use_managed: flag,
            };
            let choice = ModelSelection::from(&legacy);
            assert_eq!(choice.source, expected);
            assert_eq!(choice.provider_id, "provider");
            assert_eq!(choice.model_id, "raw-id");
            assert!(choice.validate().is_ok());
            assert_eq!(
                serde_json::from_value::<ModelSelection>(serde_json::to_value(&choice).unwrap())
                    .unwrap(),
                choice
            );
        }
        assert!(serde_json::from_value::<ModelSelection>(
            serde_json::json!({ "provider_id": "provider", "model_id": "raw-id" })
        )
        .is_err());
        assert!(serde_json::from_value::<ModelSelection>(serde_json::json!({ "source": "unknown", "provider_id": "provider", "model_id": "raw-id" })).is_err());
    }

    #[test]
    fn validation_rejects_empty_oversized_and_control_bearing_identifiers() {
        for (provider_id, model_id) in [
            (" ".into(), "id".into()),
            ("provider".into(), " ".into()),
            ("p".repeat(101), "id".into()),
            ("provider".into(), "m".repeat(257)),
            ("provider".into(), "model\nID".into()),
        ] {
            assert!(ModelSelection {
                source: ModelSource::Byok,
                provider_id,
                model_id
            }
            .validate()
            .is_err());
        }
    }
}
