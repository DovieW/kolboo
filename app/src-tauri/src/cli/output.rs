use serde::Serialize;
use std::io::Write;

use super::types::CliOutputFormat;

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
    write_json_to(&mut std::io::stdout().lock(), result)
}

pub(crate) fn write_json_to<T, W>(writer: &mut W, result: &CommandResult<T>) -> std::io::Result<()>
where
    T: Serialize,
    W: Write,
{
    let payload = serde_json::to_string(result).unwrap_or_else(|_| {
        "{\"success\":false,\"code\":3,\"message\":\"failed to serialize output\"}".to_string()
    });
    writeln!(writer, "{}", payload)
}

pub(crate) fn write_human(message: &str) -> std::io::Result<()> {
    write_human_to(&mut std::io::stdout().lock(), message)
}

pub(crate) fn write_human_to<W>(writer: &mut W, message: &str) -> std::io::Result<()>
where
    W: Write,
{
    writeln!(writer, "{}", message)
}

pub(crate) fn write_result<T>(
    output_format: CliOutputFormat,
    result: &CommandResult<T>,
) -> std::io::Result<()>
where
    T: Serialize,
{
    match output_format {
        CliOutputFormat::Json => write_json(result),
        CliOutputFormat::Human => {
            let message = result.message.as_deref().unwrap_or("Command completed");
            write_human(message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn json_output_writes_one_envelope_line() {
        let result = CommandResult::success(Some(serde_json::json!({ "state": "idle" })), None);
        let mut out = Vec::new();

        write_json_to(&mut out, &result).expect("write json");

        let output = String::from_utf8(out).expect("utf8");
        assert!(output.ends_with('\n'));
        assert!(output.starts_with("{\"success\":true,"));
        assert!(output.contains("\"state\":\"idle\""));
    }

    #[test]
    fn json_output_falls_back_when_payload_cannot_serialize() {
        let mut bad_map = HashMap::new();
        bad_map.insert((1, 2), "not a json object key".to_string());
        let result = CommandResult::success(Some(bad_map), None);
        let mut out = Vec::new();

        write_json_to(&mut out, &result).expect("write fallback json");

        let output = String::from_utf8(out).expect("utf8");
        assert!(output.contains("\"success\":false"));
        assert!(output.contains("\"code\":3"));
        assert!(output.contains("failed to serialize output"));
    }

    #[test]
    fn human_output_writes_message_only() {
        let mut out = Vec::new();
        write_human_to(&mut out, "Command completed").expect("write human");

        assert_eq!(String::from_utf8(out).expect("utf8"), "Command completed\n");
    }
}
