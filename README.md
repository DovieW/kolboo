<p align="center">
  <img src="app/src-tauri/icons/icon.png" alt="Kolboo" width="128" height="128">
</p>

# Kolboo

Accurate speech-to-text using advanced STT models like Whisper (local or via API).

This is a fork of [Tambourine](https://github.com/kstonekuan/tambourine-voice) mainly created as an alternative which does not require a standalone python server (but ended up with loads more features on top of that).

|||
|-|-|
|Home|Settings|
|<img width="2364" height="1410" alt="image" src="https://github.com/user-attachments/assets/d38fb833-243c-4014-9b42-404a30799f71" />|<img width="2364" height="1410" alt="image" src="https://github.com/user-attachments/assets/bf3c5d51-947a-483b-8f81-0a84b3e21786" />|
|Stats (WIP)|Logs|
|<img width="2364" height="1410" alt="image" src="https://github.com/user-attachments/assets/7402a31f-a580-4bff-8e75-7fc458b9cc2f" />|<img width="2364" height="1410" alt="image" src="https://github.com/user-attachments/assets/dec7fb49-c788-400f-ab5a-bb58ec455eb2" />|

## Features

- Dictate using state-of-the-art speech-to-text and language models.
- Hotkeys for: Toggle recording, Hold to record, Paste last transcription.
- Pass dictation to LLMs for further enhancement.
- Create per program workflows using the profile system.
- Clean overlay while transcribing ([image](https://github.com/user-attachments/assets/73cbdadd-a347-4bde-b473-8e974dac7eff)) with customizable sound cue.
- Comprehensive provider and model support ([full list](docs/User Docs/SUPPORTED_PROVIDERS_AND_MODELS.md)): Local, Google, OpenAI, Groq, Avalon and more.
- Logging and testing system to easily troubleshoot issues and work on prompts.
- Stats page to track cost usage and more (WIP).
- Audio refinement settings.
- Customize accent color.
- And more.

## Installation

Download Windows installer from the [latest release](https://github.com/DovieW/kolboo/releases).

| Platform | Compatibility    |
| -------- | -------------    |
| Windows  | ✅               |
| macOS    | ⚠️ (need tester) |
| Linux    | ⚠️ (need tester) |

## Development build variants

Kolboo ships with optional Local Whisper support behind Cargo features.

- **No Local Whisper** (default): runs with API-based STT providers only.
- **Local Whisper (CPU)**: enable `local-whisper`.
- **Local Whisper (CUDA)**: enable `local-whisper-cuda`.

### CUDA notes (Windows)

CUDA acceleration depends on *both*:

- An NVIDIA GPU + driver (`nvcuda.dll`)
- CUDA runtime libraries (`cudart` + `cublas` DLLs)

For broad compatibility, we recommend targeting **CUDA 12** for the runtime/toolkit.
Newer NVIDIA drivers are generally backward compatible with CUDA 12 runtime, while
building against CUDA 13 runtime can fail on machines whose drivers only report CUDA
12.x support.

Kolboo surfaces this in Settings → Local Whisper via “Compute” (what Kolboo will
request) and “Observed” (what `nvidia-smi` sees for the app PID).

For maintainers, see: `docs/How Tos/LOCAL_WHISPER_MAINTENANCE.md`.

From `app/`:

- `pnpm dev` (default)
- `pnpm dev:local-whisper`
- `pnpm dev:local-whisper:cuda`

You can also pass features directly through Tauri, e.g. `pnpm dev -- --features local-whisper-cuda`.

> CI note: we do **not** build CUDA artifacts in GitHub Actions. Build CUDA locally (on a machine
> with CUDA Toolkit installed) and upload the resulting artifacts to Releases.

## License

[AGPL-3.0](LICENSE)
