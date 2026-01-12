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
  - may include **API keys** depending on current implementation
- **History** (your transcription history)
- **Recordings** (if you enable saving audio recordings)
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

## Controlling your data

Kolboo includes controls for deleting locally stored data (history/recordings/stats/logs) and for data retention.

If you are a maintainer, the code-grounded backlog includes several privacy-hardening items (for example: secure API key storage and stricter redaction). See `docs/TODO.md`.

## Reporting concerns

If you believe there’s a privacy or security bug:

- For security issues: follow `SECURITY.md`.
- For non-security privacy concerns: open a GitHub issue with a minimal repro (avoid sensitive content).
