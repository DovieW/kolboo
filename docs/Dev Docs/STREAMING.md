# Streaming STT — Developer Guide

## Overview

Kolboo supports two transcription modes:

| Mode | How it works | When text appears |
|------|-------------|-------------------|
| **Batch** | Record → stop → upload all audio → get transcript | After recording stops (+ network round-trip) |
| **Streaming** | Open WebSocket at record-start → feed audio chunks during recording → get partials in real time | Words appear *while* you speak |

Streaming is toggled by the **"Live Output (experimental)"** setting (`stt_live_output`). When enabled and the selected provider supports it, the pipeline opens a streaming session at recording start and types partial transcripts into the target window as you speak.

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
                                        │  (live output loop)  │
                                        │  types partials into │
                                        │  target window       │
                                        └──────────────────────┘
```

### Key types

- **`SttProvider` trait** ([stt/mod.rs](app/src-tauri/src/stt/mod.rs)) — every provider implements `transcribe()` (batch). Providers that support streaming also override `supports_streaming()` → `true` and implement `start_streaming()`.
- **`StreamingSttSession`** ([stt/streaming.rs](app/src-tauri/src/stt/streaming.rs)) — holds `audio_tx` (send chunks) and `partial_rx` (receive transcripts). Call `finalize()` when recording stops to get the final text.
- **`Pipeline`** ([pipeline.rs](app/src-tauri/src/pipeline.rs)) — orchestrates everything. During recording, it routes live audio to the session's `audio_tx` and reads `partial_rx` to drive live output. On stop, it calls `finalize()` and falls back to batch if streaming failed (unless `requires_streaming()` is true).

### Pipeline flow (streaming enabled)

1. User presses record hotkey
2. Pipeline calls `stt_provider.start_streaming(sample_rate)` → gets `StreamingSttSession`
3. Audio capture callback sends f32 samples via `live_audio_tx` → session's `audio_tx`
4. Provider's background task converts f32→PCM, sends over WebSocket, reads partials
5. Pipeline reads `partial_rx`, types partials into the active window
6. User releases hotkey → pipeline calls `session.finalize()` → gets final transcript
7. Final transcript goes through the rewrite/prompt pipeline as usual

---

## Provider Support Matrix

| Provider | Batch | Streaming | Batch Transport | Stream Transport | Partials? | Notes |
|----------|:-----:|:---------:|----------------|-----------------|:---------:|-------|
| **Speechmatics** | ✅ | ✅ | WSS (`eu.rt`) | WSS (`eu2.rt`) | ✅ `AddPartialTranscript` | Batch uses RT endpoint but sends all audio upfront |
| **Fireworks** | ✅ | ✅ | HTTP POST | WSS | ✅ (custom stability logic) | Needs segment stability + age-based commit |
| **ElevenLabs** | ✅ | ✅ | HTTP POST | WSS | ✅ | Streaming can be 20x+ faster than batch |
| **OpenAI** | ✅ | ✅ | HTTP POST | WSS (Realtime API) | ✅ | `gpt-4o-realtime-transcribe` is stream-only |
| **AssemblyAI** | ✅ | ✅ | HTTP POST | WSS (v3) | ✅ turn-based | Uses `turn_is_formatted` for final turns |
| **Groq** | ✅ | ❌ | HTTP POST | — | — | |
| **Deepgram** | ✅ | ✅ | HTTP POST | WSS (`/v1/listen`) | ✅ `is_final` flag | Same models work for batch + streaming |
| **Aquavoice** | ✅ | ❌ | HTTP POST | — | — | |
| **Local Whisper** | ✅ | ❌ | Local process | — | — | |

---

## How Each Streaming Provider Works

### Speechmatics

Two separate WebSocket endpoints:
- **Batch** (`transcribe_ws`): connects to `wss://eu.rt.speechmatics.com/v2/`, sends `StartRecognition`, fires all audio chunks concurrently while draining server messages, then sends `EndOfStream` and waits for `EndOfTranscript`.
- **Streaming** (`run_streaming_task`): connects to `wss://eu2.rt.speechmatics.com/v2/`, uses `tokio::select!` to concurrently send audio chunks from `audio_rx` and receive `AddPartialTranscript` / `AddTranscript` messages.

### Fireworks

Single WSS endpoint for streaming: `wss://audio-streaming.api.fireworks.ai/v1/audio/transcriptions/streaming`.

Fireworks doesn't distinguish "partial" vs "final" — every response replaces the full transcript. Kolboo uses a custom **segment stability algorithm** to decide when to commit text for live output:

- `SEGMENT_STABILITY_THRESHOLD = 3` — commit a segment after 3 consecutive identical updates
- `SEGMENT_AGE_COMMIT_SECS = 1.5` — time-based fallback: commit after 1.5s if the segment has been stable for at least 1 update (prevents text from never appearing during short recordings)

### ElevenLabs

WSS at `wss://api.elevenlabs.io/v1/speech-to-text/realtime`. Sends base64-encoded PCM chunks as JSON. Receives partial transcripts with word-level timing.

### OpenAI

Uses the Realtime API at `wss://api.openai.com/v1/realtime?intent=transcription`. Sends binary PCM chunks, receives structured transcription events. The `gpt-4o-realtime-transcribe` model is streaming-only (`requires_streaming() = true`).

### Deepgram

WSS at `wss://api.deepgram.com/v1/listen`. Same endpoint for batch and streaming — the same models (nova-3, nova-2, etc.) work in both modes. Query params control streaming behavior:
- `interim_results=true` — receive partial transcripts as speech is processed
- `endpointing=300` — detect end of utterances after 300ms silence
- `utterance_end_ms=1500` — emit `UtteranceEnd` event after 1.5s silence
- `smart_format=true` + `punctuate=true` — clean formatted output

Protocol:
- Send binary PCM s16le mono audio chunks
- Receive `{"type": "Results", "is_final": bool, "channel": {"alternatives": [{"transcript": "..."}]}}`
- `is_final: false` → interim partial (overlay update only)
- `is_final: true` → finalized segment (commit for live paste)
- On recording stop: send `{"type": "Finalize"}` to flush, then `{"type": "CloseStream"}` to close

### AssemblyAI

WSS at `wss://streaming.assemblyai.com/v3/ws`. Turn-based protocol — the server groups speech into "turns" and sends partial/final updates per turn. Uses `turn_is_formatted: true` for punctuated final output.

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
kolboo.exe pipeline stream --provider fireworks --wav "audio.wav" --speed 0

# With language
kolboo.exe pipeline transcribe --provider fireworks --model whisper-v3-turbo --wav "audio.wav" --language en
```

Output is JSON: `{"text": "...", "elapsed_ms": 1234}`.

There's also a benchmark script at `scripts/benchmark-streaming-vs-batch.ps1`:
```powershell
.\scripts\benchmark-streaming-vs-batch.ps1 -WavFile "C:\path\to\audio.wav" -Repeat 2 -Providers speechmatics,fireworks
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
- The live output types into whatever window is focused — if the user switches windows mid-recording, text goes to the wrong place

---

## Adding Streaming to a New Provider

1. Implement `run_streaming_task()` — an async function that:
   - Connects to the provider's WSS endpoint
   - Uses `tokio::select!` to concurrently:
     - Read audio chunks from `audio_rx` (mpsc receiver of `Vec<f32>`) → convert to PCM → send as binary WS frames
     - Read WS messages → parse partials → send via `partial_tx` (mpsc sender of `String`)
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
| [stt/mod.rs](app/src-tauri/src/stt/mod.rs) | `SttProvider` trait, `SttRegistry`, `AudioFormat` |
| [stt/streaming.rs](app/src-tauri/src/stt/streaming.rs) | `StreamingSttSession` struct |
| [stt/deepgram.rs](app/src-tauri/src/stt/deepgram.rs) | Deepgram batch (HTTP) + streaming (WSS) |
| [stt/fireworks.rs](app/src-tauri/src/stt/fireworks.rs) | Fireworks batch (HTTP) + streaming (WSS) with stability algorithm |
| [stt/speechmatics.rs](app/src-tauri/src/stt/speechmatics.rs) | Speechmatics batch (WSS) + streaming (WSS) |
| [stt/elevenlabs.rs](app/src-tauri/src/stt/elevenlabs.rs) | ElevenLabs batch (HTTP) + streaming (WSS) |
| [stt/openai.rs](app/src-tauri/src/stt/openai.rs) | OpenAI batch (HTTP) + streaming (Realtime WSS) |
| [stt/assemblyai.rs](app/src-tauri/src/stt/assemblyai.rs) | AssemblyAI batch (HTTP) + streaming (WSS v3) |
| [stt/simulated_streaming.rs](app/src-tauri/src/stt/simulated_streaming.rs) | Test helper that simulates streaming from batch providers |
| [pipeline.rs](app/src-tauri/src/pipeline.rs) | Recording state machine, streaming session lifecycle, live output |
| [cli/pipeline.rs](app/src-tauri/src/cli/pipeline.rs) | CLI `pipeline transcribe` and `pipeline stream` subcommands |
| [scripts/benchmark-streaming-vs-batch.ps1](scripts/benchmark-streaming-vs-batch.ps1) | Benchmark script for comparing batch vs streaming across providers |
