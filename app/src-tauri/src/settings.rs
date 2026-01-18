use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::llm::PromptSections;

// ============================================================================
// Network / proxy settings (stored in settings.json)
// ============================================================================

/// Proxy mode for outgoing HTTP requests.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    /// Force-disable any proxy usage (ignore env/system proxies).
    NoProxy,
    /// Use system defaults (env vars / OS proxy discovery).
    #[default]
    System,
    /// Use a user-specified proxy URL.
    Manual,
}

/// Manual proxy configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ManualProxySettings {
    /// Proxy URL (applied to both http + https). Example: "http://127.0.0.1:8080".
    #[serde(default)]
    pub proxy_url: String,
    /// Comma-separated or whitespace-separated host list to bypass the proxy.
    /// Mirrors NO_PROXY semantics.
    #[serde(default)]
    pub no_proxy: String,
    /// Optional username for basic proxy auth.
    #[serde(default)]
    pub username: String,
    /// Optional password for basic proxy auth.
    #[serde(default)]
    pub password: String,
}

impl Default for ManualProxySettings {
    fn default() -> Self {
        Self {
            proxy_url: String::new(),
            // Common proxy bypass defaults.
            no_proxy: "localhost,127.0.0.1".to_string(),
            username: String::new(),
            password: String::new(),
        }
    }
}

/// Persistent proxy settings.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrustedCaCertFormat {
    #[default]
    Pem,
    Der,
}

/// A user-provided CA certificate that should be trusted for outgoing HTTPS.
///
/// Stored in settings.json so it can be applied by reqwest at runtime.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TrustedCaCertificate {
    /// Stable ID for list operations in the UI.
    #[serde(default)]
    pub id: String,
    /// Original filename (for display only).
    #[serde(default)]
    pub file_name: String,
    /// Encoding format used by reqwest when loading this certificate.
    #[serde(default)]
    pub format: TrustedCaCertFormat,
    /// Raw certificate bytes, base64-encoded.
    #[serde(default)]
    pub data_base64: String,
}

/// Persistent proxy settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ProxySettings {
    #[serde(default)]
    pub mode: ProxyMode,
    #[serde(default)]
    pub manual: ManualProxySettings,

    /// Additional trusted CA certificates for HTTPS requests.
    /// Prefer this over disabling TLS verification.
    #[serde(default)]
    pub trusted_ca_certificates: Vec<TrustedCaCertificate>,

    /// DANGEROUS: if enabled, accept invalid TLS certificates (e.g. self-signed).
    /// This weakens security and should only be used when required by your network.
    #[serde(default)]
    pub danger_accept_invalid_certs: bool,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            mode: ProxyMode::System,
            manual: ManualProxySettings::default(),
            trusted_ca_certificates: Vec::new(),
            danger_accept_invalid_certs: false,
        }
    }
}

#[cfg(desktop)]
use tauri_plugin_global_shortcut::Shortcut;

// ============================================================================
// DEFAULT HOTKEY CONSTANTS - Single source of truth for all default hotkeys
// ============================================================================

/// Default modifiers for all hotkeys
pub const DEFAULT_HOTKEY_MODIFIERS: &[&str] = &[];

/// Default key for toggle recording.
///
/// - Windows: modifier-only hotkey (Right Alt / AltGr) handled by the low-level hook.
/// - Other: F3 (portable, supported by tauri-plugin-global-shortcut).
#[cfg(target_os = "windows")]
pub const DEFAULT_TOGGLE_KEY: &str = "AltRight";

#[cfg(not(target_os = "windows"))]
pub const DEFAULT_TOGGLE_KEY: &str = "F3";

// ============================================================================
// DEFAULT VAD SETTINGS - Voice Activity Detection
// ============================================================================

/// Default VAD enabled state
pub const DEFAULT_VAD_ENABLED: bool = false;

/// Default VAD auto-stop enabled state
pub const DEFAULT_VAD_AUTO_STOP: bool = false;

/// Default VAD aggressiveness level (0-3, higher = more aggressive filtering)
pub const DEFAULT_VAD_AGGRESSIVENESS: u8 = 2;

/// Default speech frames threshold before triggering speech start
pub const DEFAULT_VAD_SPEECH_FRAMES_THRESHOLD: u32 = 3;

/// Default hangover frames (silence frames before triggering speech end)
pub const DEFAULT_VAD_HANGOVER_FRAMES: u32 = 30;

/// Default pre-roll milliseconds to capture before speech is detected
pub const DEFAULT_VAD_PRE_ROLL_MS: u32 = 300;

// ============================================================================

/// Configuration for a hotkey combination
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct HotkeyConfig {
    /// Modifier keys (e.g., ["ctrl", "alt"])
    pub modifiers: Vec<String>,
    /// The main key (e.g., "Space")
    pub key: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            modifiers: DEFAULT_HOTKEY_MODIFIERS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            key: DEFAULT_TOGGLE_KEY.to_string(),
        }
    }
}

impl HotkeyConfig {
    /// Create default toggle hotkey config
    pub fn default_toggle() -> Self {
        Self {
            modifiers: DEFAULT_HOTKEY_MODIFIERS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            key: DEFAULT_TOGGLE_KEY.to_string(),
        }
    }

    /// Default toggle hotkey as a setting value (Some = enabled).
    pub fn default_toggle_opt() -> Option<Self> {
        Some(Self::default_toggle())
    }

    /// Default hold-to-record hotkey as a setting value.
    ///
    /// Hold-to-record is disabled by default.
    pub fn default_hold() -> Option<Self> {
        None
    }

    /// Default paste-last hotkey as a setting value.
    ///
    /// Paste-last is disabled by default.
    pub fn default_paste_last() -> Option<Self> {
        None
    }

    /// Default retry-last-recording hotkey as a setting value.
    ///
    /// Retry is disabled by default.
    pub fn default_retry() -> Option<Self> {
        None
    }

    /// Default quick-ask hotkey as a setting value.
    ///
    /// Quick Ask is disabled by default.
    pub fn default_quick_ask() -> Option<Self> {
        None
    }

    /// Convert to shortcut string format like "ctrl+alt+Space"
    /// Note: modifiers must be lowercase for the parser to recognize them
    pub fn to_shortcut_string(&self) -> String {
        let mut parts: Vec<String> = self.modifiers.iter().map(|m| m.to_lowercase()).collect();
        parts.push(self.key.clone());
        parts.join("+")
    }

    /// Convert to a tauri Shortcut using FromStr parsing
    #[cfg(desktop)]
    pub fn to_shortcut(&self) -> Result<Shortcut, String> {
        let shortcut_str = self.to_shortcut_string();
        Shortcut::from_str(&shortcut_str)
            .map_err(|e| format!("Failed to parse shortcut '{}': {:?}", shortcut_str, e))
    }

    /// Convert to a tauri Shortcut, falling back to a default if parsing fails
    #[cfg(desktop)]
    #[allow(dead_code)]
    pub fn to_shortcut_or_default(&self, default_fn: fn() -> Self) -> Shortcut {
        self.to_shortcut().unwrap_or_else(|_| {
            default_fn()
                .to_shortcut()
                .expect("Default hotkey must be valid")
        })
    }
}

/// Voice Activity Detection settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VadSettings {
    /// Enable VAD processing
    pub enabled: bool,
    /// Automatically stop recording when speech ends
    pub auto_stop: bool,
    /// VAD aggressiveness level (0-3)
    pub aggressiveness: u8,
    /// Consecutive speech frames before triggering speech start
    pub speech_frames_threshold: u32,
    /// Consecutive silence frames before triggering speech end
    pub hangover_frames: u32,
    /// Milliseconds of audio to capture before speech is detected
    pub pre_roll_ms: u32,
}

impl Default for VadSettings {
    fn default() -> Self {
        Self {
            enabled: DEFAULT_VAD_ENABLED,
            auto_stop: DEFAULT_VAD_AUTO_STOP,
            aggressiveness: DEFAULT_VAD_AGGRESSIVENESS,
            speech_frames_threshold: DEFAULT_VAD_SPEECH_FRAMES_THRESHOLD,
            hangover_frames: DEFAULT_VAD_HANGOVER_FRAMES,
            pre_roll_ms: DEFAULT_VAD_PRE_ROLL_MS,
        }
    }
}

impl VadSettings {
    /// Convert to audio capture VAD config
    pub fn to_vad_auto_stop_config(&self) -> crate::audio_capture::VadAutoStopConfig {
        use crate::audio_capture::VadAutoStopConfig;
        use crate::vad::{VadAggressiveness, VadConfig};

        VadAutoStopConfig {
            enabled: self.enabled,
            auto_stop: self.auto_stop,
            vad_config: VadConfig {
                aggressiveness: match self.aggressiveness {
                    0 => VadAggressiveness::Quality,
                    1 => VadAggressiveness::LowBitrate,
                    2 => VadAggressiveness::Aggressive,
                    _ => VadAggressiveness::VeryAggressive,
                },
                speech_frames_threshold: self.speech_frames_threshold,
                hangover_frames: self.hangover_frames,
                pre_roll_ms: self.pre_roll_ms,
                frame_duration_ms: 10, // webrtc-vad supports 10/20/30ms; 10ms matches our defaults
                sample_rate: 16000,    // Current implementation runs webrtc-vad at 16kHz
            },
        }
    }
}

// ============================================================================
// Rewrite prompt settings (stored in settings.json)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PromptSectionSetting {
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CleanupPromptSectionsSetting {
    /// System prompt override. When missing/None, inherit from base prompts.
    #[serde(default)]
    pub system: Option<PromptSectionSetting>,
}

impl CleanupPromptSectionsSetting {
    /// Apply these overrides on top of a base `PromptSections`.
    ///
    /// - Any missing section inherits from `base`.
    /// - `content: None` means "use built-in default system prompt".
    pub fn apply_to(&self, base: &PromptSections) -> PromptSections {
        let mut next = base.clone();

        if let Some(system) = &self.system {
            next.system_custom = system.content.clone();
        }

        next
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RewriteProgramPromptProfile {
    pub id: String,
    pub name: String,
    #[serde(
        default,
        alias = "program_path",
        deserialize_with = "deserialize_program_paths"
    )]
    pub program_paths: Vec<String>,
    pub cleanup_prompt_sections: Option<CleanupPromptSectionsSetting>,

    // Presets/modes within this program profile.
    // Default empty list keeps older settings.json compatible.
    // Some historical frontend versions wrote `presets: null`, which would
    // otherwise fail to deserialize and cause *all* profiles to be dropped.
    #[serde(default, deserialize_with = "deserialize_null_to_default_vec")]
    pub presets: Vec<RewritePreset>,
    /// Default preset used when routing is off/undecided.
    #[serde(default)]
    pub default_preset_id: Option<String>,
    /// Description for the implicit "Default" (no preset) routing target.
    #[serde(default)]
    pub default_preset_description: Option<String>,
    /// Per-profile gate for the rewrite step when routed to the implicit "Default" target
    /// (i.e., no preset selected).
    ///
    /// This is independent of per-preset rewrite gates. The global/per-profile rewrite gate
    /// remains a hard gate that can disable rewrite for *all* presets.
    #[serde(default = "default_true")]
    pub default_target_rewrite_llm_enabled: bool,
    /// Persisted manual selection (if set, can be used as an override for routing).
    #[serde(default)]
    pub active_preset_id: Option<String>,
    /// Optional intent router configuration.
    #[serde(default)]
    pub router: Option<IntentRouterSettings>,

    /// Optional per-profile gate for the rewrite step (falls back to global setting)
    #[serde(default)]
    pub rewrite_llm_enabled: Option<bool>,

    #[serde(default)]
    pub stt_provider: Option<String>,
    #[serde(default)]
    pub stt_model: Option<String>,
    #[serde(default)]
    pub stt_timeout_seconds: Option<f64>,
    #[serde(default)]
    pub llm_provider: Option<String>,
    #[serde(default)]
    pub llm_model: Option<String>,

    // Optional per-profile provider-specific thinking/reasoning knobs.
    // These are applied at runtime when the profile is active.
    #[serde(default)]
    pub openai_reasoning_effort: Option<String>,
    #[serde(default)]
    pub gemini_thinking_budget: Option<i64>,
    #[serde(default)]
    pub gemini_thinking_level: Option<String>,
    #[serde(default)]
    pub anthropic_thinking_budget: Option<i64>,

    // Quick Ask (per-profile overrides)
    #[serde(default)]
    pub quick_ask_provider: Option<String>,
    #[serde(default)]
    pub quick_ask_model: Option<String>,
    #[serde(default)]
    pub quick_ask_system_prompt: Option<String>,

    /// Context grabbing method for highlighted-text capture.
    ///
    /// Controls what shortcut Kolboo uses to copy the current selection when probing.
    /// Expected values:
    /// - "none" (disable selection capture)
    /// - "ctrl_c" (default)
    /// - "ctrl_shift_c" (common in terminals on Windows)
    /// - "ctrl_insert" (alternative terminal-friendly copy on Windows)
    #[serde(default)]
    pub context_grab_method: Option<String>,

    // Clipboard context (per-profile toggles)
    // When enabled, Kolboo will read the current clipboard text and include it as optional
    // context when constructing LLM inputs.
    //
    // NOTE: These toggles are independent of `context_grab_method` (highlighted-selection
    // capture). Clipboard context never injects keys.
    #[serde(default)]
    pub rewrite_include_clipboard_context: Option<bool>,
    #[serde(default)]
    pub quick_replace_include_clipboard_context: Option<bool>,
    #[serde(default)]
    pub quick_ask_include_clipboard_context: Option<bool>,

    // Quick Replace (per-profile overrides)
    // When enabled and there is highlighted text when transcription starts, treat the transcript
    // as an instruction to rewrite the selected text.
    #[serde(default)]
    pub quick_replace_enabled: Option<bool>,
    #[serde(default)]
    pub quick_replace_provider: Option<String>,
    #[serde(default)]
    pub quick_replace_model: Option<String>,
    #[serde(default)]
    pub quick_replace_system_prompt: Option<String>,

    // Optional per-profile provider-specific thinking/reasoning knobs for Quick Ask.
    #[serde(default)]
    pub quick_ask_openai_reasoning_effort: Option<String>,
    #[serde(default)]
    pub quick_ask_gemini_thinking_budget: Option<i64>,
    #[serde(default)]
    pub quick_ask_gemini_thinking_level: Option<String>,
    #[serde(default)]
    pub quick_ask_anthropic_thinking_budget: Option<i64>,

    // Per-profile overrides for UI (stored in settings.json).
    #[serde(default)]
    pub sound_enabled: Option<bool>,
    #[serde(default)]
    pub playing_audio_handling: Option<String>,
    #[serde(default)]
    pub overlay_mode: Option<String>,
    #[serde(default)]
    pub widget_position: Option<String>,
    #[serde(default)]
    pub output_mode: Option<String>,
    #[serde(default)]
    pub output_hit_enter: Option<bool>,
}

// ============================================================================
// Presets + intent router (stored in settings.json)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RewritePreset {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,

    /// Example utterances / short hints used by the intent router.
    // Some historical frontend versions wrote `routing_hints: null`.
    #[serde(default, deserialize_with = "deserialize_null_to_default_vec")]
    pub routing_hints: Vec<String>,

    #[serde(default)]
    pub cleanup_prompt_sections: Option<CleanupPromptSectionsSetting>,

    /// Explicit per-preset gate for the rewrite step.
    ///
    /// Semantics:
    /// - Missing/null in legacy settings => defaults to true (backward compatible)
    /// - Does NOT override the global or per-profile rewrite gate (those are hard gates)
    #[serde(
        default = "default_true",
        deserialize_with = "deserialize_null_to_default_true_bool"
    )]
    pub rewrite_llm_enabled: bool,

    #[serde(default)]
    pub stt_provider: Option<String>,
    #[serde(default)]
    pub stt_model: Option<String>,
    #[serde(default)]
    pub stt_timeout_seconds: Option<f64>,
    #[serde(default)]
    pub llm_provider: Option<String>,
    #[serde(default)]
    pub llm_model: Option<String>,

    #[serde(default)]
    pub openai_reasoning_effort: Option<String>,
    #[serde(default)]
    pub gemini_thinking_budget: Option<i64>,
    #[serde(default)]
    pub gemini_thinking_level: Option<String>,
    #[serde(default)]
    pub anthropic_thinking_budget: Option<i64>,

    // Optional per-preset UI overrides (stored in settings.json).
    #[serde(default)]
    pub sound_enabled: Option<bool>,
    #[serde(default)]
    pub playing_audio_handling: Option<String>,
    #[serde(default)]
    pub overlay_mode: Option<String>,
    #[serde(default)]
    pub widget_position: Option<String>,
    #[serde(default)]
    pub output_mode: Option<String>,
    #[serde(default)]
    pub output_hit_enter: Option<bool>,
}

fn deserialize_null_to_default_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

fn deserialize_null_to_default_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<bool>::deserialize(deserializer)?.unwrap_or(false))
}

fn default_true() -> bool {
    true
}

fn deserialize_null_to_default_true_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<bool>::deserialize(deserializer)?.unwrap_or(true))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum IntentRouterStrategy {
    #[default]
    Off,
    Embeddings,
    Llm,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct IntentRouterSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub strategy: IntentRouterStrategy,

    // Embeddings routing knobs
    #[serde(default)]
    pub embedding_provider: Option<String>,
    #[serde(default)]
    pub embedding_model: Option<String>,

    /// If true, always pick the candidate with the highest similarity score.
    /// When enabled, threshold/margin are ignored.
    ///
    /// Some historical frontend versions may write this as null; tolerate it.
    #[serde(default, deserialize_with = "deserialize_null_to_default_bool")]
    pub pick_highest_score: bool,

    #[serde(default)]
    pub similarity_threshold: Option<f32>,
    #[serde(default)]
    pub similarity_margin: Option<f32>,

    // LLM routing knobs
    #[serde(default)]
    pub llm_provider: Option<String>,
    #[serde(default)]
    pub llm_model: Option<String>,

    // Optional per-router provider-specific thinking/reasoning knobs.
    #[serde(default)]
    pub openai_reasoning_effort: Option<String>,
    #[serde(default)]
    pub gemini_thinking_budget: Option<i64>,
    #[serde(default)]
    pub gemini_thinking_level: Option<String>,
    #[serde(default)]
    pub anthropic_thinking_budget: Option<i64>,

    // Advanced: optional override for the router system prompt.
    #[serde(default)]
    pub llm_system_prompt: Option<String>,
}

fn deserialize_program_paths<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }

    let value = Option::<OneOrMany>::deserialize(deserializer)?;
    let paths = match value {
        None => Vec::new(),
        Some(OneOrMany::One(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Vec::new()
            } else {
                vec![trimmed.to_string()]
            }
        }
        Some(OneOrMany::Many(v)) => v
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    };

    Ok(paths)
}
