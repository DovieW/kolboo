# Provider-Family Seam Decisions

Provider-family seams are only real when at least two concrete adapters use the seam and the deletion test shows caller complexity would otherwise reappear.

## Decision template

| Field                           | Value                               |
| ------------------------------- | ----------------------------------- |
| Concern                         |                                     |
| Candidate adapters              |                                     |
| Current duplication/caller pain |                                     |
| Deletion test result            |                                     |
| Decision                        | Implement / Defer / Reject / Reopen |
| Target files if implemented     |                                     |
| Characterization tests          |                                     |
| Redaction/privacy checks        |                                     |
| Notes                           |                                     |

## Managed-mode adaptation

| Field                           | Value                                                                                                                                                                                                      |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Concern                         | Managed-mode provider adaptation                                                                                                                                                                           |
| Candidate adapters              | Personal license managed routing and Enterprise policy managed routing                                                                                                                                     |
| Current duplication/caller pain | The shared behavior is already localized in `resolve_provider_mode(...)`; provider constructors consume the resolved runtime mode rather than duplicating tier/policy checks.                              |
| Deletion test result            | Additional managed-mode seam in `managed_inference/mod.rs` would be pass-through: deleting it would not reintroduce duplicated provider logic because the existing resolver remains the load-bearing seam. |
| Decision                        | Defer                                                                                                                                                                                                      |
| Target files if implemented     | None for this slice; keep `app/src-tauri/src/pipeline/config.rs` as the current seam.                                                                                                                      |
| Characterization tests          | `app/src-tauri/src/tests/managed_personal_tests.rs` (`managed_mode_characterization_covers_personal_and_enterprise_adapters`, `managed_mode_characterization_preserves_byok_fallbacks`)                    |
| Redaction/privacy checks        | No request payloads or secrets are handled by this resolver; managed tokens remain outside the tests.                                                                                                      |
| Notes                           | Reopen only if another caller starts duplicating tier/policy/eligibility routing outside `resolve_provider_mode(...)`.                                                                                     |

## Provider error classification

| Field                           | Value                                                                                                                                                                                                                                                    |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Concern                         | Retry/auth/quota/timeout/user-visible failure classification                                                                                                                                                                                             |
| Candidate adapters              | STT HTTP providers that surface status/body as `SttError::Api(...)` (e.g. Groq, Deepgram, OpenAI-compatible providers)                                                                                                                                   |
| Current duplication/caller pain | Deterministic retry classification is already centralized in `stt::retry::is_retryable_error(...)`; provider adapters still need provider-specific status/body extraction.                                                                               |
| Deletion test result            | A broader provider-family error seam would either wrap only one real shared policy (`is_retryable_error`) or erase provider-specific response parsing. Deleting such a new seam would not simplify current callers beyond the existing retry helper.     |
| Decision                        | Defer                                                                                                                                                                                                                                                    |
| Target files if implemented     | None for this slice; keep provider-specific extraction in adapters and shared retry classification in `app/src-tauri/src/stt/retry.rs`.                                                                                                                  |
| Characterization tests          | `app/src-tauri/src/tests/stt_integration_tests.rs` (`provider_error_classification_preserves_retryable_status_semantics`) plus existing provider non-success tests for Deepgram, AssemblyAI, Groq, Fireworks, Aquavoice, ElevenLabs, and Whisper Server. |
| Redaction/privacy checks        | Tests use synthetic status/body strings only; no API keys or user content.                                                                                                                                                                               |
| Notes                           | Reopen when at least two adapters can share response parsing without losing provider-specific error detail.                                                                                                                                              |

## Provider request metadata and redaction

| Field                           | Value                                                                                                                                                                                                                                                             |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Concern                         | Request metadata, diagnostics, and redaction                                                                                                                                                                                                                      |
| Candidate adapters              | STT, LLM, router, Quick Ask/Quick Replace, and OCR request-log payload fields                                                                                                                                                                                     |
| Current duplication/caller pain | Secret redaction and user-content stripping are already centralized in `request_log.rs`; adapters only attach bounded payload previews.                                                                                                                           |
| Deletion test result            | A new STT-provider metadata seam would be pass-through unless it replaced the existing request-log sanitizer. Deleting the existing sanitizer would reintroduce redaction at every payload field, so it remains the correct seam.                                 |
| Decision                        | Defer                                                                                                                                                                                                                                                             |
| Target files if implemented     | None for this slice; keep `app/src-tauri/src/request_log.rs` as the sanitizer/metadata seam.                                                                                                                                                                      |
| Characterization tests          | `app/src-tauri/src/tests/request_log_schema_tests.rs` (`request_log_redaction_preserves_provider_metadata_but_removes_secrets_and_payloads`, `request_log_preserves_strategy_independent_router_decision_diagnostics`) plus existing request-log redaction tests. |
| Redaction/privacy checks        | Authorization/token-like values are redacted, provider/model metadata remains, and user text/provider payloads are stripped by retention helpers.                                                                                                                 |
| Notes                           | Reopen if adapter-specific payload capture begins duplicating sanitizer logic outside request-log helpers.                                                                                                                                                        |

## Provider cost reporting

| Field                           | Value                                                                                                                                                                                                                         |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Concern                         | Cost reporting and provider-category summaries                                                                                                                                                                                |
| Candidate adapters              | OpenAI, Groq, Fireworks STT/LLM estimators; additional cost modules remain provider-specific                                                                                                                                  |
| Current duplication/caller pain | The aggregation path is shared in `stats.rs`/`commands/stats.rs`, while pricing tables and estimation formulas are intentionally provider-specific.                                                                           |
| Deletion test result            | A new generic cost-estimator seam would need provider-specific tables immediately and would not reduce caller complexity in the aggregation path. Deleting it would not reintroduce meaningful duplicated caller logic today. |
| Decision                        | Defer                                                                                                                                                                                                                         |
| Target files if implemented     | None for this slice; keep provider-specific estimators under `app/src-tauri/src/cost/**` and shared aggregation in stats commands.                                                                                            |
| Characterization tests          | `app/src-tauri/src/tests/pricing_llm_schema_tests.rs` (`provider_family_cost_estimators_preserve_provider_specific_rates`) plus existing cost module tests.                                                                   |
| Redaction/privacy checks        | Cost tests use synthetic token/audio counts and no user text, request payloads, or secrets.                                                                                                                                   |
| Notes                           | Reopen only if two or more providers share a real pricing formula/table shape that callers must invoke uniformly.                                                                                                             |
