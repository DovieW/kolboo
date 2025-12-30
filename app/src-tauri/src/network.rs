//! Network helpers (HTTP client configuration).
//!
//! This module centralizes HTTP client construction so that settings like proxy
//! configuration can be applied consistently across all providers.

use crate::settings::{ProxyMode, ProxySettings};
use reqwest::{Client, ClientBuilder, NoProxy, Proxy};
use std::time::Duration;

/// Apply proxy settings to a reqwest `ClientBuilder`.
///
/// Semantics:
/// - `NoProxy`: disable all proxy usage (including env/system discovery).
/// - `System`: use reqwest defaults (env/system proxy discovery).
/// - `Manual`: disable env/system proxies, then apply the configured proxy URL
///   for all schemes, with optional auth and optional no-proxy list.
pub fn apply_proxy_settings(
    mut builder: ClientBuilder,
    proxy: &ProxySettings,
) -> Result<ClientBuilder, String> {
    match proxy.mode {
        ProxyMode::NoProxy => Ok(builder.no_proxy()),
        ProxyMode::System => Ok(builder),
        ProxyMode::Manual => {
            let url = proxy.manual.proxy_url.trim();
            if url.is_empty() {
                // Treat this as a configuration error (invalid URL).
                // We return an error rather than silently falling back.
                return Err("Manual proxy mode enabled but proxy_url is empty".to_string());
            }

            // Ensure we don't accidentally combine env/system proxies with the
            // user-selected manual proxy.
            builder = builder.no_proxy();

            let mut p = Proxy::all(url).map_err(|e| e.to_string())?;

            let user = proxy.manual.username.trim();
            if !user.is_empty() {
                // Password may be empty; some proxies accept username-only.
                p = p.basic_auth(user, proxy.manual.password.as_str());
            }

            let no_proxy_raw = proxy.manual.no_proxy.trim();
            if !no_proxy_raw.is_empty() {
                // Note: `NoProxy::from_string` returns `Option<NoProxy>`.
                p = p.no_proxy(NoProxy::from_string(no_proxy_raw));
            }

            Ok(builder.proxy(p))
        }
    }
}

/// Build a reqwest `Client` configured with proxy settings.
///
/// Note: This does not set a default request timeout; call sites may still set
/// per-request timeouts (preferred for LLM providers).
pub fn build_http_client(proxy: &ProxySettings) -> Result<Client, String> {
    let builder = Client::builder();
    let builder = apply_proxy_settings(builder, proxy).map_err(|e| e.to_string())?;
    builder.build().map_err(|e| e.to_string())
}

/// Build a reqwest `Client` configured with proxy settings and a default timeout.
///
/// This is primarily used by STT providers that configure a client-wide timeout.
pub fn build_http_client_with_timeout(
    proxy: &ProxySettings,
    timeout: Duration,
) -> Result<Client, String> {
    let builder = Client::builder().timeout(timeout);
    let builder = apply_proxy_settings(builder, proxy).map_err(|e| e.to_string())?;
    builder.build().map_err(|e| e.to_string())
}
