//! Small helpers for building HTTP endpoint URLs consistently across providers.
//!
//! We intentionally keep this string-based (instead of `url::Url`) because:
//! - many provider base URLs include their own path segments (e.g. `/v1beta`)
//! - most provider code already stores endpoint URLs as strings
//! - we only need predictable joining/normalization for simple path appends

/// Trim trailing slashes from a base URL.
pub fn trim_base_url(base_url: &str) -> &str {
    // rustfmt will normalize indentation; keep logic tiny and predictable.
    base_url.trim_end_matches('/')
}

/// Build a URL by joining a base URL with a path.
///
/// - `base_url` may have trailing slashes.
/// - `path` may be prefixed with `/`.
pub fn join_base_url(base_url: &str, path: &str) -> String {
    let base = trim_base_url(base_url);
    let path_raw = path.trim_start_matches('/');

    // Convenience for OpenAI-ish base URLs:
    // Users often paste base URLs that already include `/v1`.
    // Many callsites also append paths that start with `/v1/...`.
    // Avoid generating `.../v1/v1/...` in that common case.
    if base.ends_with("/v1") && path_raw.starts_with("v1/") {
        let without_dup = path_raw.trim_start_matches("v1/");
        return format!("{}/{}", base, without_dup);
    }

    format!("{}/{}", base, path_raw)
}

/// Convenience helper to grab `(status, body_text)` from a `reqwest::Response`.
pub async fn status_and_text(resp: reqwest::Response) -> (reqwest::StatusCode, String) {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

/// Convenience helper for parsing JSON from an HTTP response body.
///
/// We keep this in `http` so providers can share the same parse-error wording.
pub fn parse_json_value(body: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(body).map_err(|e| format!("Failed to parse response JSON: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_trailing_slashes() {
        assert_eq!(trim_base_url("https://example.com"), "https://example.com");
        assert_eq!(trim_base_url("https://example.com/"), "https://example.com");
        assert_eq!(
            trim_base_url("https://example.com///"),
            "https://example.com"
        );
    }

    #[test]
    fn joins_base_url_and_path() {
        assert_eq!(
            join_base_url("https://example.com", "v1/audio/transcriptions"),
            "https://example.com/v1/audio/transcriptions"
        );
        assert_eq!(
            join_base_url("https://example.com/", "/v1/audio/transcriptions"),
            "https://example.com/v1/audio/transcriptions"
        );

        // If the user includes `/v1` in the base URL, don't duplicate it.
        assert_eq!(
            join_base_url("https://example.com/v1", "/v1/chat/completions"),
            "https://example.com/v1/chat/completions"
        );
    }

    #[test]
    fn parses_json_value_or_returns_message() {
        let ok = parse_json_value(r#"{"a": 1}"#).unwrap();
        assert_eq!(ok.get("a").and_then(|v| v.as_i64()), Some(1));

        let err = parse_json_value("not json").unwrap_err();
        assert!(err.starts_with("Failed to parse response JSON:"));
    }
}
