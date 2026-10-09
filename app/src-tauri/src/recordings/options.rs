//! Non-secret recording preferences and immutable per-recording options.
//! Missing metadata is legacy Dictation; duration/filename never selects a mode.
use crate::fs::{Fs, RealFs};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecordingMode {
    #[default]
    Dictation,
    Meeting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MeetingModel {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub use_managed: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RecordingPreferences {
    #[serde(default)]
    pub mode: RecordingMode,
    #[serde(default)]
    pub meeting_model: Option<MeetingModel>,
    /// Immutable file/meeting hint. Missing/null means auto, never later globals.
    #[serde(default)]
    pub language_hint: Option<String>,
}

impl RecordingPreferences {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(hint) = &self.language_hint {
            if hint.is_empty()
                || hint.len() > 16
                || !hint
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
            {
                return Err("Select a valid recording language hint".into());
            }
        }
        if let Some(selection) = &self.meeting_model {
            crate::ModelSelection::from(selection).validate()?;
        }
        Ok(())
    }
    pub fn save_journal(&self, path: &std::path::Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| "Could not save recording mode")?;
        RealFs
            .write_private(&path.with_extension("options.json"), &bytes)
            .map_err(|_| "Could not save recording mode".into())
    }
    pub fn load_journal(path: &std::path::Path) -> Result<Self, String> {
        read_options(&RealFs, &path.with_extension("options.json"))
    }
}

fn read_options(fs: &dyn Fs, path: &std::path::Path) -> Result<RecordingPreferences, String> {
    match fs.read(path) {
        Ok(bytes) => {
            let options: RecordingPreferences = serde_json::from_slice(&bytes)
                .map_err(|_| "Saved recording mode could not be read; audio is preserved")?;
            options.validate()?;
            Ok(options)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(RecordingPreferences::default()),
        Err(_) => Err("Saved recording mode could not be read; audio is preserved".into()),
    }
}

impl super::RecordingStore {
    pub fn save_options(&self, id: &str, options: &RecordingPreferences) -> Result<(), String> {
        let _guard = self
            .media_write
            .lock()
            .map_err(|_| "Recording store unavailable")?;
        if !Self::is_safe_request_id(id) {
            return Err("Invalid recording id".into());
        }
        options.validate()?;
        let bytes = serde_json::to_vec(options).map_err(|_| "Could not save recording mode")?;
        self.fs
            .write_private(&self.dir.join(format!("{id}.options.json")), &bytes)
            .map_err(|_| "Could not save recording mode".into())
    }
    pub fn options(&self, id: &str) -> Result<RecordingPreferences, String> {
        if !Self::is_safe_request_id(id) {
            return Err("Invalid recording id".into());
        }
        read_options(
            self.fs.as_ref(),
            &self.dir.join(format!("{id}.options.json")),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mode_roundtrips_without_inference_and_corruption_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("long-meeting-final.pcm");
        assert_eq!(
            RecordingPreferences::load_journal(&path).unwrap().mode,
            RecordingMode::Dictation
        );
        let options = RecordingPreferences {
            mode: RecordingMode::Meeting,
            meeting_model: Some(MeetingModel {
                provider: "openai".into(),
                model: "gpt-4o-transcribe-diarize".into(),
                use_managed: false,
            }),
            language_hint: Some("yi".into()),
        };
        options.save_journal(&path).unwrap();
        assert_eq!(
            RecordingPreferences::load_journal(&path)
                .unwrap()
                .meeting_model,
            options.meeting_model
        );
        assert_eq!(
            RecordingPreferences::load_journal(&path)
                .unwrap()
                .language_hint
                .as_deref(),
            Some("yi")
        );
        std::fs::write(path.with_extension("options.json"), b"broken").unwrap();
        assert!(RecordingPreferences::load_journal(&path).is_err());
    }

    #[test]
    fn legacy_hints_default_to_auto_and_invalid_hints_fail_closed() {
        let legacy: RecordingPreferences = serde_json::from_str(r#"{"mode":"meeting"}"#).unwrap();
        assert!(legacy.language_hint.is_none());
        assert!(legacy.validate().is_ok());
        for hint in ["", "English", "en\n", "x".repeat(17).as_str(), "key=secret"] {
            let invalid = RecordingPreferences {
                language_hint: Some(hint.into()),
                ..Default::default()
            };
            assert!(invalid.validate().is_err());
        }
        for hint in ["auto", "yi", "he", "en", "zh-cn"] {
            assert!(RecordingPreferences {
                language_hint: Some(hint.into()),
                ..Default::default()
            }
            .validate()
            .is_ok());
        }
    }
}
