# Synthetic audio import fixtures

These files contain only a generated 440 Hz tone (0.12 seconds at 16 kHz), with
no recorded voices or user data. They exercise container/decoder support without
network access, audio devices, or an FFmpeg dependency when running tests.

Generated with the already installed FFmpeg:

```sh
ffmpeg -f lavfi -i sine=frequency=440:sample_rate=16000:duration=0.12 \
  -map_metadata -1 -ac 1 -c:a CODEC OUTPUT
```

| File | Codec |
| --- | --- |
| tone.flac | flac |
| tone-aac.m4a | aac |
| tone-alac.m4a | alac |
| tone.aiff | pcm_s16be |
| tone.aac | aac (ADTS container) |
| tone.ogg | vorbis (use `-ac 2 -strict experimental`) |

The lossy fixtures can include codec padding beyond the original tone duration.

AIFF is an unsupported-format regression fixture, not advertised import support.
Symphonia 0.5.5 includes the SSND chunk's eight-byte offset/block header in its
audio length, so a valid AIFF can fail at the final packet. Imports reject AIFF
before staging instead of relaxing completeness checks or truncating audio.

Ogg Vorbis is also an unsupported-format regression fixture. The current Vorbis
decoder builds codebook tables from unbounded entry/dimension fields before any
output-buffer limit can be applied. Its decoder/container features remain disabled
until those allocations can be bounded safely.

Both M4A fixtures are unsupported-format regressions. The current MP4 reader
reserves sample/chunk tables directly from untrusted atom counts before checking
payload lengths. MP4/ALAC features remain disabled; tests verify these formats
cannot enter their readers even when renamed to a supported extension.
