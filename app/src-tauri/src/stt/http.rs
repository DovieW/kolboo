use std::time::Duration;

/// Trim trailing slashes from an API base URL.
///
/// This keeps URL joining consistent across STT providers.
pub fn trim_base_url(base_url: &str) -> &str {
    base_url.trim_end_matches('/')
}

/// Build a URL by joining a base URL with a path.
///
/// - `base_url` may have trailing slashes.
/// - `path` may be prefixed with `/`.
pub fn join_base_url(base_url: &str, path: &str) -> String {
    let base = trim_base_url(base_url);
    let path = path.trim_start_matches('/');
    format!("{}/{}", base, path)
}

/// Default STT HTTP timeout used by provider constructors.
///
/// Note: the pipeline typically injects its own client (with proxy settings) via `with_client(...)`.
/// This constant is mainly for provider `new(...)` constructors and tests.
#[allow(dead_code)]
pub const DEFAULT_STT_HTTP_TIMEOUT: Duration = Duration::from_secs(60);

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
    }
}
