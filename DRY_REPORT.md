# DRY_REPORT.md — DRY findings (Kolboo)

This is a practical “what should we extract?” report.

Key note: **exact copy/paste in production code is fairly low** in this repo. The main DRY opportunities are “same structure, different provider/settings keys”.

## Summary (where duplication clusters)

Top duplication hotspots:

1. **Rust STT providers** (`app/src-tauri/src/stt/**`): repeated constructor + base URL + request-log plumbing.
2. **Rust settings parsing**: repeated “read JSON from store → coerce type → clamp → default” patterns.

## Ranked duplicate groups (top 4)

Ranking criteria: (a) repeats, then (b) block size, then (c) proximity to core logic.

### 1) Rust STT provider constructor + config boilerplate (highest impact)

**What it does:** each provider repeats the same scaffolding:

- store `reqwest::Client` + `api_key` + `model`
- `new(...)` builds a client with timeout
- `with_client(...)` for tests/overrides
- `with_api_base_url(...)` + `api_base_url_trimmed()`
- optional request logging store

**Evidence (examples):**

- `app/src-tauri/src/stt/openai.rs` [37:–80:]
- `app/src-tauri/src/stt/deepgram.rs` [47:–75:]
- `app/src-tauri/src/stt/assemblyai.rs` [67:–114:]
- `app/src-tauri/src/stt/groq.rs` [31:–79:]

Representative snippet (OpenAI provider):

```rust
pub fn new(api_key: String, model: Option<String>, default_prompt: Option<String>) -> Self {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .expect("Failed to create HTTP client");

    Self { /* client/api_key/model/base_url/request_log_store */ }
}
```

**Appears:** ~8–9 providers/files.

**Recommendation:**

- Extract a small helper in a shared module, e.g. `stt/http.rs`:
  - `fn default_http_client(timeout: Duration) -> reqwest::Client`
  - `fn trim_base_url(base: &str) -> &str`
- Optionally extract a reusable struct for common fields:
  - `struct ProviderHttpContext { client, api_base_url, request_log_store }`

**Variation points to parameterize:** default model, timeout, auth header format, default prompt semantics.

**Risks:** low-to-medium. Refactor touches provider initialization; safest to do in small steps and keep the public constructors.

---

### 2) Repeated `reqwest::Client::builder()` setup (cross-provider)

**What it does:** many modules build a client in the same way (timeout/build/expect or fallback).

**Evidence:**

- `app/src-tauri/src/stt/openai.rs` [37:–52:]
- `app/src-tauri/src/stt/deepgram.rs` [47:–60:]
- `app/src-tauri/src/stt/assemblyai.rs` [67:–82:]
- `app/src-tauri/src/commands/backup.rs` [223:–229:] (`github_client()`)

Snippet (backup GitHub client):

```rust
reqwest::Client::builder()
    .user_agent("kolboo")
    .build()
    .unwrap_or_else(|_| reqwest::Client::new())
```

**Recommendation:** extract a single helper that centralizes timeouts + user-agent + TLS tweaks (if any). This reduces “fix it in 7 places” when HTTP behavior needs to change.

**Risks:** low, as long as timeouts remain provider-specific.

---

### 3) Base URL trimming + endpoint URL builders repeated across providers

**What it does:** repeated `api_base_url_trimmed()` + `format!("{}/vX/...", ...)` helpers.

**Evidence (examples):**

- `app/src-tauri/src/stt/openai.rs` [74:–90:] (`api_base_url_trimmed`, `transcriptions_url`, `responses_url`)
- `app/src-tauri/src/stt/assemblyai.rs` [98:–125:] (`api_base_url_trimmed`, `upload_url`, `transcript_url`, ...)

**Recommendation:** a tiny helper type, e.g.:

- `struct ApiBaseUrl(String)`
- `impl ApiBaseUrl { fn trimmed(&self) -> &str; fn join(&self, path: &str) -> String; }`

**Risks:** low.

---

### 4) Rust settings parsing + defensive clamping patterns

**What it does:** repeated “read value → coerce number/string → default → clamp(1..100_000)” logic.

**Evidence:**

- `app/src-tauri/src/commands/history.rs` [17:–33:] (`max_saved_recordings`)
- `app/src-tauri/src/commands/history.rs` [52:–74:] (`transcription_retention_amount` parsing)
- `app/src-tauri/src/lib.rs` [753:–763:] (clamp max saved recordings via `get_setting_from_store`)

Representative snippet (history):

```rust
let raw = get_settings_store(app, SettingsReadMode::Fresh)
    .and_then(|store| store.get("max_saved_recordings"))
    .and_then(|v| v.as_u64())
    .unwrap_or(default);

(raw.clamp(1, 100_000)) as usize
```

**Recommendation:** create a small set of typed helpers in the settings store layer:

- `get_u64(key) -> Option<u64>`
- `get_u64_coerce(key) -> Option<u64>` (accept string/float)
- `get_u64_clamped(key, default, min, max) -> u64`

**Risks:** medium. Settings semantics are subtle (missing vs `null` vs invalid); helpers must preserve current behavior.
## “Do not refactor” list (duplication that’s OK)

- The long list of explicit Tauri `invoke(...)` wrappers in `app/src/lib/tauri/commands.ts` is repetitive, but it also serves as a clear UI↔backend contract. DRY-ing it too much can hide argument shapes and make refactors harder.
- Provider implementations often *look* similar but have important differences (timeouts, auth headers, endpoints, retries, error mapping). Prefer extracting only the truly shared pieces (client creation, base URL joining) rather than forcing full inheritance.
- Test duplication is sometimes intentional and can keep scenarios readable.
