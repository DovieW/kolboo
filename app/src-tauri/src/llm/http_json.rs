use std::time::Duration;

use reqwest::RequestBuilder;
use serde::Deserialize;

use super::LlmError;

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
        |e| LlmError::Network(e),
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

    let status = response.status();
    let body = response.text().await.unwrap_or_default();

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
