# Kolboo – App setup “foot-gun” review

This is a quick, practical review of setup/defaults that commonly lead to misconfiguration, security issues, or reliability pain.

## Quick summary

### Top 5 risks

1. **Secrets/docs mismatch + legacy plaintext fallback**: API keys are *intended* to live in OS secure storage, but there’s a legacy `settings.json` fallback during migration, and user docs still say keys “may include API keys depending on current implementation”. This is confusing and can lead to bad operational choices (like copying `settings.json` around).
2. **Request logging can capture sensitive user content**: The app has a request logging system that can include transcripts, prompts, clipboard context, and provider request/response JSON. Secrets are redacted, but user text is still sensitive.
3. **One STT provider uses a no-timeout HTTP client**: The Fireworks STT provider constructs `reqwest::Client::new()` (no default timeout), which can hang a transcription call indefinitely if the network stalls.
4. **Contributor onboarding is easy to trip on**: Requires Node **24+** and a pinned pnpm version; if your Node is older, everything breaks in a confusing way.
5. **Supply-chain hardening in CI is “standard, not strict”**: GitHub Actions are referenced by version tags (e.g. `actions/checkout@v6`) rather than pinned to commit SHAs.

### Top 5 quick wins

1. **Update user-facing docs to match reality**: explicitly state “API keys are stored in OS keychain/credential manager” (and what happens if keychain is unavailable).
2. **Add a timeout to Fireworks STT HTTP client** (match other STT providers’ pattern).
3. **Make “safe log export” the default path** (strip user text + payloads by default when exporting/sharing logs).
4. **Add a one-screen “Dev Quickstart” to README** (and optionally add `.node-version` / `.nvmrc`).
5. **Optionally pin GitHub Actions to SHAs** (or add a policy note / Dependabot rule).

## First-pass inventory

### Tech stack

- **Desktop app**: Tauri v2
  - Config: `app/src-tauri/tauri.conf.json`
  - Backend: Rust 2021 (`app/src-tauri/Cargo.toml`)
- **UI**: React + Vite + TypeScript
  - Package manager: pnpm (pinned via `packageManager` field)
  - Tooling: Biome, Vitest, Knip (`app/package.json`)
- **Storage**:
  - App settings: `tauri-plugin-store` using `settings.json` (runtime file, not committed)
  - Secrets (API keys): OS credential store via the `keyring` crate (`app/src-tauri/src/secrets.rs`)

### How it’s started (dev/prod)

- Dev entrypoint (from `app/`):
  - `pnpm dev` → `tauri dev` (see `app/package.json`)
  - Tauri runs Vite via `beforeDevCommand: "pnpm dev:vite"` and loads `devUrl: http://localhost:5173` (`app/src-tauri/tauri.conf.json`)
- Build (from `app/`):
  - `pnpm build` → `tauri build`

There are **no Docker/compose/Helm manifests** in this repo (expected for a desktop app).

## “Run it mentally” (entrypoint → readiness)

- In dev:
  - `pnpm dev` starts Vite at `http://localhost:5173`, then starts the Tauri app pointing at that URL (`app/src-tauri/tauri.conf.json`).
  - The app “becomes ready” once the main window and pipeline init completes (desktop-style readiness; no HTTP health endpoint).
- Configuration:
  - There is **no `.env` usage** in-repo for normal running. (`.env*` files are ignored and none are committed.)
  - User configuration is primarily through the UI and persisted to `settings.json` (Tauri store).
  - API keys are provided through the UI and stored in OS secure storage.

## Foot-gun checklist findings

> Each finding includes: severity, evidence, why it’s a foot-gun, and a minimal fix.

| Finding | Severity | Evidence | Why it’s a foot-gun (plain English) | Fix recommendation (minimal) |
|---|---:|---|---|---|
| **Secrets: API keys are intended to be in OS keychain, but there’s a legacy plaintext fallback + docs still say “may include API keys”** | **High** | - `app/src-tauri/src/secrets.rs`: “keep API keys out of `settings.json` (plaintext at rest)” + `get_api_key()` falls back to `settings.json` via `get_legacy_api_key_from_store(...)`.<br>- Migration is “best-effort”: `migrate_api_keys_from_store(...)` keeps store value if secure storage write fails.<br>- `docs/User Docs/PRIVACY_AND_DATA.md`: “Settings … may include API keys depending on current implementation”. | Users will reasonably assume API keys are always safe, or always unsafe, depending on what they read. If secure storage fails (permissions, headless env, OS keychain issue), keys may remain in plaintext `settings.json`, and users might back it up / share it. | Update `docs/User Docs/PRIVACY_AND_DATA.md` to state current behavior clearly: “API keys are stored in OS keychain; legacy installs may temporarily store them in settings until migrated”. Consider surfacing a UI warning when secure storage fails (so users know keys are not protected). |
| **Request logging can capture sensitive user content (transcripts, prompts, clipboard context), even if secrets are redacted** | **High** | - `app/src-tauri/src/request_log.rs`: header comment “captures… API request/response details” and fields like `raw_transcript`, `formatted_transcript`, `rewrite_clipboard_context`, `*_request_json`, `*_response_json`.<br>- Redaction targets keys like `authorization`, `x-api-key`, `*_api_key` (`should_redact_key`).<br>- There is a helper to remove user text/payloads: `strip_request_log_text_and_payloads(...)`. | Logs are the #1 thing people share when debugging. Even if API keys are redacted, the logs can still include private text (dictation content, clipboard context, prompts). That’s a real “oops, I pasted my secrets/PII into an issue” risk. | Make the *default* “export logs” action use `strip_request_log_text_and_payloads` (or heavily nudge users to it). Add a one-line warning in the UI near log export: “Logs may contain transcript/clipboard text”. |
| **Network reliability: Fireworks STT provider uses a `reqwest::Client::new()` (no default timeout)** | **High** | - `app/src-tauri/src/stt/fireworks.rs`: `FireworksSttProvider::new(...)` calls `Self::with_client(reqwest::Client::new(), ...)`. | If a request stalls (Wi-Fi hiccup, provider hangs), the app can appear frozen or “stuck transcribing” indefinitely. Timeouts are the basic safety net. | Use the existing timeout builders (like other STT providers): either pass a timeout-configured client into `FireworksSttProvider::new`, or change `new(...)` to use `crate::network::build_plain_http_client_with_timeout(...)` with an appropriate duration. |
| **Onboarding: Node 24+ requirement is easy to miss** | **Medium** | - `app/package.json`: `"engines": { "node": ">=24" }` and `"packageManager": "pnpm@10.26.2"`.<br>- `CONTRIBUTING.md` lists Node 24+ as a prerequisite. | If a contributor has Node 20/22 installed (very common), installs/scripts fail in confusing ways. People waste time debugging “pnpm/lockfile/ESM” issues that are really “wrong Node version”. | Add a short “Dev Quickstart” in `README.md` that calls out Node 24+. Optional: add `.node-version` (asdf/Volta) and/or `.nvmrc` so tooling auto-detects the version. |
| **CI supply chain: actions are not pinned to SHAs** | **Low** | - Workflows use tags like `actions/checkout@v6`, `actions/setup-node@v6`, `actions/cache@v5` (e.g. `.github/workflows/check.yml`, `.github/workflows/windows-build.yml`). | Using tags is normal, but less strict than pinning commit SHAs. A compromised action release could affect builds. | If you want to harden: pin actions to commit SHAs (or use a policy like “pin for release workflows”). This is a hygiene improvement, not an urgent fix. |
| **Dev CSP is permissive (as expected), but ensure it never leaks into prod** | **Low** | - `app/src-tauri/tauri.conf.json` has `devCsp` including `'unsafe-eval'` and broad `connect-src` for Vite dev server. | This is fine for dev, but if it accidentally ships, it weakens protections. | Keep `csp` and `devCsp` separate (already done). Consider adding a small CI check that `csp` (prod) does not include `unsafe-eval`. |

## “How to run” (simplest dev bootstrap)

From `CONTRIBUTING.md` and `app/package.json`.

1) Install prerequisites:
- Node.js **24+**
- Rust toolchain (stable)
- Tauri prerequisites for your OS

2) Install deps:
- `cd app`
- `pnpm install --frozen-lockfile`

3) Run dev:
- `pnpm dev`

Optional build variants (from `README.md`):
- `pnpm dev:local-whisper`
- `pnpm dev:local-whisper:cuda`

## Quick-win patch list (small changes that reduce risk fast)

These are intentionally “small, not a refactor”.

1. **Add Fireworks STT timeout**
   - Change `FireworksSttProvider::new(...)` to use a timeout-configured client (or thread a timeout from config).
2. **Update privacy docs on secret storage**
   - Update `docs/User Docs/PRIVACY_AND_DATA.md` to reflect current behavior (OS keyring + legacy migration).
3. **Safer log sharing defaults**
   - Make log export default strip user text/payloads (use `strip_request_log_text_and_payloads`).
4. **One-page README “Dev Quickstart”**
   - Mention Node 24+, pinned pnpm, and the one command: `pnpm -C app dev`.
5. **Optional: CI action pinning**
   - Pin Actions to SHAs (starting with release workflows).

## Nice to have later (non-blocking)

- Add a “Where is my data stored?” section with OS-specific paths (settings/history/logs/recordings), so users don’t accidentally sync/share sensitive files.
- Add a small diagnostic page in-app that clearly says:
  - whether secure storage is available
  - whether any legacy plaintext keys remain in `settings.json`
- Consider a standard retry/backoff policy for transient network errors (careful to avoid retrying non-idempotent operations).
