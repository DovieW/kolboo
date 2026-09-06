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
- Stop submits one final recording through the selected provider and current
  rewrite settings, producing one History/playback entry. Nothing is transcribed
  while Home capture is running. Internal encoding blocks are not transcription
  requests. Speaker diarization and live captions are not implemented.
- Final audio is normalized to mono 16 kHz. Providers still impose their own
  file-size, duration, and timeout limits: a rejected long recording remains
  saved for another attempt. Automatic provider-limit splitting is not performed.
  The current app's 50 MiB transcription ceiling permits about 27 minutes of
  mono 16-kHz WAV; longer recordings are safely retained, not split or discarded.
  Final preparation checks that ceiling before encoding and uses bounded raw
  blocks, but the current provider API still holds the final WAV and pipeline
  copies in memory. If the ceiling is raised, four hours needs about 440 MiB per
  copy. Multi-hour provider acceptance is not yet verified.
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
- Home lists interrupted recordings. Transcribe processes the complete recording;
  a successful History row prevents resubmission after a crash before cleanup.
  Cancellation, provider errors, and history persistence errors retain the source.
- Legacy section progress is ignored when preparing a full final transcription;
  existing section History rows are preserved, not silently deleted.
- Successful completion removes the raw journal and any legacy cursor. Discard
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
partial crash frames, complete final WAV preparation, cancellation, duration limits, and entitlement
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
