pub(crate) fn normalize_language_setting(raw: Option<String>) -> Option<String> {
    raw.and_then(|value| {
        let trimmed = value.trim().to_lowercase();
        if trimmed.is_empty() || trimmed == "auto" {
            None
        } else {
            Some(trimmed)
        }
    })
}

pub(crate) fn normalize_language_code(raw: Option<String>) -> Option<String> {
    normalize_language_setting(raw)
}

pub(crate) fn normalize_language_with_detection(raw: Option<String>) -> (Option<String>, bool) {
    match normalize_language_setting(raw) {
        Some(code) => (Some(code), false),
        None => (None, true),
    }
}
