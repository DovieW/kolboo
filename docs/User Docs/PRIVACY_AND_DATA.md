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
  - API keys and session secrets are intended to live in your OS secure
    storage / credential manager; legacy installs may still have old fallback
    values until they are migrated forward
- **History** (your transcription history)
- **Recordings** (if you enable saving audio recordings)
- **Home recorder recovery audio**: Home recordings save audio locally during
  capture, independently of the ordinary saved-recording setting. On Linux,
  enabling Computer audio also includes system output. Recovery audio is not
  encrypted by Kolboo and stays until transcription succeeds or you discard it.
  Stop/Recover sends audio to your selected provider; local providers stay local.
  Completed transcription sections also have saved audio for History playback.
  Cancel during capture discards that capture; cancel during transcription keeps
  remaining audio for recovery. Delete all recordings includes recovery files.
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
  - event-only and intentionally small in scope
  - no transcripts, prompts, completions, audio, OCR payloads, or clipboard
    contents are sent
  - desktop session replay and desktop autocapture stay off
  - nothing is sent until the first-run disclosure is reviewed
  - you can disable analytics later in **Settings → Data**
  - if your organization manages the app with policy, that policy can keep
    product analytics disabled and lock the toggle off
- **Crash/error monitoring (Sentry)**
  - used for reliability failures rather than product-behavior analytics
  - should not include raw user content or secrets
  - is DSN-gated, so environments without Sentry configuration do not send it

Current desktop product analytics scope is limited to a small set of settings /
cloud-sync events. Kolboo uses a locally generated random distinct ID for those
events; it is not your transcript text or provider credentials.

## Controlling your data

Kolboo includes controls for deleting locally stored data
(history/recordings/stats/logs), adjusting retention, and reviewing the current
analytics toggle in **Settings → Data**.

If your organization manages Kolboo, **Settings → Policy** can explain why a
particular setting is locked.

If you are a maintainer, see `docs/Dev Docs/TELEMETRY_GOVERNANCE.md` and
`docs/Dev Docs/SENTRY_INTEGRATION.md` for the current telemetry posture.

## Reporting concerns

If you believe there’s a privacy or security bug:

- For security issues: follow `SECURITY.md`.
- For non-security privacy concerns: open a GitHub issue with a minimal repro (avoid sensitive content).
