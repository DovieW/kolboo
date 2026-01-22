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
pub struct OverlayAudioLevelPayload {
    pub seq: u64,
    pub rms: f32,
    pub peak: f32,
    pub wave_seq: Option<u64>,
    pub mins: Option<Vec<f32>>,
    pub maxes: Option<Vec<f32>>,
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
