#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
    #[error("validation error: {0}")]
    Validation(String),
    #[error("runtime error: {0}")]
    Runtime(String),
}

impl CliError {
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            CliError::Validation(_) => 2,
            CliError::Runtime(_) => 3,
        }
    }
}
