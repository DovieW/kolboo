use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct CommandResult<T>
where
    T: Serialize,
{
    pub(crate) success: bool,
    pub(crate) code: i32,
    pub(crate) message: Option<String>,
    pub(crate) warnings: Vec<String>,
    pub(crate) data: Option<T>,
}

impl<T> CommandResult<T>
where
    T: Serialize,
{
    pub(crate) fn success(data: Option<T>, message: Option<String>) -> Self {
        Self {
            success: true,
            code: 0,
            message,
            warnings: Vec::new(),
            data,
        }
    }

    pub(crate) fn failure(code: i32, message: String) -> Self {
        Self {
            success: false,
            code,
            message: Some(message),
            warnings: Vec::new(),
            data: None,
        }
    }
}

pub(crate) fn write_json<T>(result: &CommandResult<T>) -> std::io::Result<()>
where
    T: Serialize,
{
    let payload = serde_json::to_string(result).unwrap_or_else(|_| {
        "{\"success\":false,\"code\":3,\"message\":\"failed to serialize output\"}".to_string()
    });
    println!("{}", payload);
    Ok(())
}

pub(crate) fn write_human(message: &str) {
    println!("{}", message);
}
