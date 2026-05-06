use std::time::Duration;

use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};

use super::LlmError;
use crate::request_log::RequestLogStore;

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ErrorDetail {
    message: String,
}

fn parse_error_message(body: &str) -> Option<String> {
    serde_json::from_str::<ErrorResponse>(body)
        .ok()
        .map(|p| p.error.message)
}

pub(super) async fn send_json_request_with_error_parser(
    provider_label: &str,
    req: RequestBuilder,
    timeout: Option<Duration>,
    parse_error: fn(&str) -> Option<String>,
) -> Result<serde_json::Value, LlmError> {
    send_json_request_with_error_parser_and_network_mapper(
        provider_label,
        req,
        timeout,
        parse_error,
        LlmError::Network,
    )
    .await
}

pub(super) async fn send_json_request_with_error_parser_and_network_mapper<MapNetworkError>(
    provider_label: &str,
    req: RequestBuilder,
    timeout: Option<Duration>,
    parse_error: fn(&str) -> Option<String>,
    map_network_error: MapNetworkError,
) -> Result<serde_json::Value, LlmError>
where
    MapNetworkError: FnOnce(reqwest::Error) -> LlmError,
{
    let req = if let Some(timeout) = timeout {
        req.timeout(timeout)
    } else {
        req
    };
    let req = crate::http::with_cloudflare_access_headers_from_request_url(req);

    let response = req.send().await.map_err(|e| {
        if e.is_timeout() {
            if let Some(timeout) = timeout {
                LlmError::Timeout(timeout)
            } else {
                // If we didn't configure a timeout, treat this as a generic network error.
                LlmError::Network(e)
            }
        } else {
            map_network_error(e)
        }
    })?;

    let (status, body) = crate::http::status_and_text(response).await;

    if !status.is_success() {
        let message = parse_error(&body).unwrap_or(body);
        return Err(LlmError::Api(format!(
            "{} API error ({}): {}",
            provider_label, status, message
        )));
    }

    serde_json::from_str::<serde_json::Value>(&body)
        .map_err(|e| LlmError::InvalidResponse(format!("Failed to parse response: {}", e)))
}

pub(super) async fn send_json_request(
    provider_label: &str,
    req: RequestBuilder,
    timeout: Option<Duration>,
) -> Result<serde_json::Value, LlmError> {
    send_json_request_with_error_parser(provider_label, req, timeout, parse_error_message).await
}

pub(super) fn record_llm_request<T: Serialize>(
    request_log_store: Option<&RequestLogStore>,
    provider_id: &str,
    request: &T,
) {
    let Some(store) = request_log_store else {
        return;
    };

    let request_json = serde_json::to_value(request).unwrap_or_else(|_| {
        serde_json::json!({
            "provider": provider_id,
            "error": "failed to serialize request",
        })
    });
    store.with_current(|log| {
        log.llm_request_json = Some(request_json);
    });
}

pub(super) fn record_llm_response(
    request_log_store: Option<&RequestLogStore>,
    response_json: &serde_json::Value,
) {
    let Some(store) = request_log_store else {
        return;
    };

    let response_for_log = response_json.clone();
    store.with_current(|log| {
        log.llm_response_json = Some(response_for_log);
    });
}

pub(super) async fn send_json_request_logged<T: Serialize>(
    provider_label: &str,
    provider_id: &str,
    req: RequestBuilder,
    timeout: Option<Duration>,
    request_log_store: Option<&RequestLogStore>,
    request: &T,
) -> Result<serde_json::Value, LlmError> {
    // The HTTP layer owns the request/response logging shape so providers only
    // describe their payload once. This keeps logging behavior consistent while
    // preserving provider-specific payload construction in each adapter.
    record_llm_request(request_log_store, provider_id, request);
    let response_json = send_json_request(provider_label, req, timeout).await?;
    record_llm_response(request_log_store, &response_json);
    Ok(response_json)
}
