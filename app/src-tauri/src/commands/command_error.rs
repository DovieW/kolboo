use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CommandError {
    pub message: String,
    pub error_type: String,
}

impl CommandError {
    pub fn new(message: impl Into<String>, error_type: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: error_type.into(),
        }
    }

    pub fn unknown(message: impl Into<String>) -> Self {
        Self::new(message, "unknown")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_error_new_sets_message_and_type() {
        let error = CommandError::new("test error", "llm");
        assert_eq!(error.message, "test error");
        assert_eq!(error.error_type, "llm");
    }

    #[test]
    fn command_error_from_string_defaults_unknown() {
        let error: CommandError = "test error".to_string().into();
        assert_eq!(error.message, "test error");
        assert_eq!(error.error_type, "unknown");
    }
}
