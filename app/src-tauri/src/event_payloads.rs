use schemars::JsonSchema;

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct SystemEvent {
    pub timestamp: String,
    pub event_type: String,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct PipelineErrorPayload {
    pub message: String,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SttPartialTranscriptPayload {
    Started { session_epoch: u64 },
    Partial { session_epoch: u64, text: String },
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct OverlayAudioLevelPayload {
    pub seq: u64,
    pub rms: f32,
    pub peak: f32,
    pub wave_seq: Option<u64>,
    pub mins: Option<Vec<f32>>,
    pub maxes: Option<Vec<f32>>,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct OverlayOcrContextUnavailablePayload {
    /// Short user-facing message (e.g. "OCR context unavailable").
    pub message: String,
    /// Optional diagnostic reason (safe to show to the user; should not include secrets).
    pub reason: Option<String>,
    /// Current request id (if known) for correlating with request logs.
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct QuickAskStartedPayload {
    pub question: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct QuickAskAnswerOkPayload {
    pub ok: bool,
    pub answer: String,
    pub provider_used: Option<String>,
    pub model_used: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct QuickAskAnswerErrorPayload {
    pub ok: bool,
    pub error: String,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
#[serde(untagged)]
pub enum QuickAskAnswerPayload {
    Ok(QuickAskAnswerOkPayload),
    Err(QuickAskAnswerErrorPayload),
}

#[derive(Debug, Clone, Copy, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStateEvent {
    Idle,
    Recording,
    Transcribing,
    Routing,
    Rewriting,
    Error,
}

pub type PipelineTranscriptReadyPayload = String;
pub type EmptyEventPayload = ();
pub type SettingsChangedPayload = std::collections::BTreeMap<String, serde_json::Value>;

#[derive(Debug, Clone, Copy, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStateEvent {
    Disconnected,
    Connecting,
    Idle,
    Recording,
    Processing,
}

#[derive(Debug, Clone, serde::Serialize, JsonSchema)]
pub struct ConnectionStateChangedPayload {
    pub state: ConnectionStateEvent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_preview_events_have_distinct_kinds_and_preserve_epoch_and_rtl_text() {
        let started =
            serde_json::to_value(SttPartialTranscriptPayload::Started { session_epoch: 17 })
                .unwrap();
        assert_eq!(
            started,
            serde_json::json!({"kind":"started","session_epoch":17})
        );
        assert!(started.get("text").is_none());
        let text = "שלום\n  \"וועלט\" — hello!";
        let partial = serde_json::to_value(SttPartialTranscriptPayload::Partial {
            session_epoch: 17,
            text: text.into(),
        })
        .unwrap();
        assert_eq!(
            partial,
            serde_json::json!({"kind":"partial","session_epoch":17,"text":text})
        );
        assert_eq!(partial["text"].as_str(), Some(text));
    }
}
