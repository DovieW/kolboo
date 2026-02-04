use serde_json::Value;
use tauri_plugin_cli::Matches;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliOutputFormat {
    Json,
    Human,
}

impl CliOutputFormat {
    pub(crate) fn from_arg_value(value: Option<&Value>) -> Self {
        match value.and_then(|v| v.as_str()) {
            Some("human") => CliOutputFormat::Human,
            _ => CliOutputFormat::Json,
        }
    }
}

pub(crate) fn output_format_from(matches: &Matches) -> CliOutputFormat {
    let value = matches.args.get("output").map(|arg| &arg.value);
    CliOutputFormat::from_arg_value(value)
}

pub(crate) fn arg_string(matches: &Matches, name: &str) -> Option<String> {
    let value = matches.args.get(name).map(|arg| &arg.value)?;
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        _ => Some(value.to_string()),
    }
}
