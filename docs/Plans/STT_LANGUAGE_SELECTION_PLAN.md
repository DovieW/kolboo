# STT Language Selection Plan

## Why this doc exists
We want a single place that captures the language-selection work needed for **Settings → AI → Transcribe** so we don’t lose the provider-specific details. This document is **planning only** (no code changes yet).

## Product goals
- Add a **Language** dropdown in **Settings → AI → Transcribe**.
- The first option is **Auto-detect**.
- **English** is the default selection for new users.
- Internally, we pass provider-specific language parameters so the UI can stay provider-agnostic.
- Auto-detect should be supported per provider if possible; otherwise, fall back to “omit language” or provider defaults.

## UI/Settings expectations
- UI dropdown includes **Auto-detect** (first), **English** (default), plus other common languages.
- Persist the choice in settings (new field, e.g. `stt_language`).
- Settings normalization should map missing/invalid values to default **English**.
- A migration may be needed if any existing settings shape conflicts.

## Backend expectations
- `PipelineConfig` should receive the selected language.
- Each STT provider should apply the language in its own request format.
- When language is set to **Auto-detect**:
  - Use provider-specific detection flags where available.
  - If the provider supports auto-detect by omitting language, do that.
  - Otherwise, fall back to provider default (usually English).

## Auto-detect caveats
- Deepgram language detection is **not supported for streaming**; for streaming multilingual use cases, Deepgram recommends multilingual models with `language=multi`.
- Speechmatics auto-detect behavior is still unclear from accessible docs.

## Provider matrix (language key + format + auto-detect)
> Notes:
> - “Auto-detect” behavior varies by provider.
> - “Format” is the expected language code style.
> - Some providers are **OpenAI-compatible**; details are inferred from that compatibility when public docs are limited.

| Provider | Language key | Format | Auto-detect behavior | Notes | Evidence (doc/source) |
| --- | --- | --- | --- | --- | --- |
| OpenAI | `language` | ISO-639-1 (and some ISO-639-3) | Omit `language` | OpenAI Whisper transcription API supports `language` | OpenAI Audio Transcriptions API docs (previous research) |
| Groq (OpenAI-compatible) | `language` | ISO-639-1 | Omit `language` | OpenAI-compatible transcriptions endpoint | https://console.groq.com/docs/speech-to-text, https://console.groq.com/docs/api-reference |
| Fireworks (OpenAI-compatible) | `language` | ISO-639-1 | Omit `language` or send `null` | `language` is optional (`string | null`) | https://docs.fireworks.ai/api-reference/audio-transcriptions |
| Aquavoice (OpenAI-compatible) | `language` | ISO-639-1 | Omit `language` | Public docs only state OpenAI-compat; no explicit params | https://aquavoice.com/avalon-api |
| Whisper Server (OpenAI-compatible) | `language` | ISO-639-1 | Omit `language` | Uses OpenAI-compatible API | Internal repo usage + OpenAI-compat (inferred) |
| Local Whisper | Local config | Whisper language codes | Use `null`/omit to auto-detect | Language list comes from tokenizer map | https://github.com/openai/whisper/raw/refs/heads/main/whisper/tokenizer.py |
| AssemblyAI | `language_code` | `en_us`-style (snake) | `language_detection=true` | Default is `en_us` if omitted; detection options available | https://www.assemblyai.com/docs/api-reference/transcripts/submit, https://www.assemblyai.com/docs/speech-to-text/pre-recorded-audio/automatic-language-detection |
| Deepgram | `language` (query param) | BCP-47-ish (e.g., `en-US`) | `detect_language=true` | `detect_language` overrides `language`; default is `language=en` | https://developers.deepgram.com/docs/language, https://developers.deepgram.com/docs/language-detection |
| ElevenLabs | `language_code` | ISO-639-1/3 | Omit `language_code` | Defaults to auto-detect when `language_code` is `null` | https://elevenlabs.io/docs/api-reference/speech-to-text/convert |
| Speechmatics | `transcription_config.language` | `en` | Unknown | Realtime: language in StartRecognition; cannot change later | https://docs.speechmatics.com/api-ref/realtime-transcription-websocket, https://docs.speechmatics.com/api-ref/batch/create-a-new-job |

## Known gaps / TODOs
- Speechmatics: confirm auto-detect support and full language list.
- Aquavoice: confirm language param behavior beyond “OpenAI-compatible”.
- Deepgram: decide whether to expose a `language=multi` option for code-switching vs. keeping the UI list simple.
- Decide the **language list** shown in UI (full list vs. curated).
- Decide how to map UI labels to provider codes (e.g., English → `en`, `en-US`, `en_us`).

## Files likely to change (when implementation starts)
- UI:
  - `app/src/components/settings/prompt/TranscribeSettingsSection.tsx`
- Settings types + normalization:
  - `app/src/lib/tauri/types.ts`
  - `app/src/lib/tauri/settings.ts`
- Backend config:
  - `app/src-tauri/src/pipeline/config.rs`
- Provider implementations:
  - `app/src-tauri/src/stt/*.rs`
  - `app/src-tauri/src/stt/openai_compat.rs`

## Implementation outline (high level)
1. Add `stt_language` to settings types and defaults (English).
2. Add UI dropdown with Auto-detect + language list.
3. Normalize/migrate `stt_language` values in settings layer.
4. Extend `PipelineConfig` to include language.
5. Update each provider to pass language or auto-detect flags.
6. Add tests for settings normalization + provider request building.

## Acceptance checks
- Default is English for new users.
- Auto-detect option exists and is first in the dropdown.
- Language is passed correctly per provider.
- Auto-detect works per provider or falls back safely.
