# Seam Evidence

## Telemetry Mapping

- Interface: `telemetry::cost_inputs_from_request_log(...)`.
- Implementation: maps rich `RequestLog` variants into compact `CostTelemetryInputs`.
- Depth: Stats no longer knows Quick Ask / Quick Replace field priority rules.
- Deletion test: removing the Module pushes request-kind branching back into `stats.rs` and any future telemetry consumers.
- Characterization: unit tests in `telemetry.rs` cover normal transcription, Quick Ask precedence, and Quick Replace fallback.

## Cost Reporting

- Interface: `cost::reporting::{report_stt_cost, report_llm_cost}`.
- Implementations/Adapters: OpenAI and Groq are covered by shared report tests; additional provider branches remain provider-specific and table/formula-owned.
- Depth: parsing provider response telemetry and choosing event-level estimates moved out of `stats.rs`.
- Deletion test: removing the Module reintroduces a long provider-specific match in `stats.rs` for every stats event emission path.
- Characterization: unit tests in `cost/reporting.rs` cover OpenAI STT duration fallback, Groq STT duration pricing, OpenAI/Groq LLM usage mapping with distinct rates, and Anthropic cache token mapping.

## Embeddings Module

- Interface: `EmbeddingsProvider` plus `embeddings::build_provider(...)` and input-role helpers.
- Implementations/Adapters: OpenAI, Cohere, and Fireworks concrete providers.
- Depth: production routing now calls through the Interface rather than direct provider modules.
- Deletion test: removing the Module helpers pushes provider default/model/input-type logic and direct HTTP calls back into `pipeline/routing.rs`.
- Characterization: routing tests cover shared cache-key use and Cohere document input-role diagnostics.

## STT provider construction locality

- Interface: `stt_cloud_adapters::adapter_for(...).build(...)`.
- Implementations/Adapters: OpenAI and Groq construction are characterized without network calls; registry includes all cloud STT providers.
- Depth: `stt_provider.rs` validates shared preconditions and delegates constructor quirks.
- Deletion test: removing the adapter registry restores the large constructor match and provider-specific wiring in the factory.
- Characterization: adapter registry tests verify expected IDs and two concrete provider constructions.

## WebSocket transport policy

- Interface: `stt::websocket_transport::connect_ws_with_transport_policy(...)` plus the compatibility wrapper `stt::streaming::connect_ws_split_with_timeout(...)`.
- Implementations/Adapters: OpenAI Realtime, Deepgram, ElevenLabs, Speechmatics, AssemblyAI, and Fireworks now pass provider-owned `ProxySettings` into the shared transport path while keeping provider-specific headers, URLs, and protocol messages in their adapters.
- Depth: manual proxy CONNECT tunnelling, manual `no_proxy` bypass, and trusted-CA / invalid-cert TLS overrides moved out of provider adapters; `stt/streaming.rs` stays focused on provider-independent session lifecycle helpers.
- Deletion test: removing the Module pushes manual proxy/tunnel/TLS policy back into every realtime STT adapter or reintroduces the old silent gap where WS ignored configured transport settings.
- Characterization: unit tests in `stt/websocket_transport.rs` cover `no_proxy` host matching and the remaining unsupported HTTPS-proxy diagnostic.

## Recording Orchestration

- Interface: `recording_orchestration::{spawn_routing_started_watcher, spawn_rewriting_started_watcher}`.
- Implementation: command-facing phase watchers emit UI events when the pipeline reaches a target state.
- Depth: duplicate watcher loops moved out of `commands/recording.rs`; the state machine still owns transitions.
- Deletion test: removing the Module duplicates polling intervals, terminal-state handling, hard-stop timeout, and event emission in every recording command flow.
- Characterization: pure classification tests cover target, waiting, and terminal states.
