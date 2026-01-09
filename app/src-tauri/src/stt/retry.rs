//! Retry utilities for STT providers with exponential backoff.

use crate::stt::SttError;
use std::time::Duration;

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
        SttError::Audio(_) => false, // Don't retry audio errors
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
pub async fn with_retry<F, Fut, T>(
    config: &RetryConfig,
    operation: F,
) -> Result<T, SttError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, SttError>>,
{
    let mut last_error: Option<SttError> = None;

    for attempt in 0..=config.max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if !is_retryable_error_with_config(&e, config) || attempt == config.max_retries {
                    return Err(e);
                }

                let delay = config.delay_for_attempt(attempt);
                log::warn!(
                    "STT request failed (attempt {}/{}), retrying in {:?}: {}",
                    attempt + 1,
                    config.max_retries + 1,
                    delay,
                    e
                );

                tokio::time::sleep(delay).await;
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| SttError::Api("All retry attempts exhausted".to_string())))
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
        assert!(is_retryable_error(&SttError::Api("500 Internal Server Error".to_string())));
        assert!(is_retryable_error(&SttError::Api("429 Rate limit exceeded".to_string())));
        assert!(!is_retryable_error(&SttError::Config("Invalid API key".to_string())));
        assert!(!is_retryable_error(&SttError::Audio("Invalid audio format".to_string())));
    }
}
