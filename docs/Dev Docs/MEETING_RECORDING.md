# Home and meeting recording

## Behavior

- Home has a compact single-row floating recorder with icon-only Record, Pause/Resume,
  Stop & transcribe, Stop & save for later, and elapsed captured time. Active ordinary
  dictation and transcription still offer Cancel. Its options popover holds
  a remembered Dictation/Meeting selector. Meeting options include a separate model
  picker and Computer audio. Record has an accessible label but no tooltip. Detailed
  errors and saved recordings open a separate dialog; the popover has no scrollbar.
- Home recordings save transcripts to History, never type or paste into another
  application. That output mode belongs to the Rust session and also applies
  when F3 stops a Home recording. Ordinary F3 dictation is unchanged.
- Pausing keeps capture devices open but excludes paused samples. The elapsed
  counter measures retained audio, not wall-clock time. Ordinary F3 sessions do
  not offer meeting pause controls.
- Stop assembles one transcript and one successful History/playback entry. Dictation
  applies optional rewriting once; Meeting bypasses rewriting, routing, clipboard
  context and automatic OCR regardless of profile/preset settings. Nothing is
  transcribed while Home capture is running. There are no live captions.
- Recording preferences use the non-secret `recording_preferences` settings key.
  The mode, explicit meeting provider/model, and managed/BYOK route are snapshotted in an owner-only
  `.options.json` file before capture and copied alongside the complete saved WAV.
  Recovery and reruns use this snapshot, not the current popup selection. Missing
  legacy metadata means Dictation; unreadable metadata fails closed. Length never
  determines recording mode.
- Meeting's model picker lists enabled managed models and configured user-key/local
  options separately. Its route does not depend on or change Dictation's route.
  An unavailable managed choice fails with audio retained, never silently uses a key.
- Final audio is normalized to mono 16 kHz. After Stop, uploads contain at most
  ten minutes (~19.2 MB), below the managed gateway's 25 MB request limit. Cuts
  prefer a quiet boundary in the last ten seconds; sample ranges have no gaps or
  overlap. This is not periodic live transcription or separate History sections.
- The 50 MiB dictation limit remains unchanged. Meeting transcription and History
  reruns use a separate four-hour normalized-WAV ceiling. Capture is still limited
  to four hours or 2 GiB raw, whichever comes first (high-rate/stereo inputs can
  reach the disk ceiling sooner). Final WAV preparation uses bounded raw blocks;
  the current pipeline still holds the final WAV and copies in memory, about
  440 MiB per copy at four hours. Physical multi-hour acceptance remains unverified.
- Providers may impose shorter duration/timeouts or quotas. A failed upload keeps
  the complete source and completed progress for retry. Automatic splitting does
  not bypass quotas, authentication, or managed model policy.
- Escape or **Stop & save for later** during a Home capture stops it without transcription and keeps
  its recovery audio and original mode/model. This also applies while paused.
  Saved recordings offers Transcribe or an explicit, confirmed Discard action.
  Ordinary F3 cancellation stays unchanged: it does not start writing a recovery
  journal. Cancel during transcription keeps the full recovery audio.

## Local file imports

`recording_import_file` is an explicit user action from the Transcribe file page.
It takes an absolute local path plus a `RecordingPreferences` snapshot. Native
dialog/drag selection alone never calls it. No source path is put in settings,
History, request logs or browser local storage; the UI shows only the basename.

Local decoding uses the same Symphonia 0.5 family already used by rodio, enabling
WAV/PCM, MP3, FLAC and ADTS AAC. No external FFmpeg process,
system install, new provider adapter or credential is required. See the
[decoder's supported formats](https://docs.rs/symphonia/0.5.5/symphonia/).
Decode runs off the async runtime and webview in bounded packets/minute blocks,
downmixing/resampling to the existing mono 16 kHz journal format. Inputs are
regular local mono/stereo files up to 192 kHz, 2 GiB and four hours. Format changes,
known decode errors and declared lossless frame-count mismatches reject the import;
the decoder cannot detect an already-truncated but otherwise valid lossy source.
AIFF is not enabled: a valid synthetic fixture fails in the current decoder's
AIFF packet handling. MP4/M4A (including ALAC) and Ogg/Vorbis are also disabled
pending bounded container/decoder setup allocation support in the decoder family.
Their optional parsers are not enabled, rather than relying only on file extensions.
Supported codecs have offline synthetic fixture coverage, including malformed
input regressions. Filename extensions do not determine completeness checks.

A UUID-named private `.importing` file and options snapshot are staged first.
Only a completely decoded/synced file is renamed to `.pcm` and made recoverable;
an interrupted import is never presented as a complete recording. The original
source remains untouched. Interrupted staging is cleaned separately from complete
recovery audio. Subsequent upload failures return a recovery ID so the same page
offers resumable recovery, not a duplicate paid import. Native job ownership spans
decode, final WAV preparation and transcription, including cancellation. Existing
provider limits, managed policy and recorded route choices remain in force.

`FileImportResult.transcription_complete` distinguishes a saved History result from
a failed transcription even when temporary-file cleanup fails. Finish cleanup checks
the durable History completion marker before requiring PCM or submitting any audio.
Staging cleanup is fallible and preserves mode metadata if its audio cannot be
removed; Delete all recordings includes interrupted imports under exclusive ownership.
Only decoding and raw preparation are block-bounded: the existing final-WAV pipeline
still has the memory-copy ceiling described above.

The page remains mounted but hidden during navigation, preserving the selected
file and job state in memory. Native drag listeners detach off-page. No automatic
upload, microphone start, clipboard insertion or provider switching is introduced.
Complete imports share the recovery/History persistence and retention behavior
documented below. File-manager context menus, a separate drop overlay, video and
multiple-file queues remain separate features.

## Computer audio capabilities

Linux uses the system FFmpeg PulseAudio input adapter (`/usr/bin/ffmpeg`) and
`pactl`. PipeWire's PulseAudio compatibility server is supported. On Ubuntu,
install `ffmpeg` and `pulseaudio-utils`. A Homebrew FFmpeg build without PulseAudio
input support is insufficient.

Enabling the switch records the **system default microphone and default output
monitor**, mixed to mono 16 kHz. Microphone-only mode uses Kolboo's selected input.
The switch starts off and is locked during a recording. A failed capture startup
returns an error instead of silently recording microphone-only audio. Output
device changes during a recording require stopping and starting a new recording.

Windows and macOS still support the microphone pipeline, but computer-audio
capture is explicitly unavailable there until native adapters are implemented
and tested. No cross-platform computer-audio release claim is made.

## Recovery and privacy

Home recordings deliberately write raw audio into `meeting-recovery` under the
application data directory. This is additional local persistence even if normal
completed-recording retention is disabled.
No provider request is made until Stop & transcribe or the saved recording's
Transcribe action.

- Append-only audio is synced approximately once per second and at normal stop.
  An abrupt process crash can lose the unsynced tail; incomplete final frames are
  ignored. The audio journal retains samples beyond the ordinary memory ring.
- Stopping/cancelling a journal-backed Home recording drains the capture worker
  and syncs the retained tail, including when Hot Mic is enabled. Ordinary Hot Mic
  shortcut dictation keeps its armed stream. If a Home worker cannot shut down
  cleanly, the journal is frozen and a new capture is refused until app restart;
  the stopped audio remains available. Cancelling before the first audio callback
  removes empty recording metadata rather than creating an unusable recovery item.
- Journals use owner-only file/directory permissions on Unix. Audio is not
  encrypted on disk; OS account and disk encryption protect it.
- A recording is limited to four hours or 2 GiB of raw audio, whichever comes
  first. Storage/capture failure stops the session and exposes retained audio for
  recovery. Recovery files are not automatically purged on an age timer.
- Home lists interrupted recordings. Transcribe resumes completed upload results
  from owner-only `.transcripts` checkpoints in the recovery directory. They
  contain sensitive transcript text and optional speaker segments, are not encrypted by Kolboo, and are synced
  after each successful upload. A partial trailing checkpoint line is ignored.
  Cache keys bind the complete audio checksum, sample range, provider/model,
  language, transcription prompt, and Meeting's managed/BYOK choice. Changing these starts fresh uploads.
  A successful History row prevents resubmission after a crash before cleanup.
  Cancellation, provider errors, and history persistence errors retain the source.
- A crash after a provider finishes but before its checkpoint is synced can repeat
  that one upload; this is resumability, not exactly-once provider billing.
  Explicit History reruns of completed meetings are fresh attempts without a
  persistent partial-text cache. Failed original recovery remains resumable.
- Legacy section progress is ignored when preparing a full final transcription;
  existing section History rows are preserved, not silently deleted.
- Successful completion removes the raw journal, mode metadata, partial transcripts, and any legacy cursor. Discard
  removes the selected journal. Delete all recordings includes recovery journals and rejects
  deletion while capture/transcription is active. Completed recording WAVs use the
  existing recording store and its controls.
- Recovery is exclusive with new recording and other retry commands. The
  recovery cancellation token also covers final audio preparation.

## Meeting speaker labels

`gpt-4o-transcribe-diarize` uses the existing OpenAI BYOK adapter, or the existing
managed Edge path when authorized and enabled in the Edge catalog. No provider key
is provisioned by this feature and no model/default is automatically enabled.
Requests use `response_format=diarized_json` and `chunking_strategy=auto`, with no
prompt, timestamp-granularity or known-speaker-reference fields.

Each upload's text and speaker segments are checkpointed. The displayed document
contains simple speaker-labelled paragraphs and part boundaries. Speaker A in
different parts does **not** assert the same identity. Original STT text and segment
metadata remain separate from manual corrections; edited text is never falsely
realigned to timestamps. There is no speaker management or synchronized highlighting.

## History reader, playback, and corrections

- List queries return a bounded 320-character preview plus lightweight metadata;
  expanding one card loads its complete document on demand. Explicit Copy loads
  complete corrected text, never the preview. Existing filters/pagination remain.
- Cards expand rather than copy. Inline transcripts scroll after 300 px. Every card
  offers Open full view: title, fixed player, literal search with previous/next
  matches, Copy and explicit Edit mode, above one scrolling reading surface.
- One playback controller belongs to the History view. It creates a fresh HTML
  media element per source so late webview errors cannot affect a newer recording.
  Audio and waveform preparation are lazy until Play; playback pauses on collapse,
  modal close, switching entries and navigation, and preserves session positions.
  Controls include waveform seeking, keyboard seek, ±10 seconds,
  elapsed/total time and speed.
- Rust streams PCM to generate at most 4096 min/max waveform pairs, caches them
  beside the WAV and validates the source fingerprint. WaveSurfer renders these
  precomputed peaks; the webview does not decode the complete meeting to draw it.
  Kolboo's tokenized, process-local playback stream accepts only validated
  RecordingStore ids, supports bounded byte ranges and exposes no filesystem
  paths or recursive directory access. The generic Tauri asset protocol,
  whole-file base64 fallback and playback timeout are not used.
- Corrections are stored in `history-edits/<hashed-entry-id>.json`, owned by
  HistoryStorage. Original History output remains unchanged. Writes are serialized,
  use synced private temporary files and same-directory replacement, and reject
  stale revisions. Saves debounce at 600 ms with a five-second maximum while typing.
  Closing flushes pending edits; failed saves leave the draft available for copying,
  retry or explicit conflict resolution. Unsaved drafts survive view navigation in
  memory, not a process crash. No transcript is put in browser local storage.
- Restore original restores text without resetting the revision; reruns create
  separate entries referencing the same recording. Search, previews, Copy, analysis
  and existing exports use corrected text. Retention/deletion remove correction
  sidecars and original metadata with their entry; audio deletion removes waveform
  caches and recording options. Delete-transcripts also clears speaker metadata
  and persists a revision barrier: delayed autosaves and stale correction sidecars
  cannot restore cleared text after a restart.

Focused tests use synthetic audio, mocked provider HTTP and a local DOM environment.
Visual acceptance should use user screenshots of a collapsed card, expanded card,
full reader and recorder popover. Production enablement, paid provider smoke tests
and release publication are separate rollout actions, not part of this change.

## Entitlement status correction

A freshly validated Active entitlement without an expiration date remains
Active during its seven-day validation window. A refresh failure enters Grace;
an explicitly Expired entitlement is not revived by cached timestamps. Community
operation remains available without managed access.

## Validation and remaining acceptance

The prior History/recording-mode checkpoint (`daaedb5`, before the desktop QoL
batch) passed the desktop's full Rust test command
(829 passed, 12 ignored), frontend tests (664 passed, 57 skipped), typechecking,
lint, formatting, Knip, renderer production build and the local-Whisper compile
check. API Edge's full suite passes (153 tests), including mocked enabled/disabled
diarization routes. No real credentials or paid API calls are needed by these tests.
Native playback, visual acceptance, and physical recording of this redesign still
require an app restart and manual checks. No release or deployment was performed.

For the newer desktop/file-import batch's checks and remaining acceptance, see
`DESKTOP_QOL_BATCH.md`; the counts above are checkpoint evidence, not a claim
about subsequent changes.

### Earlier recorder baseline checks (before the History redesign)

Deterministic tests cover session output ownership, pause state, recovery job
exclusivity/cancellation, journal data beyond the memory ring, explicit discard,
partial crash frames, complete final WAV preparation, post-stop upload assembly,
checkpoint resumption, cancellation, isolated size limits, and entitlement
status. Frontend tests cover the controls and invoke contracts.

The Linux FFmpeg adapter was exercised against an isolated PulseAudio null sink,
including pause/resume and shutdown, without recording a physical microphone or
contacting a provider. Source-app startup and rendering were checked on this
machine. Physical microphone/speaker combinations, multi-hour recordings, sudden
power loss, and Windows/macOS acceptance remain manual checks—not completed
release evidence. The published beta does not include these changes until a new
release is built and authorized.

On this Ubuntu desktop, the final native-window check showed missing painted
content until the process was launched with `WEBKIT_DISABLE_DMABUF_RENDERER=1`.
That workaround is set only on the running local test service, not globally or
in packaged platform defaults. The final window then rendered correctly. The
optional cold Clippy run was stopped to keep the handoff bounded; it is not
reported as a passing check.
