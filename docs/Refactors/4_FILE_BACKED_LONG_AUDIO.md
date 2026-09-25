# File-backed final transcription for long recordings

Status: concrete follow-up, not implemented by the September desktop QoL batch.

## Evidence and scope

Local import decoding is packet/block-bounded, and recording journals stream to
disk. However, `audio_capture/journal.rs::final_wav` returns the complete final WAV;
`commands/recording.rs::recover_recording_owned` saves it, and
`retry_transcription_inner` loads and clones it for the existing pipeline.
`pipeline.rs::transcribe_saved_audio` and `pipeline/meeting_transcription.rs` accept
whole-recording bytes. A four-hour mono 16 kHz PCM16 WAV is roughly 440 MiB per copy,
before decoded samples and upload buffers. The four-hour limit is not a claim of
low or constant peak memory, and physical multi-hour acceptance is outstanding.

This seam now serves both Home recordings and imported files. Before expanding
long-recording use on memory-constrained machines, make these existing owners
file-backed rather than adding a separate import-only transcription pipeline:

- Let journal finalization stream into a private temporary WAV and atomically
  publish it through RecordingStore; retain the recovery source until History
  commits successfully.
- Expose a bounded sample-range reader for the existing quiet-boundary upload
  splitter, hashing and resumable checkpoints. Preserve exact sample ownership,
  one complete playback file, mode/model snapshots and one final History result.
- Keep small dictation adapters compatible; do not require every provider to
  implement a new transport interface just to avoid local whole-file copies.
- Make blocking preparation keep its exclusive job lease until the worker exits,
  including when the owning async future is dropped. Ordinary cancellation today
  awaits preparation; cancelling a token during Drop is not a join guarantee.

Validation should measure peak allocations on synthetic long files, prove bounded
reads without hours of capture, and retain tests for cancellation, checkpoint
resumption, cleanup-only retries, source preservation and no duplicate completed
uploads. Do not solve resource pressure by silently truncating audio or lowering
the displayed duration limit without a product decision.
