# Supported Providers & Models

This doc lists the **provider IDs** and **model IDs** currently supported by the app’s settings pickers.

Source of truth:

- Provider list: `app/src-tauri/src/commands/config.rs` (`STT_PROVIDERS`, `LLM_PROVIDERS`)
- Model lists (shown in the UI): `app/src/lib/modelOptions.ts` (`STT_MODELS`, `LLM_MODELS`)

Notes:

- Ollama discovers installed models. BYOK language-model discovery merges its
  results with the bundled catalog; an unavailable discovery service does not
  remove bundled choices. Local Whisper remains paused in normal builds.
- “Supported” here means implemented and selectable, not that every model has
  been tested against a paid vendor account. Account access and preview availability
  still depend on the vendor. Catalog review: **2026-10-08**.
- Saved model IDs remain visible as saved selections when a catalog changes.
  This does not guarantee a retired upstream model still works, and never switches
  the request to another provider. An unset model displays **Provider default**,
  not the first catalog entry. Explicitly selecting a new provider/model saves it.
- New models without a verified cost estimate show unknown pricing, not free
  usage. Live discovery can supply pricing; it is an estimate, not your invoice.
- Model pickers have two searchable tabs: **Kolboo Managed** and **Your models**.
  Both stay visible. Sign-in or a failed Managed catalog does not block BYOK/local
  choices. Browsing tabs, search and model details do not change routing.
  Selecting a result saves its source, provider and exact model ID together.
  Unconfigured providers offer secure setup; unknown prices never appear as free.

---

## Custom OpenAI-compatible providers

In **Settings → Providers → Custom providers**, add a name, API base URL
(including `/v1` where your server requires it), and model IDs. Then enter the
provider's API key in its key field. Add transcription models, language models,
or both; model discovery is manual.

Each model row has a required **Model ID** and an optional **Display name**. You
can still bulk-add IDs, then name individual rows. Display names are plain text
and affect picker labels only: requests, saved selections, and history keep the
original model ID. Leave the display name blank to show the ID.

Transcription, rewriting, Quick Ask, Quick Replace, intent routing, History
analysis, Prompt Lab, files and meetings use the same picker. Choices are filtered
for the action; realtime-only transcription models are not offered for files or
meetings. Missing saved choices stay visible rather than selecting another model.
Profile and linked-preset overrides retain inheritance; choosing **Use inherited
selection** clears only that override. Linked presets share their model choices.
Unpinned older selections are labeled **legacy routing** and retain their previous
preferences until you select an explicit source/model. Provider defaults are not
replaced by the first catalog result.

New or missing dictation language settings default to **Auto-detect**. Existing
explicit hints (including English) are preserved; profiles and presets can still
inherit a hint or explicitly select Auto-detect. Files and meeting setup also
offer a language hint, initially Auto-detect. That choice is saved with the audio
so retries do not pick up later settings changes. Hints guide supported models;
they do not add languages that a model does not support.
Details distinguish unknown pricing from a free model.

Custom providers always use your key directly, never Kolboo Managed. They appear
in normal/profile model choices and the Meeting model picker after a key is saved.
They remain available under **Your models** for Managed accounts too.

- Language models use `POST {base}/chat/completions` and JSON chat responses.
- Transcription models use `POST {base}/audio/transcriptions` with WAV multipart
  uploads and a JSON `text` response. Realtime, custom diarization formats, and
  vendor-specific options are not negotiated.
- Remote endpoints require HTTPS. HTTP is allowed for localhost and private or
  link-local IP addresses. Redirects are rejected; enter the final API base URL.
- Keys remain in OS secure storage. On Linux this uses Secret Service (for
  example KWallet or GNOME Keyring) so keys survive restarts. Provider metadata
  contains no keys.
- Transcription, rewriting and ad-hoc LLM tools honor configured proxy/TLS settings,
  including manual proxy bypass rules and explicit No proxy mode.
- Removing a provider removes its saved key but does not replace saved model
  selections. Select another model before using a recording/profile that referred
  to the removed provider; it will fail safely instead of contacting another vendor.

---

## Speech-to-Text (STT)

### `openai` (OpenAI)

- `gpt-transcribe` — GPT Transcribe _(file transcription)_
- `gpt-live-transcribe` — GPT Live Transcribe _(realtime only)_
- `gpt-4o-realtime-transcribe` — GPT-4o Realtime Transcribe _(realtime streaming)_
- `gpt-4o-mini-realtime-transcribe` — GPT-4o Mini Realtime Transcribe _(realtime streaming)_
- `gpt-4o-transcribe` — GPT-4o Transcribe
- `gpt-4o-mini-transcribe` — GPT-4o Mini Transcribe
- `gpt-4o-transcribe-diarize` — GPT-4o Transcribe with speaker labels
- `whisper-1` — Whisper-1 _(file transcription)_

### `aquavoice` (Aquavoice)

- `avalon-v1.5` — Avalon v1.5 (new default; API base `https://api.aquavoice.com/v1`)
- `avalon-v1-en` — Avalon v1 (legacy saved selections remain supported)

The Avalon v1.5 estimate uses $0.39/audio hour with the documented ten-second
minimum for non-empty audio. It does not invent an unpublished rounding increment.

### `groq` (Groq)

- `whisper-large-v3-turbo` — Whisper Large V3 Turbo _(Managed or BYOK)_
- `whisper-large-v3` — Whisper Large V3 _(Managed or BYOK)_

### `assemblyai` (AssemblyAI)

- `universal-3-6-pro` — Universal 3.6 Pro _(realtime; 32 languages)_
- `universal-3-5-pro` — Universal 3.5 Pro _(file transcription; 18 languages)_
- `universal-2` — Universal 2 _(file transcription; 99 languages)_
- `universal-streaming-english` — Universal Streaming English _(realtime)_
- `universal-streaming-multilingual` — Universal Streaming Multilingual _(realtime)_
- `universal` — legacy server-default alias (unchanged for existing defaults)

`slam-1` and `best` are no longer offered to new selections; saved IDs are not
rewritten. Sync, Dictation and Universal 3.5 realtime are separate integration work,
not enabled by adding a file-transcription model ID. Existing language hints are
preserved. Select a suitable model for the languages you need; a hint cannot
extend its language support.

### `speechmatics` (Speechmatics)

- `enhanced` — Enhanced
- `standard` — Standard

### `yiddishlabs` (YiddishLabs)

- `yiddishlabs-3-turbo` — Flash Turbo, uploaded after recording; not realtime.
- `yiddishlabs-standard` — Standard, asynchronous jobs for imported files and
  meetings only; not offered for dictation.

In **Settings → API keys**, Flash and Standard have separate secure key fields.
Saving one key does not make the other model ready. Managed models appear only
when Kolboo has configured and published their server-side routes; a BYOK key
does not grant Managed access.

Both APIs receive an explicit `auto` language hint unless you select an override.
Flash accepts Auto, Yiddish, English and Hebrew; Standard additionally accepts
Lashon Kodesh (`lk`). Incompatible saved hints stay visible rather than being
changed to Auto. These are the provider's documented hints, not an accuracy
guarantee for Yiddish, Hebrew or mixed-language speech.

YiddishLabs says Flash does not store submitted audio or transcripts. Standard
stores submitted jobs/content. Standard's local upload receipt contains no audio,
transcript or plaintext key. Retrying a known job resumes polling rather than
uploading again. An uncertain submission stays pending; do not create another
upload to work around it. Cancelling Kolboo's local work does not cancel vendor
processing or charges. Failed recordings remain available for recovery.

Long uploads use the file/meeting chunking and completed-chunk checkpoints.
Each chunk's receipt includes its recording position, so identical audio in two
different chunks cannot accidentally share a paid job or allowance reservation.
On Unix, receipt bytes and their directory entries are synced before submission.
Windows filesystem recovery and native recording/paste behavior still require a
Windows smoke test; Linux test fixtures do not establish that platform evidence.
Unknown vendor pricing is not displayed as free, and Managed allowance is
measured in audio seconds rather than estimated vendor credits.

Protocol and retention descriptions: [YiddishLabs API documentation](https://app.yiddishlabs.com/app/developer/api-docs).

### `deepgram` (Deepgram)

- `nova-3` — Nova 3
- `nova-2` — Nova 2
- `nova` — Nova
- `enhanced` — Enhanced
- `base` — Base

### `whisper` (Local Whisper)

- Paused/absent in normal builds; not reintroduced by this catalog refresh.

### `whisper-server` (Whisper-compatible server)

- `whisper-1` — conservative default; configured server models may differ.

### `elevenlabs` (ElevenLabs)

- `scribe_v2` — Scribe v2 _(realtime streaming)_
- `scribe_v1` — Scribe v1

### Fireworks audio (retired upstream)

Fireworks deprecated audio inference on June 10, 2026. It is no longer offered
as a new STT provider. The legacy adapter remains for saved configurations/history;
retaining it does not promise the upstream endpoint still operates. Fireworks LLM
support remains available.

---

## Language Models (LLM)

### `cerebras` (Cerebras)

- `qwen-3.8-27b` — Qwen 3.8 27B
- `gpt-oss-120b` — GPT-OSS 120B

### `openai` (OpenAI, LLM)

- `gpt-6.1-sol` — GPT-6.1 Sol
- `gpt-6-astra` — GPT-6 Astra
- `gpt-6-luna` — GPT-6 Luna
- `gpt-5.2` — GPT-5.2
- `gpt-5.1` — GPT-5.1
- `gpt-5` — GPT-5
- `gpt-5-mini` — GPT-5 Mini
- `gpt-5-nano` — GPT-5 Nano
- `gpt-4.1` — GPT-4.1
- `gpt-4.1-mini` — GPT-4.1 Mini
- `gpt-4.1-nano` — GPT-4.1 Nano
- `gpt-4o-mini` — GPT-4o Mini (unchanged backend default)

Reasoning options are model-specific. GPT-6 Astra does not offer `none`;
temperature is sent to GPT-6 only with supported, explicitly selected `none`.

### `gemini` (Google AI Studio)

- `gemini-3.8-flash` — Gemini 3.8 Flash
- `gemini-3.7-flash` — Gemini 3.7 Flash
- `gemini-3.6-flash` — Gemini 3.6 Flash
- `gemini-3.5-flash-lite` — Gemini 3.5 Flash-Lite
- `gemini-3.1-pro-preview` — Gemini 3.1 Pro (Preview)
- `models/gemini-3-flash-preview` — Gemini 3 Flash (Preview)
- `gemini-2.5-pro` — Gemini 2.5 Pro
- `gemini-2.5-flash` — Gemini 2.5 Flash
- `gemini-2.5-flash-lite` — Gemini 2.5 Flash-Lite

### `anthropic` (Anthropic)

- `claude-fable-5-1` — Claude Fable 5.1
- `claude-opus-5-5` — Claude Opus 5.5
- `claude-sonnet-5-5` — Claude Sonnet 5.5
- `claude-haiku-5-5` — Claude Haiku 5.5
- `claude-sonnet-4-5` — Claude Sonnet 4.5
- `claude-haiku-4-5` — Claude Haiku 4.5
- `claude-opus-4-5` — Claude Opus 4.5

The obsolete manual thinking-budget control is not offered for Claude 5.x;
saved older budget values are not sent to those models.

### `groq` (Groq, LLM)

- `openai/gpt-oss-120b` — GPT-OSS 120B
- `openai/gpt-oss-20b` — GPT-OSS 20B
- `qwen/qwen3.8-27b` — Qwen 3.8 27B (Preview)
- `llama-3.3-70b-versatile` — Llama 3.3 70B (Enterprise access)
- `llama-3.1-8b-instant` — Llama 3.1 8B (Enterprise access)

### `fireworks` (Fireworks, LLM)

- `accounts/fireworks/models/deepseek-v4p1-flash` — DeepSeek V4.1 Flash
- `accounts/fireworks/models/glm-5p3` — GLM 5.3
- `accounts/fireworks/models/kimi-k3` — Kimi K3
- `accounts/fireworks/models/llama-v3p1-8b-instruct` — Llama 3.1 8B
- `accounts/fireworks/models/llama-v3p1-70b-instruct` — Llama 3.1 70B

### `cohere` (Cohere)

- `command-a-plus-05-2026` — Command A+
- `command-a-03-2025` — Command A
- `command-r-plus-08-2024` — Command R+
- `command-r-08-2024` — Command R

### `ollama` (Ollama)

- No fixed list here (models are discovered from what you have installed in Ollama).

## Defaults and catalog scope

An explicit saved ID is never migrated just because a newer model exists.
For an unset LLM model, backend defaults are: Cerebras `gpt-oss-120b`, OpenAI
`gpt-4o-mini`, Gemini `gemini-3.5-flash-lite`, Anthropic `claude-haiku-5-5`,
Groq `openai/gpt-oss-20b`, Fireworks `accounts/fireworks/models/llama-v3p1-8b-instruct`,
and Cohere `command-r-08-2024`. Managed availability remains server-authoritative;
this BYOK catalog does not grant service access or transmit managed credentials.

## Upstream references

- [AssemblyAI streaming models](https://www.assemblyai.com/docs/streaming/select-the-speech-model)
  and [file models](https://www.assemblyai.com/docs/pre-recorded-audio/select-the-speech-model).
- [OpenAI models](https://developers.openai.com/api/docs/models) and
  [Realtime transcription](https://developers.openai.com/api/docs/guides/realtime-transcription).
- [Gemini models](https://ai.google.dev/gemini-api/docs/models) and
  [thinking options](https://ai.google.dev/gemini-api/docs/thinking).
- [Claude models](https://platform.claude.com/docs/en/models/overview).
- [Cerebras models](https://inference-docs.cerebras.ai/models/overview),
  [Groq models](https://console.groq.com/docs/models), and
  [Cohere models](https://docs.cohere.com/docs/models).
- [Fireworks recommended models](https://docs.fireworks.ai/guides/recommended-models)
  and [audio deprecation](https://docs.fireworks.ai/updates/changelog).
- [Aquavoice API](https://aquavoice.com/docs/api).

## OCR context label

Rewrite, Quick Ask and Quick Replace label automatic active-window OCR as
**Always**. This is a wording correction: the stored value remains `auto`, existing
settings are unchanged, and no additional screen capture is enabled on upgrade.
