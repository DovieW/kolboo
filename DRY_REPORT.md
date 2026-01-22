# DRY_REPORT.md — DRY findings (Kolboo)

This is a practical “what should we extract?” report.

It was generated using a **simple duplicate scan + manual review** (no AST parsing, no embeddings). See `DRY_PLAN.md` for the exact command used.

## Summary (where duplication clusters)

The most meaningful duplication is in:

1. **Backend provider plumbing (Rust)** — repeated HTTP request setup, timeouts, error mapping, request/response logging, and response parsing across:
	 - `app/src-tauri/src/stt/**`
	 - `app/src-tauri/src/embeddings/**`

2. **Settings UI patterns (TS/TSX)** — repeated “select with Default + hint” rendering blocks across settings screens.

## Ranked duplicate groups (top 10)

Ranking criteria: (a) repeat count, then (b) size of block, then (c) importance/core runtime impact.

### 1) STT providers: multipart transcription request + consistent error/log handling

**What it does:** Build a multipart form (`wav_transcription_form`), send to a transcription endpoint, map timeout/network errors, parse non-2xx responses, log request/response JSON, extract `text` from JSON.

**Where (examples / evidence):**

- `app/src-tauri/src/stt/openai.rs` `[124:5 - 165:16]` and `[120..260]` shows request logging + send + status check + JSON parsing
- `app/src-tauri/src/stt/groq.rs` `[80..200]` shows nearly identical flow
- `app/src-tauri/src/stt/whisper_server.rs` `[177:36 - 191:2]` (very similar send/status/json/log)
- Additional similar blocks detected in `stt/fireworks.rs`, `stt/elevenlabs.rs`, `stt/deepgram.rs`, `stt/assemblyai.rs`, `stt/aquavoice.rs`.

**How many times it appears:** the same flow shows up across **5–8** STT provider implementations.

**Recommendation:** Extract a helper for the common “transcribe WAV via OpenAI-compatible multipart endpoint” path.

- Proposed helper: `openai_compat::transcribe_wav_multipart(...)` (or `stt/openai_style.rs`)
- Proposed signature (example):
	- `async fn transcribe_wav_multipart(client: &Client, provider: &'static str, endpoint: &str, api_key: Option<&str>, model: &str, audio: &[u8], prompt: Option<&str>, log: Option<&RequestLogStore>) -> Result<String, SttError>`
- Variable parameters:
	- provider name
	- endpoint URL
	- auth strategy
	- model/prompt rules

**Risks / gotchas:**

- Prompt clamping is model/provider-specific (keep that logic per provider; pass final prompt into helper).

---

### 2) Tests: repeated scaffolding (low priority)

**What it does:** repeated test setup/fixtures.

**Where (evidence):**

- `app/src-tauri/src/pipeline/tests.rs` has multiple repeated blocks (jscpd flagged many clones)
- Several `.test.ts` files under `app/src/lib/contracts/schemas/` were flagged

**How many times it appears:** many occurrences, but mostly in tests.

**Recommendation:** Optional. Only refactor test duplication if it improves readability without making the tests “too clever”.

## “Do not refactor” list (duplication that’s OK / risky)

- The long list of explicit Tauri `invoke(...)` wrappers in `app/src/lib/tauri/commands.ts` is repetitive, but it also serves as a clear UI↔backend contract. DRY-ing it too much can hide argument shapes and make refactors harder.
- Provider implementations often *look* similar but have important differences (timeouts, auth headers, endpoints, request schemas, error schemas). Prefer extracting only the truly shared lifecycle bits (request building/logging/timeout/error mapping) rather than forcing full inheritance.
- Key injection code is easy to break: extract helpers only if you can keep behavior identical and have good regression coverage.
- Test duplication is sometimes intentional and can keep scenarios readable.
