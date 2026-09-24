# Privacy & Data

Kolboo is a voice dictation app. Depending on your settings, it can:

- record microphone audio
- transcribe locally (Local Whisper) **or** send audio to a third-party speech-to-text (STT) provider
- optionally send text to a third-party language model (LLM) provider for rewriting/cleanup

This document explains, at a high level, what data may be stored or transmitted.

In addition to microphone audio, Kolboo can interact with **on-screen text context** to provide a smoother dictation workflow (for example, rewriting highlighted text, or using clipboard-based paste modes).

## What Kolboo may store locally

Exact file names/locations can vary by OS, but generally live under your OS “app data” directory.

Common categories of local data include:

- **Settings** (via the Tauri store)
  - includes your chosen providers/models and feature toggles
  - API keys and session secrets live in your OS secure storage / credential
    manager (Windows Credential Manager, macOS Keychain, or Linux Secret
    Service such as KWallet/GNOME Keyring). Legacy installs may still have old
    fallback values until they are migrated forward. On Linux, a working Secret
    Service is required so credentials persist across desktop sessions and
    reboots.
- **History** (your transcription history)
- **Recordings** (if you enable saving audio recordings)
- **Home recorder recovery audio**: Home recordings save audio locally during
  capture, independently of the ordinary saved-recording setting. On Linux,
  enabling Computer audio also includes system output. Recovery audio is not
  encrypted by Kolboo and stays until transcription succeeds or you discard it.
  Stop/Recover sends audio to your selected provider; local providers stay local.
  A final transcription also has saved audio for History playback; capture does
  not send periodic transcription requests. After Stop, long recordings are
  uploaded in smaller parts and combined into one transcript. Completed parts'
  transcript text is saved locally (not encrypted by Kolboo) so interrupted work
  can resume. Recovery audio and partial text are removed after the final result
  is saved, or when you discard the recovery recording. Provider failures keep them.
  Cancel during capture discards that capture; cancel during transcription keeps
  the full audio for recovery. Delete all recordings includes recovery files.
- **Usage/cost stats** (a local ledger of cost events)
- **Logs**
  - the app includes request/response logging features intended for troubleshooting
  - logs should redact secrets, but you should still treat logs as potentially sensitive

If you are sharing logs for debugging, please review and redact anything sensitive (API keys, tokens, transcripts, window titles, file paths, etc.).

## Clipboard and highlighted text context

Depending on the features you use (and sometimes the target app you are typing into), Kolboo may:

- **Read the clipboard** to support clipboard-based output modes and/or to restore your clipboard after temporarily writing a transcription for paste.
- **Write the clipboard** (for example, placing the transcription on the clipboard before simulating a paste).
- **Use highlighted/selected text** as context for rewrite flows (e.g., “rewrite the selection”) when you trigger actions that operate on existing text.

Because clipboard contents and selected text can be sensitive, please treat them as part of the app’s “potentially sensitive data surface,” similar to transcripts.

## What Kolboo may send over the network

Kolboo only makes network requests when features/providers you enable require it.

Depending on your configuration, Kolboo may send:

- **Audio** (to an STT provider)
- **Transcripts / text** (to an LLM provider)
- **Provider metadata** (model IDs, language, etc.)

Third-party providers have their own privacy policies and retention behavior.

## Product analytics and crash telemetry

Kolboo currently separates telemetry into two buckets:

- **Product analytics (PostHog)**
  - event-only, with fixed page names and bounded recording metadata
  - no transcripts, prompts, completions, audio, OCR payloads, or clipboard
    contents are sent
  - desktop session replay and desktop autocapture stay off
  - nothing is sent until the current first-run disclosure is reviewed; its
    collection and identifier details expand beneath the short choice
  - basic unlinked usage events then run when PostHog is configured; each event
    has a fresh ID and no persistent device or account ID, so these counts do
    not identify returning users or unique installations
  - basic events count app opens, page visits, recording starts and outcomes,
    transcription starts/results/errors; recording outcomes include rounded
    duration, mode, and recognized provider/model names (unknown/custom values
    are grouped as “other”)
  - **Settings → Data** controls optional linked analytics. When enabled, events
    use an opaque account ID while signed in, or a random local installation ID
    while signed out. No email address is sent; disabling it stops linked events
    immediately, without joining prior unlinked events to the account
  - organization policy can disable both streams
- **Crash/error monitoring (Sentry)**
  - used for reliability failures rather than product-behavior analytics
  - keeps code locations, release/platform details and limited error categories;
    raw error text, recordings, transcripts, provider responses, account identity,
    request details and automatic UI/console breadcrumbs are not included
  - is DSN-gated, so environments without Sentry configuration do not send it
  - does not enable session replay, profiling or continuous log uploads

The shared PostHog project marks events as development, beta, or production so
local testing can be excluded from release reports. Counts are best effort when
the app is offline or PostHog is unavailable. The PostHog server still sees the
network request, including its source IP.

## Controlling your data

Kolboo includes controls for deleting locally stored data
(history/recordings/stats/logs), adjusting retention, and reviewing the optional
linked analytics toggle in **Settings → Data**.

If your organization manages Kolboo, **Settings → Policy** can explain why a
particular setting is locked.

If you are a maintainer, see `docs/Dev Docs/TELEMETRY_GOVERNANCE.md` and
`docs/Dev Docs/SENTRY_INTEGRATION.md` for the current telemetry posture.

## Reporting concerns

If you believe there’s a privacy or security bug:

- For security issues: follow `SECURITY.md`.
- For non-security privacy concerns: open a GitHub issue with a minimal repro (avoid sensitive content).
