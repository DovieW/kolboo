//! Retry utilities for STT providers with exponential backoff.

use crate::stt::SttError;
use serde::Serialize;
use std::time::Duration;

/// Structured telemetry about retries/backoff.
///
/// This is intended for diagnostics/benchmarking so we can tell whether a slow STT
/// request was genuinely slow, or slow because we retried/backed off.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RetryTelemetry {
    /// Total attempts including the first attempt. `attempts = retries + 1` on success.
    pub attempts: u32,
    /// Number of retries performed (does not include the initial attempt).
    pub retries: u32,
    /// Total time spent sleeping between retry attempts.
    pub total_delay_ms: u64,
    /// The last error observed before a success (if any), or the final error on failure.
    pub last_error: Option<String>,
}

#[derive(Debug)]
pub struct RetryOutcome<T> {
    pub result: Result<T, SttError>,
    pub telemetry: RetryTelemetry,
}

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay before first retry (doubles with each attempt)
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Whether to retry on rate limit errors
    pub retry_on_rate_limit: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            retry_on_rate_limit: true,
        }
    }
}

impl RetryConfig {
    /// Create a new retry config with custom max retries
    #[allow(dead_code)]
    pub fn with_max_retries(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    /// Calculate the delay for a given attempt number (0-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay = self
            .initial_delay
            .saturating_mul(2u32.saturating_pow(attempt));
        std::cmp::min(delay, self.max_delay)
    }
}

fn is_retryable_error_with_config(error: &SttError, config: &RetryConfig) -> bool {
    match error {
        SttError::NetworkMessage(_) => true,
        SttError::Network(_) => true,
        SttError::Timeout => true,
        SttError::Api(msg) => {
            // Retry on server errors (5xx) or rate limits (429)
            msg.contains("500")
                || msg.contains("502")
                || msg.contains("503")
                || msg.contains("504")
                || (config.retry_on_rate_limit
                    && (msg.contains("429")
                        || msg.to_lowercase().contains("rate limit")
                        || msg.to_lowercase().contains("too many requests")))
        }
        SttError::Audio(_) => false,  // Don't retry audio errors
        SttError::Config(_) => false, // Don't retry config errors
    }
}

/// Determines if an error is retryable.
///
/// Note: this uses a default policy (including retrying rate-limit errors).
/// If you need to respect a specific `RetryConfig`, use `with_retry`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn is_retryable_error(error: &SttError) -> bool {
    is_retryable_error_with_config(error, &RetryConfig::default())
}

/// Execute an async function with retry logic
pub async fn with_retry<F, Fut, T>(config: &RetryConfig, operation: F) -> Result<T, SttError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, SttError>>,
{
    let outcome = with_retry_report(config, operation).await;
    outcome.result
}

/// Execute an async function with retry logic and return structured telemetry.
///
/// Note: on timeout/cancellation, callers typically won't get telemetry because the
/// operation future is cancelled; this telemetry focuses on retry/backoff behavior.
pub async fn with_retry_report<F, Fut, T>(config: &RetryConfig, operation: F) -> RetryOutcome<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, SttError>>,
{
    let mut telemetry = RetryTelemetry::default();

    for attempt in 0..=config.max_retries {
        telemetry.attempts += 1;

        match operation().await {
            Ok(result) => {
                return RetryOutcome {
                    result: Ok(result),
                    telemetry,
                };
            }
            Err(e) => {
                telemetry.last_error = Some(e.to_string());

                if !is_retryable_error_with_config(&e, config) || attempt == config.max_retries {
                    return RetryOutcome {
                        result: Err(e),
                        telemetry,
                    };
                }

                telemetry.retries += 1;

                let delay = config.delay_for_attempt(attempt);
                telemetry.total_delay_ms = telemetry
                    .total_delay_ms
                    .saturating_add(delay.as_millis() as u64);

                log::warn!(
                    "STT request failed (attempt {}/{}), retrying in {:?}: {}",
                    attempt + 1,
                    config.max_retries + 1,
                    delay,
                    e
                );

                tokio::time::sleep(delay).await;
            }
        }
    }

    RetryOutcome {
        result: Err(SttError::Api("All retry attempts exhausted".to_string())),
        telemetry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_calculation() {
        let config = RetryConfig::default();

        // Initial delay: 500ms
        assert_eq!(config.delay_for_attempt(0), Duration::from_millis(500));
        // Second attempt: 1000ms
        assert_eq!(config.delay_for_attempt(1), Duration::from_millis(1000));
        // Third attempt: 2000ms
        assert_eq!(config.delay_for_attempt(2), Duration::from_millis(2000));
        // Fourth attempt: 4000ms
        assert_eq!(config.delay_for_attempt(3), Duration::from_millis(4000));
    }

    #[test]
    fn test_max_delay_capping() {
        let config = RetryConfig {
            max_delay: Duration::from_secs(2),
            ..Default::default()
        };

        // Should cap at max_delay
        assert_eq!(config.delay_for_attempt(10), Duration::from_secs(2));
    }

    #[test]
    fn test_is_retryable_error() {
        assert!(is_retryable_error(&SttError::Timeout));
        assert!(is_retryable_error(&SttError::Api(
            "500 Internal Server Error".to_string()
        )));
        assert!(is_retryable_error(&SttError::Api(
            "429 Rate limit exceeded".to_string()
        )));
        assert!(!is_retryable_error(&SttError::Config(
            "Invalid API key".to_string()
        )));
        assert!(!is_retryable_error(&SttError::Audio(
            "Invalid audio format".to_string()
        )));
    }

    #[tokio::test]
    async fn retry_report_counts_attempts_without_sleeping() {
        let config = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(0),
            max_delay: Duration::from_millis(0),
            retry_on_rate_limit: true,
        };

        let calls = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let out = with_retry_report(&config, || {
            let calls = calls.clone();
            let call_num = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            async move {
                if call_num < 3 {
                    Err(SttError::Timeout)
                } else {
                    Ok("ok".to_string())
                }
            }
        })
        .await;

        assert_eq!(out.result.unwrap(), "ok");
        assert_eq!(out.telemetry.attempts, 3);
        assert_eq!(out.telemetry.retries, 2);
        assert_eq!(out.telemetry.total_delay_ms, 0);
        assert!(out.telemetry.last_error.is_some());
    }
}
