# STT Realtime (Implementation Detail) + Live Mode (User Setting)

This document outlines a simple plan to:

1) Use *realtime/streaming STT* automatically when a provider truly supports it (implementation detail).
2) Warn users when they select a provider that is *batch-only* (notably Groq STT in our current integration).
3) Add a user-facing **Live mode** setting that disables rewrite and outputs text incrementally (“chunk-by-chunk”).

---

## Why we’re doing this

Today, Kolboo feels “batch”: you record, stop, then wait for transcription + optional routing/rewriting before you see/paste anything.

The goal is to reduce perceived latency:

- **Normal mode:** keep high-quality final output (rewrite, routing, etc.)
- **Live mode:** prioritize speed and immediacy (incremental output; no rewrite)

---

## Definitions (so we don’t get confused by the word “streaming”)

There are three different things people often call “streaming”:

1) **Realtime STT session** (true streaming)
   - Audio is sent while recording (typically WebSocket)
   - Provider can return *partials* and a final transcript
   - This is the only kind that can meaningfully reduce end-to-end latency

2) **Chunked batch STT** (fake streaming)
   - We split audio into chunks and call the normal “transcribe this file” endpoint repeatedly
   - Can produce incremental output, but often increases cost and may reduce accuracy unless we implement overlap + merging

3) **SSE/token streaming for chat**
   - This is for LLM text generation, not audio transcription

In this plan:

- “Realtime” = (1)
- “Live mode” = user experience of incremental output (via (1) or (2))

---

## Current state (as of Feb 2026)

- Pipeline behavior is “record → stop → transcribe → (route) → (rewrite) → transcript ready”.
- Backend emits final-only transcript events (no partial transcript events today).
- **Groq STT integration** uses a single multipart upload to an OpenAI-compatible `/audio/transcriptions` endpoint (batch).
- **Speechmatics** already uses a WebSocket flow internally, but we currently disable partials and only emit final text.

Relevant files:

- `app/src-tauri/src/pipeline.rs`
- `app/src-tauri/src/pipeline/stt_flow.rs`
- `app/src-tauri/src/events.rs`
- `app/src-tauri/src/stt/mod.rs`
- `app/src-tauri/src/stt/groq.rs`
- `app/src-tauri/src/stt/speechmatics.rs`

---

## Product rules (the “simple plan”)

### Rule A — Realtime is automatic (implementation detail)

If a provider supports a true realtime STT session, Kolboo should just use it when it improves latency.

Users should not need to know *how* the provider works internally unless it affects UX.

### Rule B — Warn when provider is batch-only

If a provider can only do batch transcription (or we only support it in batch mode), show a clear warning:

- “This provider transcribes after you stop recording. Expect higher latency.”

This is especially important for Groq (given our current OpenAI-compatible integration).

### Rule C — Live mode is a user-facing setting

Add a **Live mode (no rewrite)** toggle that:

- Disables rewrite (and routing, if routing depends on the final transcript)
- Produces incremental output (UI shows it / paste/typing happens progressively)

Provider gating:

- If the selected provider can do realtime STT with partials, Live mode is available.
- If incremental output would require chunked batch STT and that’s known to be costly/poor UX (e.g., Groq minimum-billing quirks), hide or disable Live mode.

---

## Provider capability snapshot (initial)

This is intentionally conservative; we’ll expand it after auditing other providers.

| Provider | True realtime session? | Partials available? | Live mode initially? | Notes |
|---------|-------------------------|--------------------|----------------------|------|
| Speechmatics | Yes (WebSocket) | Yes (if enabled) | Yes | We already have WS transport; we currently set `enable_partials: false`. |
| Groq Whisper (OpenAI-compat) | No (batch in our current integration) | N/A | No (hidden/disabled) | Show warning: batch-only, higher latency. |
| Others (Deepgram, ElevenLabs, etc.) | TBD | TBD | TBD | Audit later; add capabilities one-by-one. |

---

## UX details

### 1) Groq “no streaming” warning

Where it should appear:

- Settings page where the STT provider is selected
- Anywhere else we present “latency expectations” (optional)

Text (draft):

- **Groq STT is batch-only in Kolboo right now.** You’ll only get a transcript after you stop recording, so latency is higher than realtime providers.

### 2) Live mode toggle

- Label: **Live mode (no rewrite)**
- Description: “Shows/outputs text as you speak. Faster, but less polished.”
- Behavior changes:
  - Disable rewrite step
  - Disable any UI that assumes a final post-processed transcript (or mark it as not available)

### 3) Live output behavior

Two variants we may choose between (start with the simplest):

- **Variant 1 (simplest):** show partial transcript in UI, but only paste/type the final transcript on stop
  - Pros: avoids annoying mid-sentence typing changes
  - Cons: doesn’t fully match “chunk-by-chunk” paste requirement

- **Variant 2 (true live typing):** paste/type incremental chunks
  - Pros: feels extremely fast
  - Cons: partials often revise earlier text; we need a strategy to avoid thrashing

If the intent is specifically “paste chunk by chunk”, we should do Variant 2, but it requires clear rules.

Recommended initial rule for Variant 2:

- Only output “stable” chunks (e.g., only commit text after we receive a final segment / end-of-utterance marker / punctuation boundary / stable partial window)

---

## Backend implementation sketch

### Step 0 — Add latency instrumentation (before we change behavior)

Add structured timings around:

- stop-recording → transcription started
- time spent in provider STT
- time spent waiting for OCR (if applicable)
- time spent routing
- time spent rewriting

This tells us where the real latency is for each provider.

### Step 1 — Add provider capability flags

Add a way to answer:

- Does this provider support realtime sessions?
- Does it support partials?

Implementation options:

- Add a `capabilities()` method on the STT provider type
- Or keep a static registry mapping provider enum → capability flags

### Step 2 — Add partial transcript events

Add new events (names TBD; keep them consistent with existing pipeline events):

- “partial transcript updated” (payload: text + stability metadata if available)
- “final transcript ready” (payload: final text)

### Step 3 — Speechmatics Live mode

When Live mode is enabled and Speechmatics is selected:

- Enable partials in the Speechmatics config
- Start the WS transcription session when recording starts
- Forward partial transcripts to the frontend via the new event
- On stop, finalize and emit final transcript

### Step 4 — Normal mode stays “final-only”

When Live mode is OFF:

- Keep the current behavior (emit only final transcript)
- (Optional) still use realtime transport internally if it reduces time-to-final, but do not surface partials

---

## Frontend implementation sketch

### Settings gating logic

- If provider is Groq:
  - Show “batch-only” warning
  - Hide/disable Live mode toggle (with a tooltip/explanation)

- If provider supports realtime partials:
  - Show Live mode toggle

### Live transcript UI

- Display partial transcript updates in-place
- If we implement live typing:
  - Maintain “committed text” vs “uncommitted partial”
  - Only commit text according to a stability rule

---

## Rollout checklist

### Phase 1 — Clarity + guardrails (small)

- [ ] Add Groq batch-only warning in settings
- [ ] Add Live mode toggle (gated; initially only available for Speechmatics)
- [ ] Ensure Live mode disables rewrite (and routing if needed)

### Phase 2 — Plumbing (medium)

- [ ] Add backend latency instrumentation logs (and surface a debug panel if we want)
- [ ] Add partial transcript event

### Phase 3 — Speechmatics realtime UX (medium)

- [ ] Enable Speechmatics partials in Live mode
- [ ] Forward partial transcript updates
- [ ] Add “commit strategy” for live typing (if we do chunk-by-chunk paste)

### Phase 4 — Expand provider support (later)

- [ ] Audit other STT providers for realtime capabilities
- [ ] Implement one provider at a time behind the same capability interface

---

## Testing plan

Keep tests deterministic (no network calls):

- Unit tests around settings gating (provider → Live mode available?)
- Unit tests around “commit strategy” for partial text (given a stream of partials, what gets committed?)
- If we add capability flags in Rust, add a small Rust unit test verifying the mapping

Manual verification:

- Speechmatics Live mode: partials appear; stop produces final transcript
- Normal mode: behavior unchanged (final-only)
- Groq selected: warning appears; Live mode not shown/disabled

---

## Risks / gotchas

- **Partials revision:** realtime STT partials can change earlier words; naive live typing will look glitchy.
- **Cost blowups:** chunked batch STT can increase billed audio time (especially providers with minimum billed durations).
- **UX confusion:** “Streaming” can be interpreted as “faster final transcript” or “live typing”; we should name it in UX as “Live mode (no rewrite)”.
- **Pipeline complexity:** we must keep cancellation and state transitions robust (don’t wedge pipeline on stop/cancel).
