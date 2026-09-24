# Recordings and History

The small microphone button on Home starts a recording without a shortcut.
The options button lets you choose **Dictation** or **Meeting**; Kolboo remembers
the choice. Recordings started here stay in History instead of being pasted into
another app. F3 dictation keeps its normal insertion behavior.

## Transcribe an audio file

Open **Transcribe file** in the sidebar. Drop one audio file onto the
page or choose it with the file picker. Selection is local: audio is not uploaded
until you press **Transcribe**. WAV, MP3, FLAC and AAC (ADTS)
are supported, with mono/stereo audio up to four hours and a 2 GiB source-file
limit. M4A/MP4, AIFF, Ogg, other codecs, video files and multi-track selection are
not supported yet. Export these recordings as WAV, MP3 or FLAC before importing.

Choose a transcription model directly. **Speaker labels** filters the picker to
diarization-capable models; changing the toggle clears the selection rather than
silently changing providers. File imports never run rewriting. These file-specific
choices do not overwrite the recorder's saved preferences. Managed choices must
already be enabled for your account. Before you select a file or change its
options, this page follows the recorder's saved meeting model, if present.
Once selected, your file's options stay unchanged
when you navigate away or change the recorder defaults elsewhere.

The full audio is prepared locally and submitted in bounded uploads; the complete
transcript and playable recording appear in History. You can navigate to another
page while it runs. Cancel during preparation leaves the original file unchanged;
cancel or failure after preparation keeps a complete local copy. **Retry saved audio**
resumes completed uploads, also available from Home → Recording options → Saved
recordings. Starting a new import instead is a new request and can repeat provider
charges. A crash can repeat the last upload if its result wasn't saved yet.

If the transcript was saved but temporary-file cleanup failed, **Open History** is
still available. **Finish cleanup** retries local cleanup without transcribing again.

Imported audio is copied into Kolboo's local storage even when normal dictation
audio retention is off, so History playback and recovery work. Completed copies
follow recording retention; unfinished copies remain until transcribed or explicitly
discarded. Your original file is never edited or deleted. Local transcription keeps
audio on-device. Cloud models send the audio to your explicitly selected provider.

## Usage

**Activity** shows successful recordings, words, known audio duration and active
days, with trends for the selected period. **Models** breaks those totals down by
transcription model. Both are local views of retained History, not account-wide
cloud statistics. Deleting History removes those entries from the totals.

Reruns of the same recording count once, using its earliest retained successful
result. Words count the original transcription, before rewriting or manual edits.
Older recordings without duration are counted but do not add estimated audio time.
Daily chart buckets use UTC.

**Spend** contains local API cost estimates and filters, not provider invoices.
It is available in Community mode and hidden for signed-in Pro or Business
accounts. Activity and Models remain available to everyone.

## Recording controls

The microphone chooser follows the current system default unless you select a
specific input. Use **Refresh** after plugging in or changing a device, and
**Test** to check its live level; the test reports when it hears no signal or
cannot start. If a saved input disappears, Kolboo warns you and falls back to
the system default during capture.

Dictation uses your normal transcription and optional rewriting settings. Meeting
has its own model choice, supports pause/resume and never rewrites. Computer audio
is currently available on Linux only: it mixes the system-default microphone and
output, rather than Kolboo's selected microphone. Nothing is transcribed
until you press Stop. Audio is saved locally during capture so interrupted sessions
can be recovered from Recording options → Saved recordings.

Pressed Escape by accident? A Home recording stops and stays in Saved recordings.
The **Stop & save for later** icon does the same;
choose Transcribe when you're ready, or Discard to remove it. No transcription runs
just because you cancel. Ordinary F3 cancellation still discards the dictation and
does not create saved recovery audio.

Meeting models marked **Managed** use your Kolboo access; **Your key** uses the
provider key you configured. This choice is independent of Dictation. Recovery
retains the original recording's mode and model choice.

Click a History card to expand it. Kolboo prepares its waveform and audio without
starting playback. Playback controls become available as soon as the audio is
ready; first-time waveform analysis can finish afterward without blocking playback
or saving/deleting other recordings. Click or keyboard-seek on the waveform, then use the centered
playback controls when you are ready. Copy is a separate button. The overflow menu
contains rerun, request logs and deletion. Failed recordings show a centered
**Retry** button even when collapsed. Kolboo checks saved audio when
you retry and reports if it has been removed. A completed retry replaces the
failed item rather than adding a duplicate; successful recordings can still be
rerun as separate results. If you manually corrected a failed item, Kolboo keeps
your notes as a separate item instead of deleting them.

**Open full view** is available for every recording. It has a larger transcript,
search and an Edit button for changing the title and transcript. Corrections save automatically;
save status appears while editing or when attention is needed. Copy includes your current
corrections, even if saving failed. Restore original brings back the original
transcript. If a save fails, keep the app open and retry or copy your draft. Unsaved
changes cannot survive a process crash.

For simple meeting speaker labels, select **GPT-4o Transcribe · speaker labels** with
your existing OpenAI key, or when the model is available through managed access.
Long recordings are uploaded in parts after Stop. Speaker labels restart for each
part: two people labelled Speaker A in different parts may not be the same person.
Editing the transcript does not retime its original speaker metadata.

Long-recording preparation writes directly to disk, and uploads read one bounded
chunk at a time. Retrying saved audio reuses completed preparation when the source
and prepared recording are unchanged, as well as resuming saved upload results.
Interrupted preparation leaves the source intact; its incomplete temporary output
is cleaned up on the next app start. Cancelling and starting another recording
cannot let an old transcription reset the newer session.

Reopening a card shares any waveform analysis already in progress. Native analysis
jobs are queued, and completed waveforms are cached locally. The hidden Data settings
tab stops its periodic storage scans; History checks only request-log identifiers
for its log button instead of repeatedly loading full diagnostic payloads.

Saved audio, transcripts, corrections and waveform caches stay in the app's local
data directory until retained/deleted according to your settings. They are not
encrypted by Kolboo; use OS account protection and disk encryption. Transcription
sends audio only to the provider/path you have selected.

The live recording indicator uses centered, symmetric bars with visual gain
that adapts to quiet and loud microphones without altering the recorded audio.
It uses the level meter if waveform buckets are unavailable, and settles when
audio pauses. During transcription and rewriting, two subtle waves travel
inward from the edges and back out.
A delayed hide request cannot hide an active recording unless you explicitly
choose the Never overlay preference. Clipboard insertion verifies the new text
before sending the paste shortcut; if verification fails, copy the transcript
from History instead.
