use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Serialize)]
pub struct CommandError {
    pub message: Box<str>,
    pub error_type: Box<str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<Box<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Box<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<Box<str>>,
}

impl CommandError {
    pub fn new(message: impl Into<String>, error_type: impl Into<String>) -> Self {
        Self {
            message: message.into().into_boxed_str(),
            error_type: error_type.into().into_boxed_str(),
            code: None,
            details: None,
            retryable: None,
            request_id: None,
        }
    }

    pub fn unknown(message: impl Into<String>) -> Self {
        Self::new(message, "unknown")
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into().into_boxed_str());
        self
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into().into_boxed_str());
        self
    }

    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = Some(retryable);
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into().into_boxed_str());
        self
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self::unknown(message)
    }
}

impl From<&str> for CommandError {
    fn from(message: &str) -> Self {
        Self::unknown(message)
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

pub type CommandResult<T> = Result<T, CommandError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_error_new_sets_message_and_type() {
        let error = CommandError::new("test error", "llm");
        assert_eq!(error.message.as_ref(), "test error");
        assert_eq!(error.error_type.as_ref(), "llm");
        assert_eq!(error.code, None);
        assert_eq!(error.details, None);
        assert_eq!(error.retryable, None);
        assert_eq!(error.request_id, None);
    }

    #[test]
    fn command_error_from_string_defaults_unknown() {
        let error: CommandError = "test error".to_string().into();
        assert_eq!(error.message.as_ref(), "test error");
        assert_eq!(error.error_type.as_ref(), "unknown");
    }

    #[test]
    fn command_error_display_uses_message() {
        let error = CommandError::new("hello", "test");
        assert_eq!(format!("{}", error), "hello");
    }

    #[test]
    fn command_error_builder_sets_optional_fields() {
        let error = CommandError::new("message", "test")
            .with_code("E_TEST")
            .with_details("more info")
            .with_retryable(true)
            .with_request_id("req_123");
        assert_eq!(error.code.as_deref(), Some("E_TEST"));
        assert_eq!(error.details.as_deref(), Some("more info"));
        assert_eq!(error.retryable, Some(true));
        assert_eq!(error.request_id.as_deref(), Some("req_123"));
    }
}
