//! Small helpers for building HTTP endpoint URLs consistently across providers.
//!
//! We intentionally keep this string-based (instead of `url::Url`) because:
//! - many provider base URLs include their own path segments (e.g. `/v1beta`)
//! - most provider code already stores endpoint URLs as strings
//! - we only need predictable joining/normalization for simple path appends

const CF_ACCESS_CLIENT_ID_HEADER: &str = "CF-Access-Client-Id";
const CF_ACCESS_CLIENT_SECRET_HEADER: &str = "CF-Access-Client-Secret";

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

fn non_empty_env(key: &str) -> Option<String> {
    let value = std::env::var(key).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn hostname(value: &str) -> Option<String> {
    reqwest::Url::parse(value)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_ascii_lowercase()))
}

fn cloudflare_access_target_hosts() -> Vec<String> {
    ["TAURI_API_BASE_URL", "TAURI_MANAGED_INFERENCE_GATEWAY_URL"]
        .into_iter()
        .filter_map(non_empty_env)
        .filter_map(|url| hostname(&url))
        .collect()
}

fn url_matches_configured_hosts(target_url: &str, configured_hosts: &[String]) -> bool {
    let Some(target_host) = hostname(target_url) else {
        return false;
    };

    configured_hosts
        .iter()
        .any(|host| host.eq_ignore_ascii_case(&target_host))
}

pub fn cloudflare_access_headers_for_url(target_url: &str) -> Option<(String, String)> {
    if !url_matches_configured_hosts(target_url, &cloudflare_access_target_hosts()) {
        return None;
    }

    let client_id = non_empty_env("TAURI_CLOUDFLARE_ACCESS_CLIENT_ID")?;
    let client_secret = non_empty_env("TAURI_CLOUDFLARE_ACCESS_CLIENT_SECRET")?;
    Some((client_id, client_secret))
}

/// Attach Cloudflare Access service-token headers for requests aimed at configured
/// Kolboo edge origins. This intentionally does nothing for arbitrary provider URLs
/// so the dev Access token is never sent to BYOK providers.
pub fn with_cloudflare_access_headers_if_target(
    req: reqwest::RequestBuilder,
    target_url: &str,
) -> reqwest::RequestBuilder {
    let Some((client_id, client_secret)) = cloudflare_access_headers_for_url(target_url) else {
        return req;
    };

    req.header(CF_ACCESS_CLIENT_ID_HEADER, client_id)
        .header(CF_ACCESS_CLIENT_SECRET_HEADER, client_secret)
}

pub fn with_cloudflare_access_headers_from_request_url(
    req: reqwest::RequestBuilder,
) -> reqwest::RequestBuilder {
    let target_url = req
        .try_clone()
        .and_then(|clone| clone.build().ok())
        .map(|request| request.url().to_string());

    if let Some(target_url) = target_url {
        with_cloudflare_access_headers_if_target(req, &target_url)
    } else {
        req
    }
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
    fn cloudflare_access_target_matches_configured_base_hosts() {
        let configured_hosts = vec!["kolboo.dovie.dev".to_string()];

        assert!(url_matches_configured_hosts(
            "https://kolboo.dovie.dev/v1/sync/settings",
            &configured_hosts
        ));
        assert!(!url_matches_configured_hosts(
            "https://api.openai.com/v1/responses",
            &configured_hosts
        ));
    }

    #[test]
    fn parses_json_value_or_returns_message() {
        let ok = parse_json_value(r#"{"a": 1}"#).unwrap();
        assert_eq!(ok.get("a").and_then(|v| v.as_i64()), Some(1));

        let err = parse_json_value("not json").unwrap_err();
        assert!(err.starts_with("Failed to parse response JSON:"));
    }
}
