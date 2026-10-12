# Streaming STT — Developer Guide

## Overview

Kolboo supports two transcription modes:

| Mode | How it works | When text appears |
|------|-------------|-------------------|
| **Batch** | Record → stop → upload all audio → get transcript | After recording stops (+ network round-trip) |
| **Streaming** | Open WebSocket at record-start → feed audio chunks during recording → get partials in real time | Words appear *while* you speak |

Streaming **input** and live **output** are distinct. The profile/preset-inheritable
`stt_input_mode` defaults to `auto`: concurrent input for a capable selected model, even when
**Live Output (experimental)** (`stt_live_output`) is off. Live Output controls
pasting committed segments while recording; it does not turn WebSocket audio
upload on/off. `after_recording` sends no audio until stop; realtime-only selections
reject that mode and retain the recording instead of substituting another model.
With Live Output off, provisional text appears in the overlay, then the final
result is optionally rewritten and pasted once. Preview text uses automatic text
direction; RTL contents are not reversed or rewritten by the UI.
Simulated streaming is a separate opt-in path for batch-only providers. Home
history-only recordings do not start this live dictation path.

Live insertion requires rewriting, routing and Quick Replace to be off, a
non-Quick-Ask session, and a verifiable focused target. Windows uses UIA runtime
identity, macOS uses the focused Accessibility element, and supported Linux
applications use AT-SPI focused/editable state plus active-window identity.
Unavailable accessibility support falls back to preview and final output. Losing
that target stops further insertion and fails finalization; already inserted text
is never automatically replaced. Platform-native Windows/macOS behavior still
requires platform smoke testing; Linux fixtures do not verify those APIs.

Each recording freezes its provider, model, source, language and pipeline options
before capture. Settings updates apply to subsequent recordings. Token renewal
may refresh credentials for the same account/issuer/organization, but cannot turn
a BYOK failure into a Managed retry or silently bill another account.

---

## Architecture

```
┌──────────────┐      f32 chunks       ┌──────────────────────┐
│ Audio Capture ├──────────────────────►│ StreamingSttSession  │
│ (CPAL/WASAPI) │ via live_audio_tx    │  ┌────────────────┐  │
└──────────────┘                        │  │  audio_tx ──►  │  │  WSS
                                        │  │  WebSocket  ◄──┼──┼──────► Provider
                                        │  │  partial_rx ◄─ │  │
                                        │  └────────────────┘  │
                                        └──────────┬───────────┘
                                                   │ partial transcripts
                                                   ▼
                                        ┌──────────────────────┐
                                        │    Pipeline          │
                                        │  preview by default  │
                                        │  optional committed  │
                                        │  live insertion      │
                                        └──────────────────────┘
```

### Key types

- **`SttProvider` trait** ([stt/mod.rs](../../app/src-tauri/src/stt/mod.rs)) — every provider implements `transcribe()` (batch). Providers that support streaming also override `supports_streaming()` → `true` and implement `start_streaming()`.
- **`StreamingSttSession`** ([stt/streaming.rs](../../app/src-tauri/src/stt/streaming.rs)) — holds `audio_tx` (send chunks) and `partial_rx` (receive transcripts). Call `finalize()` when recording stops to get the final text.
- **`Pipeline`** ([pipeline.rs](../../app/src-tauri/src/pipeline.rs)) — orchestrates everything. During recording, it routes live audio to the session's `audio_tx` and reads `partial_rx` to drive live output. On stop, it calls `finalize()`. A failed stream may fall back to batch only if the model supports batch and no live insertion has started. Otherwise it preserves the recording and reports a recoverable error; it never automatically replaces text already inserted.

Connection work, the provider task and the registered preview/live-insertion
consumer are abort-on-drop:
discarding a session or cancelling its finalization future cannot detach either.
The pipeline attaches a deferred consumer before publishing the session, and
activates it under its state lock only after accepting the connection. A connection
that finishes after stop may replay the complete retained audio and immediately
close input. Stop waits up to ten seconds for the owned connection, without
polling. Cancelled or superseded sessions cannot activate a late handoff. Preview
events carry a recording epoch, and the overlay ignores closed/older epochs.
Finalization
drains it before returning a successful transcript. A consumer panic fails the
stream with a content-free error, rather than accepting unfinished output.
`finalize()` (including
the pipeline and CLI) uses a 60-second tail limit; direct callers can set a
different deadline with `finalize_with_timeout()`. The provider and consumer share
one deadline. That tail starts after audio input closes, not at recording start.
A timeout is a failed stream, not a successful partial result.
This does not cancel provider-side processing or assert a vendor billing refund.

Capture uses a nonblocking `StreamingAudioSender`. A full or closed provider
queue marks the entire stream incomplete, aborts its provider task, and rejects
subsequent chunks. Finalization reports a recoverable failure instead of accepting
a partial transcript as success. The captured recording is independent of this
queue; safe batch fallback still requires that no live insertion has started.

The CPAL callback queue also fails closed on overflow/pool exhaustion, rather
than dropping the oldest recorded samples. Callback-time epochs and drain permits
retain the Hot Mic tail with its original recording. Incomplete capture is not
submitted, but its available WAV remains recoverable. Native test runs use an
isolated null PCM, not the operator's microphone.
Hot Mic VAD audio and events carry the same recording epoch. The processor resets
for each new recording, and queued speech-end events from stopped or older
recordings are discarded instead of stopping a later recording.
The capture worker guard joins audio/VAD workers on stream construction or play
failure as well as normal shutdown; a failed microphone start cannot leave
detached capture workers running.

ElevenLabs and Speechmatics buffered input share the session's PCM feeder. It
downmixes complete frames, bounds chunk sizes without splitting multichannel
frames, drains previews locally, and rejects a closed delivery queue before
finalization. Provider-specific WebSocket protocols remain in their adapters.

Provider-local transcript reconciliation preserves protocol semantics. AssemblyAI
commits each numbered turn once and orders queued final turns. OpenAI uses item
IDs and committed-item predecessor metadata, not completion arrival order; see
the [official realtime transcription guide](https://developers.openai.com/api/docs/guides/realtime-transcription).
Deepgram rejects overlapping/out-of-order final ranges and ignores duplicates.
Provisional tails are not promoted to successful final transcripts. ElevenLabs
timestamp supplements are not treated as second commits. Losing a committed
consumer-queue chunk fails the stream; a preview-only frame may be coalesced.

The recording buffer owns live delivery under the same mutex as sample appends.
Attaching after connection replays the saved prefix (including pre-roll) before
later chunks; there is no second forwarding path to duplicate samples. Prefixes
that have already wrapped or failed storage fail closed; capture format changes
invalidate an active input.
Clearing delivery after worker drain closes input without aborting normal
finalization; resetting a recording aborts any old input.

### Pipeline flow (streaming enabled)

1. User presses record hotkey
2. Pipeline calls `stt_provider.start_streaming(sample_rate)` → gets `StreamingSttSession`
3. Audio capture callback sends f32 samples via `live_audio_tx` → session's `audio_tx`
4. Provider's background task converts f32→PCM, sends over WebSocket, reads partials
5. Pipeline reads `partial_rx`, previews text and optionally inserts committed chunks
6. User releases hotkey → pipeline calls `session.finalize()` → gets final transcript
7. Final transcript goes through the rewrite/prompt pipeline as usual

---

## Provider Support Matrix

| Provider | Batch | Streaming | Batch Transport | Stream Transport | Partials? | Notes |
|----------|:-----:|:---------:|----------------|-----------------|:---------:|-------|
| **Speechmatics** | ✅ | ✅ | WSS (`eu.rt`) | WSS (`eu2.rt`) | ✅ `AddPartialTranscript` | Both modes require an explicit language hint |
| **Fireworks (legacy)** | legacy | legacy | HTTP POST | WSS | custom stability logic | Upstream audio inference deprecated June 10, 2026; no new STT selections |
| **ElevenLabs** | ✅ | ✅ | HTTP POST or buffered WSS | WSS | ✅ | Scribe v2 realtime uses the same owned engine in both input modes |
| **OpenAI** | ✅ | ✅ | HTTP POST | WSS (Realtime API) | ✅ | `gpt-live-transcribe` and legacy realtime IDs are stream-only |
| **AssemblyAI** | ✅ | ✅ | HTTP POST | WSS (v3) | ✅ turn-based | Final turns require both `end_of_turn` and `turn_is_formatted` |
| **Groq** | ✅ | ❌ | HTTP POST | — | — | |
| **Deepgram** | ✅ | model/hint dependent | HTTP POST | WSS (`/v1/listen`) | ✅ `is_final` flag | Auto uses documented Nova multilingual input; other models need an explicit hint for realtime |
| **YiddishLabs Flash Turbo** | ✅ | ❌ | HTTP POST | — | — | Separate Flash key; language defaults to explicit `auto` |
| **YiddishLabs Standard** | files/meetings | ❌ | HTTP submission and polling | — | — | Separate Standard key; protected upload receipts prevent automatic paid resubmission |
| **Aquavoice** | ✅ | ❌ | HTTP POST | — | — | |
| **Local Whisper (paused)** | optional build | ❌ | Local process | — | — | Not enabled in normal builds |

---

## How Each Streaming Provider Works

### Speechmatics

Two separate WebSocket endpoints:
- **After recording** (`transcribe_ws`): connects to `wss://eu.rt.speechmatics.com/v2/`, then feeds buffered mono audio through the same owned task used for live input. Sending a chunk cannot be cancelled by a competing receive branch.
- **Streaming** (`run_streaming_task`): connects to `wss://eu2.rt.speechmatics.com/v2/`, uses `tokio::select!` to concurrently send audio chunks from `audio_rx` and receive `AddPartialTranscript` / `AddTranscript` messages.

Final text concatenates the provider's formatted `metadata.transcript` without
adding whitespace or changing RTL contents. `EndOfStream.last_seq_no` counts
chunks sent, not acknowledgements received. Success requires `EndOfTranscript`
after audio delivery; an early close or unfinalized partial fails.
The existing Enhanced/Standard WebSocket adapter requires a language hint.
Speechmatics documents [Auto identification as batch-only](https://docs.speechmatics.com/speech-to-text/languages);
Kolboo does not silently replace Auto with English.
Unknown model IDs also fail before connecting instead of selecting Enhanced.
Startup heartbeats cannot extend the overall handshake deadline.

### Fireworks

This section describes the retained legacy adapter, not an available new audio
product. See the [upstream deprecation](https://docs.fireworks.ai/updates/changelog).

Single WSS endpoint for streaming: `wss://audio-streaming.api.fireworks.ai/v1/audio/transcriptions/streaming`.

Fireworks doesn't distinguish "partial" vs "final" — every response replaces the full transcript. Kolboo uses a custom **segment stability algorithm** to decide when to commit text for live output:

- `SEGMENT_STABILITY_THRESHOLD = 3` — commit a segment after 3 consecutive identical updates
- `SEGMENT_AGE_COMMIT_SECS = 1.5` — time-based fallback: commit after 1.5s if the segment has been stable for at least 1 update (prevents text from never appearing during short recordings)

### ElevenLabs

WSS at `wss://api.elevenlabs.io/v1/speech-to-text/realtime`. Both input modes use
base64 PCM chunks, manual commits and one outstanding commit at a time. Kolboo
commits at most twenty seconds of audio per window, then waits for its committed
transcript before sending the next window. Stopping commits the remaining audio;
an empty final chunk with `commit=true` flushes an already-sent tail.
Timestamp supplements do not create another commit. Missing or unrequested
commit responses fail instead of accepting a partial recording.
This follows the provider's [manual commit guidance](https://elevenlabs.io/docs/eleven-api/guides/how-to/speech-to-text/realtime/transcripts-and-commit-strategies).

### OpenAI

Uses the Realtime API at `wss://api.openai.com/v1/realtime?intent=transcription`.
Audio is resampled to 24 kHz mono PCM16, base64 encoded and sent in JSON
`input_audio_buffer.append` messages, followed by `input_audio_buffer.commit`.
It is not sent as binary WebSocket frames.

- `gpt-live-transcribe`: GA `session.update` with `type: transcription`, nested
  `audio.input`, `languages` array and disabled turn detection. No beta header or
  server VAD is sent to this model.
- Legacy realtime IDs: retain their `transcription_session.update` contract,
  beta header, singular language field and existing server VAD configuration.
- File `gpt-transcribe`: multipart `languages[]`; older Whisper-compatible
  model IDs retain singular `language`. Recorded requests reflect this distinction.

Realtime-only IDs set `requires_streaming()`: do not silently use a batch endpoint
after a streaming failure. Mock WebSocket tests cover both session contracts,
authentication headers, audio encoding, commit, partials and final text.

### Deepgram

Live input uses `wss://api.deepgram.com/v1/listen`; after-recording uploads use
the HTTP transcription endpoint. Realtime availability depends on the selected
model and language hint. WebSocket query parameters control live behavior:
- `interim_results=true` — receive partial transcripts as speech is processed
- `endpointing=300` — detect end of utterances after 300ms silence
- `utterance_end_ms=1500` — emit `UtteranceEnd` event after 1.5s silence
- `smart_format=true` + `punctuate=true` — clean formatted output
- Auto on `nova-2`, `nova-3` and `nova-3-medical` sends `language=multi`, not
  batch-only `detect_language=true`. Other selected models with Auto use
  after-recording input. Explicit hints stay unchanged. See
  [Deepgram multilingual input](https://developers.deepgram.com/docs/multilingual-code-switching).

Protocol:
- Send binary PCM s16le mono audio chunks
- Receive `{"type": "Results", "is_final": bool, "channel": {"alternatives": [{"transcript": "..."}]}}`
- `is_final: false` → interim partial (overlay update only)
- `is_final: true` → finalized segment (commit for live paste)
- On recording stop: send `{"type": "Finalize"}` to flush, then `{"type": "CloseStream"}` to close

### AssemblyAI

WSS at `wss://streaming.assemblyai.com/v3/ws`. Turn-based protocol — the server
groups speech into turns and sends partial/final updates per turn.

- Pin `speech_model` to the selected realtime model. Long-lived keys are sent
  only in `Authorization`, never as a URL `token` (that parameter is for temporary
  tokens). Request logs omit credentials and parse errors omit payload contents.
- Universal 3.6 Pro accepts the existing language hint as `language_code`, reduced
  to its base ISO code; no legacy `format_turns` parameter is sent.
- Legacy Universal Streaming uses `format_turns=true`; commit only when both
  `end_of_turn` and `turn_is_formatted` are true, avoiding duplicate commits of
  the unformatted predecessor. Intermediate text remains a partial.
- Universal 3.5 Pro and Universal 2 currently use file upload/polling. Sync,
  Dictation and Universal 3.5 realtime require separate protocol work.

Local mock-server tests exercise upload/model selection/polling and WebSocket
authentication, intermediate/final turns, termination and sanitized failures.

---

## CLI Testing

The CLI binary supports both batch and streaming transcription for quick testing without the full UI:

```powershell
# Batch transcribe a WAV file
kolboo.exe pipeline transcribe --provider speechmatics --model enhanced --wav "audio.wav"

# Stream a WAV file (simulates real-time playback)
kolboo.exe pipeline stream --provider speechmatics --model enhanced --wav "audio.wav" --speed 1

# Stream at max speed (dumps audio as fast as possible — useful for latency testing,
# but misleading for real-time providers since they still process at ~1x)
kolboo.exe pipeline stream --provider assemblyai --model universal-3-6-pro --wav "audio.wav" --speed 0

# With language
kolboo.exe pipeline transcribe --provider groq --model whisper-large-v3-turbo --wav "audio.wav" --language en
```

Output is JSON: `{"text": "...", "elapsed_ms": 1234}`.

There's also a benchmark script at `scripts/benchmark-streaming-vs-batch.ps1`:
```powershell
.\scripts\benchmark-streaming-vs-batch.ps1 -WavFile "C:\path\to\audio.wav" -Repeat 2 -Providers speechmatics,deepgram
```

---

## Common Pitfalls

### Benchmarking with `--speed 0` is misleading for streaming

Real-time streaming APIs (Speechmatics, AssemblyAI) process audio at ~1x speed regardless of how fast you send it. Using `--speed 0` dumps all audio instantly but the server still takes ~N seconds for N seconds of audio. The total wall time will be roughly `audio_duration + tail_latency`, making streaming *appear* slower than batch. Always use `--speed 1` for realistic streaming benchmarks.

### Speechmatics batch uses a WebSocket (not HTTP)

Unlike most providers, Speechmatics batch goes through a **real-time WebSocket endpoint** (`eu.rt`). Audio is sent as binary WebSocket frames and transcripts arrive as JSON messages. This means even "batch" has the overhead of a WebSocket handshake + message framing.

Previously, the batch code waited for an `AudioAdded` ack after *every* chunk before sending the next one. For a 21-second recording at 100ms chunks, that's ~210 sequential round-trips to EU servers — easily 10-20 seconds of pure network latency. This was fixed to send all audio concurrently while draining messages.

### Fireworks segment stability

Fireworks sends cumulative full-transcript updates, not discrete partials. Without stability detection, live output would retype the entire transcript on every update. The stability algorithm in `run_streaming_task` tracks per-segment repetition counts to commit only stable (finalized) segments. The `SEGMENT_AGE_COMMIT_SECS` fallback ensures text appears even during short recordings where `SEGMENT_STABILITY_THRESHOLD` may never be reached.

### Live output is experimental

The UI marks this as "(experimental)" because:
- Provider-specific quirks (Fireworks stability heuristics, AssemblyAI turn boundaries) can cause jumpy output
- Some providers may duplicate or reorder text during rapid speech
- Target identity may be unavailable in an application. Kolboo then keeps preview/final output; changing a verified target stops live insertion rather than typing into the new target.

---

## Catalog and streaming review (2026-10-08)

Keep session lifecycle, cancellation, finalization and shared audio normalization
in the existing shared modules. Provider-local adapters own the wire protocol and
turn accumulation. Similar JSON messages are not grounds for a new shared adapter.

The shared searchable picker now separates **Kolboo Managed** and **Your models**,
filters by the action's capabilities, and explains readiness and limitations.
Provider key setup remains separate from choosing a model. Defaults remain
distinct from explicit selections; Managed choices are server-authoritative.
Saved unavailable selections remain visible, never silently substituted.

The recording entry point (`core/recording.rs`) starts concurrent input independently
of `stt_live_output`; the model selector should explain that network timing. The
pipeline owns the active session and cancellation token. Connection/finalization
are cancellation-aware, and the session now owns an abort-on-drop provider task
with bounded finalization and fail-closed, buffered audio delivery. Batch fallback
is blocked once live insertion has started. Late connection handoff after stop,
target changes and pinning the original model/payment source for fallback
are covered by owned session epochs, retained audio, target leases and focused
protocol/native fixtures. Windows/macOS native smoke checks remain separate from
Linux fixture evidence. Do not introduce a second streaming state machine in the UI.

AssemblyAI Sync/Dictation/3.5 realtime and Deepgram Flux remain separate protocol
work. YiddishLabs uses its documented Flash and Standard HTTP contracts, not
realtime. Local contract tests do not establish language accuracy; representative
opt-in audio and paid provider testing require credentials and separate approval.

## Adding Streaming to a New Provider

1. Implement `run_streaming_task()` — an async function that:
   - Connects to the provider's WSS endpoint
   - Uses `tokio::select!` to concurrently:
     - Read audio chunks from `audio_rx` (mpsc receiver of `Vec<f32>`) → convert to the documented encoding → send binary or base64 JSON frames as required by that provider
     - Read WS messages → reconcile provider ordering/deduplication → send `PartialTranscript` values via `partial_tx`
   - Require complete audio delivery and the provider's final acknowledgement;
     do not treat an early socket close or a provisional tail as success
   - On `audio_rx` close (recording stopped), send end-of-stream signal and drain final results

2. Override trait methods on your `SttProvider` impl:
   ```rust
   fn supports_streaming(&self) -> bool { true }

   async fn start_streaming(&self, sample_rate: u32) -> Result<StreamingSttSession, SttError> {
       let (audio_tx, audio_rx) = tokio::sync::mpsc::channel(128);
       let (partial_tx, partial_rx) = tokio::sync::mpsc::channel(64);

       let handle = tokio::spawn(async move {
           Self::run_streaming_task(audio_rx, partial_tx, sample_rate, /* ... */).await
       });

       Ok(StreamingSttSession::new(audio_tx, partial_rx, handle))
   }
   ```

3. If the provider is streaming-only (no batch endpoint), also override:
   ```rust
   fn requires_streaming(&self) -> bool { true }
   ```

4. Add the provider to the CLI's `stream` subcommand model mappings in `cli/pipeline.rs`.

---

## File Map

| File | What it does |
|------|-------------|
| [stt/mod.rs](../../app/src-tauri/src/stt/mod.rs) | `SttProvider` trait, `SttRegistry`, `AudioFormat` |
| [stt/streaming.rs](../../app/src-tauri/src/stt/streaming.rs) | `StreamingSttSession` struct |
| [stt/deepgram.rs](../../app/src-tauri/src/stt/deepgram.rs) and [realtime.rs](../../app/src-tauri/src/stt/deepgram/realtime.rs) | Deepgram batch (HTTP) + streaming (WSS) |
| [stt/fireworks.rs](../../app/src-tauri/src/stt/fireworks.rs) and [realtime.rs](../../app/src-tauri/src/stt/fireworks/realtime.rs) | Fireworks batch (HTTP) + streaming (WSS) with stability algorithm |
| [stt/speechmatics.rs](../../app/src-tauri/src/stt/speechmatics.rs) and [realtime.rs](../../app/src-tauri/src/stt/speechmatics/realtime.rs) | Speechmatics batch (WSS) + streaming (WSS) |
| [stt/elevenlabs.rs](../../app/src-tauri/src/stt/elevenlabs.rs) and [realtime.rs](../../app/src-tauri/src/stt/elevenlabs/realtime.rs) | ElevenLabs batch (HTTP) + streaming (WSS) |
| [stt/openai.rs](../../app/src-tauri/src/stt/openai.rs) and [realtime.rs](../../app/src-tauri/src/stt/openai/realtime.rs) | OpenAI batch (HTTP) + streaming (Realtime WSS) |
| [stt/assemblyai.rs](../../app/src-tauri/src/stt/assemblyai.rs) and [realtime.rs](../../app/src-tauri/src/stt/assemblyai/realtime.rs) | AssemblyAI batch (HTTP) + streaming (WSS v3) |
| [stt/simulated_streaming.rs](../../app/src-tauri/src/stt/simulated_streaming.rs) | Explicit advanced opt-in simulation using additional batch requests; not realtime |
| [pipeline.rs](../../app/src-tauri/src/pipeline.rs) | Recording state machine, streaming session lifecycle, live output |
| [cli/pipeline.rs](../../app/src-tauri/src/cli/pipeline.rs) | CLI `pipeline transcribe` and `pipeline stream` subcommands |
| [scripts/benchmark-streaming-vs-batch.ps1](../../scripts/benchmark-streaming-vs-batch.ps1) | Benchmark script for comparing batch vs streaming across providers |
