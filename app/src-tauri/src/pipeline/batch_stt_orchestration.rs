//! Batch STT request orchestration.
//!
//! Provider selection stays in `stt_provider_resolver.rs`, and the actual STT
//! transport/retry execution stays in `stt_flow.rs`. This module owns the
//! cross-flow wrapper around batch STT attempts: managed-auth refresh retry,
//! `stt_complete` bookkeeping, and shared failure-state handling for normal
//! batch, streaming fallback, retry, and CLI transcription flows.

use crate::stt::{SttError, SttProvider};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use super::{stt_flow, PipelineError, SharedPipeline};

pub(super) fn is_managed_auth_token_error(err: &PipelineError) -> bool {
    let message = match err {
        PipelineError::Stt(SttError::Api(msg)) => msg,
        PipelineError::Stt(SttError::NetworkMessage(msg)) => msg,
        _ => return false,
    };

    let lower = message.to_lowercase();
    lower.contains("auth_invalid_token")
        || lower.contains("supabase auth user lookup rejected token")
        || lower.contains("unauthorized")
        || lower.contains("401")
}

impl SharedPipeline {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn run_batch_stt_request(
        &self,
        stt_provider: Arc<dyn SttProvider>,
        stt_provider_id: &str,
        stt_model: Option<String>,
        stt_language: Option<String>,
        wav_bytes: &[u8],
        retry_config: &crate::stt::RetryConfig,
        timeout: Duration,
        cancel_token: &CancellationToken,
        log_prefix: &str,
        stt_complete_reason: &str,
    ) -> Result<stt_flow::SttResult, PipelineError> {
        let stt_result = stt_flow::run_stt_transcription(
            stt_provider,
            wav_bytes,
            retry_config,
            Some(timeout),
            cancel_token,
            log_prefix,
        )
        .await;

        match stt_result {
            Ok(result) => {
                self.mark_stt_complete(stt_complete_reason);
                Ok(result)
            }
            Err(error) => {
                let managed_auth_recovered = {
                    let managed_enabled = self
                        .inner
                        .lock()
                        .map(|inner| inner.config.managed_inference_enabled)
                        .unwrap_or(false);
                    managed_enabled && is_managed_auth_token_error(&error)
                };

                if managed_auth_recovered {
                    match self
                        .try_refresh_managed_auth_and_retry_stt(
                            stt_provider_id,
                            stt_model,
                            stt_language,
                            wav_bytes,
                            retry_config,
                            timeout,
                            cancel_token,
                            log_prefix,
                        )
                        .await
                    {
                        Ok(recovered) => {
                            self.mark_stt_complete(stt_complete_reason);
                            Ok(recovered)
                        }
                        Err(retry_error) => {
                            self.finish_failed_stt_attempt(&retry_error)?;
                            Err(retry_error)
                        }
                    }
                } else {
                    self.finish_failed_stt_attempt(&error)?;
                    Err(error)
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    // This helper keeps retry orchestration readable at the call site: the
    // arguments are the STT attempt context that should remain visible together
    // rather than hidden behind another broad state bag.
    async fn try_refresh_managed_auth_and_retry_stt(
        &self,
        stt_provider_id: &str,
        stt_model: Option<String>,
        stt_language: Option<String>,
        wav_bytes: &[u8],
        retry_config: &crate::stt::RetryConfig,
        timeout: Duration,
        cancel_token: &CancellationToken,
        log_prefix: &str,
    ) -> Result<stt_flow::SttResult, PipelineError> {
        let app_handle = self
            .app_handle
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .ok_or_else(|| {
                PipelineError::Config(
                    "Managed auth refresh unavailable: app handle missing".to_string(),
                )
            })?;

        log::warn!(
            "{}: Managed auth token appears expired; attempting one-shot session refresh",
            log_prefix
        );

        crate::commands::licensing::license_refresh_entitlement(app_handle, Some(false))
            .await
            .map_err(|e| {
                PipelineError::Stt(SttError::Api(format!(
                    "Managed auth refresh failed: {}",
                    e.message
                )))
            })?;

        let refreshed_provider = {
            let mut inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;
            inner.get_or_create_stt_provider(stt_provider_id, stt_model, stt_language)?
        };

        stt_flow::run_stt_transcription(
            refreshed_provider,
            wav_bytes,
            retry_config,
            Some(timeout),
            cancel_token,
            &format!("{} (post-refresh)", log_prefix),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_auth_error_detection_matches_gateway_token_failures() {
        for message in [
            "auth_invalid_token",
            "Supabase auth user lookup rejected token",
            "unauthorized",
            "401 Unauthorized",
        ] {
            assert!(is_managed_auth_token_error(&PipelineError::Stt(
                SttError::Api(message.to_string())
            )));
        }
    }

    #[test]
    fn managed_auth_error_detection_rejects_non_auth_errors() {
        assert!(!is_managed_auth_token_error(&PipelineError::Stt(
            SttError::Api("429 rate limit".to_string())
        )));
        assert!(!is_managed_auth_token_error(&PipelineError::Config(
            "unauthorized setting name but not an STT transport error".to_string()
        )));
    }
}
