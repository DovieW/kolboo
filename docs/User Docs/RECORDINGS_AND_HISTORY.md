# Recordings and History

The small microphone button on Home starts a recording without a shortcut.
The options button lets you choose **Dictation** or **Meeting**; Kolboo remembers
the choice. Recordings started here stay in History instead of being pasted into
another app. F3 dictation keeps its normal insertion behavior.

## Transcribe an audio file

Open **Transcribe file** in the sidebar or from Home. Drop one audio file onto the
page or choose it with the file picker. Selection is local: audio is not uploaded
until you press **Transcribe**. WAV, MP3, FLAC and AAC (ADTS)
are supported, with mono/stereo audio up to four hours and a 2 GiB source-file
limit. M4A/MP4, AIFF, Ogg, other codecs, video files and multi-track selection are
not supported yet. Export these recordings as WAV, MP3 or FLAC before importing.

Choose Dictation to use your normal transcription and optional rewriting settings,
or Meeting for an independent model and no rewriting. Speaker labels require a
diarization-capable model. These file-specific choices do not overwrite the recorder's
saved preferences. Managed choices must already be enabled for your account.
Before you select a file or change its options, this page follows the recorder's
current mode and meeting model. Once selected, your file's options stay unchanged
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
audio on-device; optional cloud rewriting can still send text according to your settings.

## Recording controls

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

Click a History card to expand it. Play or seek through its waveform; opening a card
does not start playback. Copy is a separate button. The overflow menu contains rerun,
request logs and deletion. Rerun produces a new result without changing your notes.

**Open full view** is available for every recording. It has a larger transcript,
search, an editable title and an explicit Edit button. Corrections save automatically;
watch Saving/Saved or the error message. Restore original brings back the original
transcript. If a save fails, keep the app open and retry or copy your draft. Unsaved
changes cannot survive a process crash.

For simple meeting speaker labels, select **GPT-4o Transcribe · speaker labels** with
your existing OpenAI key, or when the model is available through managed access.
Long recordings are uploaded in parts after Stop. Speaker labels restart for each
part: two people labelled Speaker A in different parts may not be the same person.
Editing the transcript does not retime its original speaker metadata.

Saved audio, transcripts, corrections and waveform caches stay in the app's local
data directory until retained/deleted according to your settings. They are not
encrypted by Kolboo; use OS account protection and disk encryption. Transcription
sends audio only to the provider/path you have selected.
