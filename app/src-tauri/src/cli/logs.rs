use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tauri::AppHandle;
use tauri_plugin_cli::Matches;

use crate::cli::{
    arg_string, arg_u64, output_format_from, CliError, CliOutputFormat, CommandResult,
};
use crate::request_log::strip_request_log_text_and_payloads;

pub(crate) fn handle_logs(
    app: &AppHandle,
    matches: &Matches,
) -> Result<(CliOutputFormat, CommandResult<Value>), CliError> {
    let Some(subcommand) = matches.subcommand.as_ref() else {
        return Err(CliError::Validation("Missing logs subcommand".to_string()));
    };

    match subcommand.name.as_str() {
        "request" => handle_request_logs(app, &subcommand.matches),
        "app" => handle_app_logs(app, &subcommand.matches),
        _ => Err(CliError::Validation(format!(
            "Unknown logs subcommand: {}",
            subcommand.name
        ))),
    }
}

fn handle_request_logs(
    app: &AppHandle,
    matches: &Matches,
) -> Result<(CliOutputFormat, CommandResult<Value>), CliError> {
    let Some(subcommand) = matches.subcommand.as_ref() else {
        return Err(CliError::Validation(
            "Missing logs request subcommand".to_string(),
        ));
    };

    match subcommand.name.as_str() {
        "list" => {
            let output_format = output_format_from(&subcommand.matches);
            let limit = arg_u64(&subcommand.matches, "limit")
                .unwrap_or(50)
                .clamp(1, 200) as usize;
            let strip_text = arg_bool(&subcommand.matches, "strip_text");

            let mut logs = crate::commands::logs::get_request_logs(app.clone(), Some(limit));
            if strip_text {
                logs = logs
                    .into_iter()
                    .map(strip_request_log_text_and_payloads)
                    .collect();
            }

            let count = logs.len();
            Ok((
                output_format,
                CommandResult::success(
                    Some(json!({
                        "count": count,
                        "limit": limit,
                        "strip_text": strip_text,
                        "logs": logs,
                    })),
                    Some(format!("Loaded {count} request logs")),
                ),
            ))
        }
        "clear" => {
            let output_format = output_format_from(&subcommand.matches);
            crate::commands::logs::clear_request_logs(app.clone());
            Ok((
                output_format,
                CommandResult::success(None, Some("Request logs cleared".to_string())),
            ))
        }
        "export" => {
            let output_format = output_format_from(&subcommand.matches);
            let file = arg_string(&subcommand.matches, "file")
                .ok_or_else(|| CliError::Validation("Missing --file".to_string()))?;
            let limit = arg_u64(&subcommand.matches, "limit").map(|v| v.clamp(1, 200) as usize);
            let strip_text = arg_bool(&subcommand.matches, "strip_text");

            crate::commands::logs::export_request_logs_to_file(
                app.clone(),
                file.clone(),
                limit,
                strip_text,
            )
            .map_err(CliError::Runtime)?;

            Ok((
                output_format,
                CommandResult::success(
                    Some(json!({
                        "file": file,
                        "limit": limit,
                        "strip_text": strip_text,
                    })),
                    Some("Request logs exported".to_string()),
                ),
            ))
        }
        _ => Err(CliError::Validation(format!(
            "Unknown logs request subcommand: {}",
            subcommand.name
        ))),
    }
}

fn handle_app_logs(
    _app: &AppHandle,
    matches: &Matches,
) -> Result<(CliOutputFormat, CommandResult<Value>), CliError> {
    let Some(subcommand) = matches.subcommand.as_ref() else {
        return Err(CliError::Validation(
            "Missing logs app subcommand".to_string(),
        ));
    };

    match subcommand.name.as_str() {
        "dir" => {
            let output_format = output_format_from(&subcommand.matches);
            let dir = app_logs_dir()?;
            Ok((
                output_format,
                CommandResult::success(
                    Some(json!({ "dir": dir })),
                    Some("Resolved app logs directory".to_string()),
                ),
            ))
        }
        "list" => {
            let output_format = output_format_from(&subcommand.matches);
            let limit = arg_u64(&subcommand.matches, "limit")
                .unwrap_or(20)
                .clamp(1, 200) as usize;
            let dir = PathBuf::from(app_logs_dir()?);

            let files = list_log_files(&dir, Some(limit))?;
            let payload_files: Vec<Value> = files
                .iter()
                .map(|f| {
                    json!({
                        "name": f.name,
                        "path": f.path.to_string_lossy().to_string(),
                        "size_bytes": f.size_bytes,
                        "modified_unix_ms": f.modified_unix_ms,
                    })
                })
                .collect();

            let count = payload_files.len();
            Ok((
                output_format,
                CommandResult::success(
                    Some(json!({
                        "dir": dir.to_string_lossy().to_string(),
                        "count": count,
                        "files": payload_files,
                    })),
                    Some(format!("Found {count} app log files")),
                ),
            ))
        }
        "show" => {
            let output_format = output_format_from(&subcommand.matches);
            let lines = arg_u64(&subcommand.matches, "lines")
                .unwrap_or(200)
                .clamp(1, 5000) as usize;
            let file_arg = arg_string(&subcommand.matches, "file");

            let dir = PathBuf::from(app_logs_dir()?);
            let path = resolve_log_file_path(&dir, file_arg.as_deref())?;

            let content = std::fs::read(&path)
                .map_err(|err| CliError::Runtime(format!("Failed to read log file: {err}")))?;
            let text = String::from_utf8_lossy(&content).to_string();
            let (tail, total_lines, truncated) = tail_lines(&text, lines);

            Ok((
                output_format,
                CommandResult::success(
                    Some(json!({
                        "dir": dir.to_string_lossy().to_string(),
                        "file": path.to_string_lossy().to_string(),
                        "requested_lines": lines,
                        "total_lines": total_lines,
                        "truncated": truncated,
                        "content": tail,
                    })),
                    Some(format!(
                        "Loaded {} lines from {}",
                        if truncated { lines } else { total_lines },
                        path.display()
                    )),
                ),
            ))
        }
        _ => Err(CliError::Validation(format!(
            "Unknown logs app subcommand: {}",
            subcommand.name
        ))),
    }
}

fn arg_bool(matches: &Matches, name: &str) -> bool {
    let Some(value) = matches.args.get(name).map(|arg| &arg.value) else {
        return false;
    };

    match value {
        Value::Bool(v) => *v,
        Value::String(v) => {
            let lowered = v.trim().to_lowercase();
            matches!(lowered.as_str(), "1" | "true" | "yes" | "on")
        }
        Value::Number(v) => v.as_u64().unwrap_or(0) > 0,
        _ => false,
    }
}

fn app_logs_dir() -> Result<String, CliError> {
    crate::commands::logs::get_app_logs_dir()
        .ok_or_else(|| CliError::Runtime("App log directory is unavailable".to_string()))
}

#[derive(Debug, Clone)]
struct LogFileMeta {
    name: String,
    path: PathBuf,
    size_bytes: u64,
    modified_unix_ms: Option<u64>,
}

fn list_log_files(dir: &Path, limit: Option<usize>) -> Result<Vec<LogFileMeta>, CliError> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(dir)
        .map_err(|err| CliError::Runtime(format!("Failed to read logs directory: {err}")))?;

    for entry in entries {
        let entry = entry
            .map_err(|err| CliError::Runtime(format!("Failed to read log file entry: {err}")))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|err| CliError::Runtime(format!("Failed to read file metadata: {err}")))?;

        if !metadata.is_file() {
            continue;
        }

        let modified_unix_ms = metadata
            .modified()
            .ok()
            .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64);

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(ToString::to_string)
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        files.push(LogFileMeta {
            name,
            path,
            size_bytes: metadata.len(),
            modified_unix_ms,
        });
    }

    files.sort_by(|a, b| {
        b.modified_unix_ms
            .cmp(&a.modified_unix_ms)
            .then_with(|| a.name.cmp(&b.name))
    });

    if let Some(limit) = limit {
        files.truncate(limit);
    }

    Ok(files)
}

fn resolve_log_file_path(log_dir: &Path, file_arg: Option<&str>) -> Result<PathBuf, CliError> {
    if let Some(file_arg) = file_arg {
        let candidate = PathBuf::from(file_arg);
        if candidate.is_absolute() {
            return Ok(candidate);
        }
        return Ok(log_dir.join(candidate));
    }

    let latest = list_log_files(log_dir, Some(1))?
        .into_iter()
        .next()
        .ok_or_else(|| CliError::Runtime("No app log files found".to_string()))?;
    Ok(latest.path)
}

fn tail_lines(input: &str, max_lines: usize) -> (String, usize, bool) {
    if input.is_empty() {
        return (String::new(), 0, false);
    }

    let lines: Vec<&str> = input.lines().collect();
    let total_lines = lines.len();
    let truncated = total_lines > max_lines;
    let start = if truncated {
        total_lines.saturating_sub(max_lines)
    } else {
        0
    };

    (lines[start..].join("\n"), total_lines, truncated)
}

#[cfg(test)]
mod tests {
    use super::{list_log_files, tail_lines};

    fn make_temp_dir(name: &str) -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        dir.push(format!("kolboo-cli-logs-{name}-{unique}"));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn tail_lines_returns_last_n_lines() {
        let input = "a\nb\nc\nd";
        let (tail, total, truncated) = tail_lines(input, 2);
        assert_eq!(tail, "c\nd");
        assert_eq!(total, 4);
        assert!(truncated);
    }

    #[test]
    fn list_log_files_orders_newest_first() {
        let dir = make_temp_dir("order");
        let older = dir.join("older.log");
        let newer = dir.join("newer.log");

        std::fs::write(&older, "old").expect("write older");
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&newer, "new").expect("write newer");

        let files = list_log_files(&dir, None).expect("list files");
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "newer.log");
        assert_eq!(files[1].name, "older.log");

        let _ = std::fs::remove_dir_all(dir);
    }
}
