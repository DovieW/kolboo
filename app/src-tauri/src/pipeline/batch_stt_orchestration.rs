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

/// This comparison only prevents account/environment switches; it does not
/// authenticate JWTs. The gateway still verifies every token and allowance.
pub(super) fn same_managed_owner(previous: Option<&str>, current: Option<&str>) -> bool {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    fn identity(token: &str) -> Option<(String, String, Option<String>)> {
        if token.len() > 65536 {
            return None;
        }
        let payload = token.split('.').nth(1)?;
        let data = URL_SAFE_NO_PAD.decode(payload).ok()?;
        let claims: serde_json::Value = serde_json::from_slice(&data).ok()?;
        let subject = claims.get("sub")?.as_str()?.trim();
        let issuer = claims.get("iss")?.as_str()?.trim();
        if subject.is_empty() || issuer.is_empty() {
            return None;
        }
        let org = |value: &serde_json::Value| {
            ["org_id", "organization_id"].iter().find_map(|name| {
                value
                    .get(name)?
                    .as_str()
                    .map(str::trim)
                    .filter(|org| !org.is_empty())
                    .map(str::to_owned)
            })
        };
        let organization = org(&claims).or_else(|| claims.get("app_metadata").and_then(org));
        Some((subject.into(), issuer.into(), organization))
    }
    let (Some(previous), Some(current)) = (previous, current) else {
        return false;
    };
    if previous.is_empty() || current.is_empty() {
        return false;
    }
    previous == current
        || identity(previous).is_some_and(|old| identity(current).is_some_and(|new| old == new))
}

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
        managed_token: Option<String>,
        wav_bytes: &[u8],
        retry_config: &crate::stt::RetryConfig,
        timeout: Duration,
        cancel_token: &CancellationToken,
        log_prefix: &str,
        stt_complete_reason: &str,
        upload_context: Option<&crate::stt::SttUploadContext>,
    ) -> Result<stt_flow::SttResult, PipelineError> {
        let stt_result = stt_flow::run_stt_transcription(
            stt_provider.clone(),
            wav_bytes,
            retry_config,
            Some(timeout),
            cancel_token,
            log_prefix,
            upload_context,
        )
        .await;

        match stt_result {
            Ok(result) => {
                if stt_complete_reason != "meeting_upload" {
                    self.mark_stt_complete(cancel_token, stt_complete_reason)?;
                }
                Ok(result)
            }
            Err(error) => {
                if cancel_token.is_cancelled() {
                    return Err(PipelineError::Cancelled);
                }
                let managed_auth_recovered =
                    { managed_token.is_some() && is_managed_auth_token_error(&error) };

                if managed_auth_recovered {
                    match self
                        .try_refresh_managed_auth_and_retry_stt(
                            stt_provider,
                            stt_provider_id,
                            stt_model,
                            stt_language,
                            managed_token.as_deref(),
                            wav_bytes,
                            retry_config,
                            timeout,
                            cancel_token,
                            log_prefix,
                            upload_context,
                        )
                        .await
                    {
                        Ok(recovered) => {
                            if stt_complete_reason != "meeting_upload" {
                                self.mark_stt_complete(cancel_token, stt_complete_reason)?;
                            }
                            Ok(recovered)
                        }
                        Err(retry_error) => {
                            self.finish_failed_stt_attempt(cancel_token, &retry_error)?;
                            Err(retry_error)
                        }
                    }
                } else {
                    self.finish_failed_stt_attempt(cancel_token, &error)?;
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
        original_provider: Arc<dyn SttProvider>,
        stt_provider_id: &str,
        stt_model: Option<String>,
        stt_language: Option<String>,
        original_token: Option<&str>,
        wav_bytes: &[u8],
        retry_config: &crate::stt::RetryConfig,
        timeout: Duration,
        cancel_token: &CancellationToken,
        log_prefix: &str,
        upload_context: Option<&crate::stt::SttUploadContext>,
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
            let inner = self
                .inner
                .lock()
                .map_err(|e| PipelineError::Lock(e.to_string()))?;
            if cancel_token.is_cancelled() {
                return Err(PipelineError::Cancelled);
            }
            if !same_managed_owner(
                original_token,
                inner.config.managed_inference_access_token.as_deref(),
            ) {
                return Err(PipelineError::Config("The managed account or organization changed; the recording is retained for an explicit retry".into()));
            }
            super::stt_provider_resolver::managed_transport_active(
                &inner,
                stt_provider_id,
                stt_model.as_deref(),
                Some(crate::model_selection::ModelSource::Managed),
            )?;
            let _ = stt_language; // The original adapter owns the frozen hint.
            original_provider.with_renewed_managed_token(
                inner.config.managed_inference_access_token.as_deref().unwrap_or_default()
            ).ok_or_else(|| PipelineError::Config("The original Managed adapter could not renew its credentials; the recording is retained".into()))?
        };

        stt_flow::run_stt_transcription(
            refreshed_provider,
            wav_bytes,
            retry_config,
            Some(timeout),
            cancel_token,
            &format!("{} (post-refresh)", log_prefix),
            upload_context,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_renewal_cannot_change_account_issuer_or_organization() {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let token = |sub, issuer, org, iat| {
            format!("header.{}.signature", URL_SAFE_NO_PAD.encode(serde_json::to_vec(&serde_json::json!({"sub":sub,"iss":issuer,"app_metadata":{"org_id":org},"iat":iat})).unwrap()))
        };
        let old = token("u", "issuer", "org", 1);
        assert!(same_managed_owner(Some(&old), Some(&old)));
        assert!(same_managed_owner(
            Some(&old),
            Some(&token("u", "issuer", "org", 2))
        ));
        for other in [
            token("other", "issuer", "org", 2),
            token("u", "other", "org", 2),
            token("u", "issuer", "other", 2),
            token("", "issuer", "org", 2),
            token("u", "", "org", 2),
            "invalid".into(),
            "header.%%%%.sig".into(),
            format!("h.{}.s", URL_SAFE_NO_PAD.encode("not JSON")),
            "x".repeat(65537),
        ] {
            assert!(!same_managed_owner(Some(&old), Some(&other)));
        }
        for (old, new) in [
            (None, None),
            (Some(""), Some("")),
            (Some("x"), None),
            (None, Some("x")),
            (Some("x"), Some("")),
        ] {
            assert!(!same_managed_owner(old, new));
        }
        let old = token("u", "issuer", "", 1);
        assert!(same_managed_owner(
            Some(&old),
            Some(&token("u", "issuer", "", 2))
        ));
    }

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
