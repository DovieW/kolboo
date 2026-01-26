pub fn normalize_exe_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.replace('\\', "/").to_lowercase())
}

pub fn app_identity_key(exe_path: Option<&str>) -> Option<String> {
    exe_path.and_then(normalize_exe_path)
}
