use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowsTextContextSource {
    Uia,
    Clipboard,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowsInsertMethod {
    UiaValuePattern,
    Paste,
    Typing,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowsTextTargetSnapshot {
    pub captured_at_ms: u64,
    pub process_id: Option<u32>,
    pub exe_path: Option<String>,
    pub window_title: Option<String>,
    pub uia_runtime_id: Option<Vec<i32>>,
    pub is_password: Option<bool>,
    pub is_enabled: Option<bool>,
    pub is_read_only: Option<bool>,
    pub supports_text_pattern: bool,
    pub supports_value_pattern: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowsTextContext {
    pub selection_text: Option<String>,
    pub surrounding_text: Option<String>,
    pub source: WindowsTextContextSource,
    pub truncated: bool,
    pub max_chars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowsInsertPlan {
    pub method: WindowsInsertMethod,
    pub reason: String,
    pub allowed: bool,
}
