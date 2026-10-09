//! A model's payment source is part of its identity, never inferred from access.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
