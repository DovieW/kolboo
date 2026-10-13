# Waveform rendering

## Recording overlay

`app/src/overlay/BackendAudioWave.tsx` renders 32 stationary SVG bars centered
vertically. It subscribes to native `overlay-audio-level` frames only while
recording and visible; the renderer never opens a second microphone. No audio
frames for 350 ms settle the display to silence and reset its visual scale.

`liveWaveform.ts` maps native min/max buckets to the bars. It uses typical peaks,
fast headroom and slow release to keep both quiet speech and louder microphones
readable without pinning every bar. Malformed/stale buckets fall back to the
reported RMS/peak level. This is display gain only, not audio normalization or
voice activity detection.

During transcription, routing and rewriting, CSS pulses travel inward from both
edges and back outward. They do not require audio, JavaScript frame loops or
microphone access. The window disables background throttling and does not take
focus from the target application. Enabling detailed loading in UI settings
replaces the waveform with phase text instead.

The small processing indicator intentionally continues under
`prefers-reduced-motion: reduce`, as requested by the maintainer on October 7.
Previously that rule hid all pulses and left a faint flat line. This exception
applies only to the loading wave; History reveal animations and other existing
reduced-motion behavior remain unchanged. Windows and macOS desktop motion
preferences both affect the CSS media query, so do not add platform-specific
suppression of the processing indicator.

## Saved recordings in History

`app/src/components/history/HistoryAudioPlayer.tsx` gives WaveSurfer the existing
audio element and precomputed recording peaks. `normalize: true` scales each
recording's drawing to the available 64px height instead of drawing quiet samples
at their tiny absolute amplitude. It does not modify the source peaks, saved
recording, playback volume, duration or seek coordinates. Relative amplitudes
and silence remain meaningful; an isolated large transient can still dominate
the scale.

The player shell appears immediately. Once the waveform has rendered, its bars
rise vertically from their center baseline; controls enable when media is ready.
With reduced motion the reveal is immediate, without the rise animation.

## Validation

Component tests cover native-frame gain, stalled/hidden capture cleanup,
processing transitions, mirrored pulse timing and actual CSS under both desktop
motion preferences. History tests assert display normalization with unchanged
peaks and media. Browser-engine smoke checks should sample actual pulse
transform/opacity changes and canvas pixels, including quiet audio and silence;
component tests alone do not establish native Windows/macOS overlay visibility.

On October 7 (rechecked October 8), headless Chromium and WebKit checks using
the production SVG/CSS confirmed changing pulse transforms and opacity with both `reduce` and
`no-preference`. The previous CSS reproduced animation suppression under
`reduce`. The real WaveSurfer renderer drew synthetic quiet peaks at 64px rather
than 1px with normalization enabled, left silence at 1px, and retained the same
peaks and audio-element volume. These are browser-engine results on Linux, not
physical Windows/WebView2 or macOS/WKWebView acceptance.
