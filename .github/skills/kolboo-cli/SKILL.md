---
name: kolboo-cli
description: "Use when: testing, debugging, benchmarking, or documenting the Kolboo CLI; running pipeline status/transcribe/stream, config export, settings, profiles, or logs commands; validating CLI startup with app data from com.kolboo.app."
argument-hint: "What Kolboo CLI behavior should be tested or documented?"
---

# Kolboo CLI Skill

Use this skill when you need to run, test, debug, benchmark, or document the Kolboo command-line interface.

The CLI is implemented in the Tauri backend and shares the desktop app data directory (`com.kolboo.app`), so it can use existing settings, secure-storage API keys, logs, and recordings from a normal Kolboo install.

## What this skill produces

- A safe CLI validation plan.
- Commands that exercise the real Kolboo CLI with existing app data.
- Sanitized result summaries that do not dump transcripts, prompts, API keys, tokens, or secrets.
- Fixes for CLI startup/runtime issues when the CLI itself is broken.
- Documentation updates when CLI behavior, command names, or expected output changes.

## Key files and directories

- CLI config: `app/src-tauri/tauri.conf.json`
- CLI implementation: `app/src-tauri/src/cli/**`
- CLI bootstrap: `app/src-tauri/src/lib.rs` setup block that calls `tauri_plugin_cli::init()` and `cli::handle_cli(...)`
- Binary entrypoint: `app/src-tauri/src/main.rs`
- Package script: `app/package.json` script `cargo:cli`
- App data on Windows: `C:\Users\Dovie\AppData\Roaming\com.kolboo.app`
- Recordings: `C:\Users\Dovie\AppData\Roaming\com.kolboo.app\recordings\*.wav`
- App logs: `C:\Users\Dovie\AppData\Roaming\com.kolboo.app\logs\*.log`

## Command surface

Top-level CLI commands are configured in `app/src-tauri/tauri.conf.json`:

- `pipeline status`
- `pipeline transcribe -f <wav>`
- `pipeline stream -f <wav> --stt_provider <provider>`
- `pipeline run`
- `pipeline stop`
- `config export`
- `settings get -k <key>`
- `settings set -k <key> -v <value>`
- `profiles list`
- `profiles use -p <profile>`
- `diagnostics`
- `logs request list|clear|export`
- `logs app dir|list|show`

Most commands accept `-o json` or `-o human`; default behavior is JSON.

The JSON envelope is:

- `success: boolean`
- `code: number`
- `message: string | null`
- `warnings: string[]`
- `data: object | null`

Exit code conventions in `app/src-tauri/src/cli/errors.rs`:

- `0`: success
- `2`: validation/argument error
- `3`: runtime error

## Standard workflow

### 1. Locate a runnable CLI binary

Prefer an already-built executable first because it avoids unnecessary Cargo work:

- Windows debug binary: `app/src-tauri/target/debug/kolboo.exe`
- Isolated CLI build script: `pnpm -C app cargo:cli -- <args>`
- Direct Cargo fallback: `cargo run --manifest-path app/src-tauri/Cargo.toml --target-dir app/src-tauri/target-cli --bin kolboo -- <args>`

If another Cargo process is holding the default target lock, use an isolated target dir such as `app/src-tauri/target-cli` or `app/src-tauri/target-agent`.

Before Rust/Cargo commands, follow repo rules:

- Set `RUSTC_WRAPPER=sccache` when `sccache` is available; otherwise clear it.
- Set `CARGO_BUILD_JOBS` conservatively, about half logical cores capped at 8.

### 2. Smoke-test startup

Run a no-secret command first:

- `pipeline status -o json`
- `config export -o json`
- `logs app list -o json`

Expected result:

- Process exits `0`.
- Last CLI result JSON has `success: true` and `code: 0`.
- `pipeline status` returns `data.state`, usually `idle`.

Note: debug builds may emit tracing or WebView/Chromium messages around the CLI JSON. When scripting, capture output and parse the last line that starts with `{"success"`.

PowerShell pattern for robust JSON extraction:

```powershell
$exe = Resolve-Path .\app\src-tauri\target\debug\kolboo.exe
$raw = & $exe pipeline status -o json 2>&1
$jsonLine = $raw | Where-Object { $_ -like '{"success"*' } | Select-Object -Last 1
$result = $jsonLine | ConvertFrom-Json
```

### 3. Use existing app data

On Windows, check the workspace app-data folder first:

- Recordings: `C:\Users\Dovie\AppData\Roaming\com.kolboo.app\recordings`
- Logs: `C:\Users\Dovie\AppData\Roaming\com.kolboo.app\logs`
- Settings store: `C:\Users\Dovie\AppData\Roaming\com.kolboo.app\settings.json`

Do not print secrets. Prefer CLI commands that redact or omit sensitive values, such as `config export -o json`, which removes keys containing `api_key`, `token`, or `secret`.

### 4. Choose a WAV for real transcription

Use an existing recording if available. Prefer the smallest WAV for a fast validation run.

Validation command shape:

- `pipeline transcribe -f <wav> -o json`

For benchmark runs:

- Add `-r <repeat>` for measured repeats.
- Add `-w <warmup>` for warmup runs.
- Repeat/warmup values are clamped to `1..50` and `0..50` by the CLI.

### 5. Sanitize transcription output

Do not dump `stt_text`, `final_text`, prompt text, or raw request/response payloads into chat by default.

Report metadata instead:

- `success`
- `code`
- `stt_text_length`
- `final_text_length`
- `stt_duration_ms`
- `llm_duration_ms`
- `llm_outcome`
- `stt_retry.attempts`
- `stt_retry.retries`
- `diagnostics.wav.duration_secs_est`
- `diagnostics.file.size_bytes`

PowerShell pattern for a sanitized transcription summary:

```powershell
$exe = Resolve-Path .\app\src-tauri\target\debug\kolboo.exe
$file = "C:\Users\Dovie\AppData\Roaming\com.kolboo.app\recordings\<recording>.wav"
$raw = & $exe pipeline transcribe -f $file -o json 2>&1
$jsonLine = $raw | Where-Object { $_ -like '{"success"*' } | Select-Object -Last 1
$result = $jsonLine | ConvertFrom-Json
$data = $result.data
[pscustomobject]@{
   success = $result.success
   code = $result.code
   stt_text_length = if ($data.stt_text) { $data.stt_text.Length } else { $null }
   final_text_length = if ($data.final_text) { $data.final_text.Length } else { $null }
   stt_duration_ms = $data.stt_duration_ms
   llm_duration_ms = $data.llm_duration_ms
   llm_outcome = $data.llm_outcome
   stt_retry_attempts = $data.stt_retry.attempts
   stt_retry_retries = $data.stt_retry.retries
   wav_duration_secs_est = $data.diagnostics.wav.duration_secs_est
   file_size_bytes = $data.diagnostics.file.size_bytes
} | ConvertTo-Json -Depth 6
```

If the user explicitly asks to inspect transcript text, confirm that it is okay to show potentially personal audio content.

### 6. Branch on failures

#### CLI does not start or no subcommand is recognized

Inspect:

- `app/src-tauri/src/cli/mod.rs`
- `app/src-tauri/src/lib.rs` setup block
- `app/src-tauri/tauri.conf.json` command definitions

Check that:

- The first CLI argument is a known top-level command.
- `tauri_plugin_cli::init()` is registered before `app.cli().matches()`.
- `cli::is_cli_invocation()` includes the relevant top-level command if desktop-only behavior is being gated.
- The Tauri config command names and Rust `handle_cli` dispatch names match.

Add or update tests near `cli/mod.rs` for command detection when changing command names.

#### JSON is polluted by logs

Debug builds can emit tracing or Chromium messages. For tests/scripts, parse the last CLI envelope line beginning with `{"success"`.

If clean machine-readable output is required, inspect logging initialization and ensure CLI-mode logs go to stderr or are suppressed before writing JSON to stdout.

#### `pipeline transcribe` fails because the file is missing or invalid

Use a WAV from `com.kolboo.app\recordings` or create a small deterministic WAV for tests. The command rejects empty files and reports WAV diagnostics when successful.

#### `pipeline transcribe` fails because no API key is configured

Do not ask the user to paste secrets into chat.

Use existing app secure storage/settings if available. If a key genuinely is missing, report which provider/key name is expected, for example `<provider>_api_key`, and stop before making network calls.

#### Provider/model behavior is under test

Use per-run overrides instead of mutating settings when possible:

- `--stt_provider <provider>`
- `--stt_model <model>`
- `--llm_provider <provider>`
- `--llm_model <model>`
- `-p <profile>` for a profile override

After changing persisted settings with `settings set`, remember that the CLI calls `sync_pipeline_config` so runtime config updates immediately.

#### Streaming behavior is under test

Use:

- `pipeline stream -f <wav> --stt_provider <provider> -o json`

Notes:

- The stream command requires a provider that supports streaming.
- It requires a configured API key for the selected provider.
- It supports `--stt_model`, `--language`, and `--speed`; `--speed 0` means as-fast-as-possible.
- It prints partial/commit progress to stderr for realtime visibility and returns final JSON on success.

## Quality checks

For docs-only skill updates:

- No app test/check commands are required.
- Validate YAML frontmatter manually:
  - `name` matches the folder name.
  - `description` is quoted and keyword-rich.
  - No tabs in YAML.

For CLI code changes:

1. Run formatting first:
   - `pnpm -C app cargo:fmt`
2. Run focused Rust tests or checks:
   - targeted tests for touched CLI modules, or
   - `pnpm -C app cargo:test`
3. Run at least one real CLI smoke command:
   - `pipeline status -o json`
4. If the task touches transcription behavior, run one sanitized `pipeline transcribe -f <wav> -o json` against an app-data WAV when credentials/settings are available.

## Reporting format

When finished, summarize:

- The executable path or build path used.
- The app-data folder used.
- Commands run and exit codes.
- Sanitized transcription metadata, not transcript text.
- Any CLI startup issue fixed or any remaining blocker.
- Files changed and validation status.
