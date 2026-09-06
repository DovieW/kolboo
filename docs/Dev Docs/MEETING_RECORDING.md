# Home and meeting recording

## Behavior

- Home has a compact single-row floating recorder with icon-only Record, Pause/Resume,
  Stop & transcribe, Cancel, and elapsed captured time. Its options popover holds
  Computer audio and recovery actions. Tooltips explain icons; errors open that popover
  without expanding the bar; saved recordings highlight the options button.
- Home recordings save transcripts to History, never type or paste into another
  application. That output mode belongs to the Rust session and also applies
  when F3 stops a Home recording. Ordinary F3 dictation is unchanged.
- Pausing keeps capture devices open but excludes paused samples. The elapsed
  counter measures retained audio, not wall-clock time. Ordinary F3 sessions do
  not offer meeting pause controls.
- Stop transcribes the saved recording, assembles one transcript, and applies
  current rewrite settings once, producing one successful History/playback entry.
  Nothing is transcribed while Home capture is running. Speaker diarization and
  live captions are not implemented.
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
- Cancel during capture discards that capture. Cancel during transcription keeps
  the full recovery audio.

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
completed-recording retention is disabled; the recorder explains it before use.
No provider request is made until Stop & transcribe or the saved recording's
Transcribe action.

- Append-only audio is synced approximately once per second and at normal stop.
  An abrupt process crash can lose the unsynced tail; incomplete final frames are
  ignored. The audio journal retains samples beyond the ordinary memory ring.
- Journals use owner-only file/directory permissions on Unix. Audio is not
  encrypted on disk; OS account and disk encryption protect it.
- A recording is limited to four hours or 2 GiB of raw audio, whichever comes
  first. Storage/capture failure stops the session and exposes retained audio for
  recovery. Recovery files are not automatically purged on an age timer.
- Home lists interrupted recordings. Transcribe resumes completed upload results
  from owner-only `.transcripts` checkpoints in the recovery directory. They
  contain sensitive transcript text, are not encrypted by Kolboo, and are synced
  after each successful upload. A partial trailing checkpoint line is ignored.
  Cache keys bind the complete audio checksum, sample range, provider/model,
  language, and transcription prompt. Changing these starts fresh uploads.
  A successful History row prevents resubmission after a crash before cleanup.
  Cancellation, provider errors, and history persistence errors retain the source.
- A crash after a provider finishes but before its checkpoint is synced can repeat
  that one upload; this is resumability, not exactly-once provider billing.
  Explicit History reruns of completed meetings are fresh attempts without a
  persistent partial-text cache. Failed original recovery remains resumable.
- Legacy section progress is ignored when preparing a full final transcription;
  existing section History rows are preserved, not silently deleted.
- Successful completion removes the raw journal, partial transcripts, and any legacy cursor. Discard
  removes the selected journal. Delete all recordings includes recovery journals and rejects
  deletion while capture/transcription is active. Completed recording WAVs use the
  existing recording store and its controls.
- Recovery is exclusive with new recording and other retry commands. The
  recovery cancellation token also covers final audio preparation.

## Entitlement status correction

A freshly validated Active entitlement without an expiration date remains
Active during its seven-day validation window. A refresh failure enters Grace;
an explicitly Expired entitlement is not revived by cached timestamps. Community
operation remains available without managed access.

## Validation and remaining acceptance

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
