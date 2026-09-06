# Home and meeting recording

## Behavior

- Home has a floating recorder with Record, Pause/Resume, Stop & transcribe,
  Cancel, elapsed captured time, and an opt-in Computer audio switch.
- Home recordings save transcripts to History, never type or paste into another
  application. That output mode belongs to the Rust session and also applies
  when F3 stops a Home recording. Ordinary F3 dictation is unchanged.
- Pausing keeps capture devices open but excludes paused samples. The elapsed
  counter measures retained audio, not wall-clock time. Ordinary F3 sessions do
  not offer meeting pause controls.
- Stop transcribes sequential 30-second sections through the selected provider
  and current rewrite settings. Each section has its own History/playback entry.
  This is not yet a single meeting document, speaker diarization, or live captions.
- Cancel during capture discards that capture. Cancel during transcription keeps
  remaining recovery audio; completed sections remain in History.

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
No provider request is made until Stop & transcribe or Recover & transcribe.

- Append-only audio is synced approximately once per second and at normal stop.
  An abrupt process crash can lose the unsynced tail; incomplete final frames are
  ignored. The audio journal retains samples beyond the ordinary memory ring.
- Journals use owner-only file/directory permissions on Unix. Audio is not
  encrypted on disk; OS account and disk encryption protect it.
- A recording is limited to four hours or 2 GiB of raw audio, whichever comes
  first. Storage/capture failure stops the session and exposes retained audio for
  recovery. Recovery files are not automatically purged on an age timer.
- Home lists interrupted recordings. Recover resumes after durable progress;
  already successful History sections cover the history/progress crash window.
  Cancellation, provider errors, and history persistence errors retain the source.
- Successful completion removes the raw journal and cursor. Discard removes the
  selected journal. Delete all recordings includes recovery journals and rejects
  deletion while capture/transcription is active. Completed section WAVs use the
  existing recording store and its controls.
- Recovery is exclusive with new recording and other retry commands. The
  recovery cancellation token persists across gaps between sections.

## Entitlement status correction

A freshly validated Active entitlement without an expiration date remains
Active during its seven-day validation window. A refresh failure enters Grace;
an explicitly Expired entitlement is not revived by cached timestamps. Community
operation remains available without managed access.

## Validation and remaining acceptance

Deterministic tests cover session output ownership, pause state, recovery job
exclusivity/cancellation, journal data beyond the memory ring, explicit discard,
partial crash frames/cursors, progress resumption, duration limits, and entitlement
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
