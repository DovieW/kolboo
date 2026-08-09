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

Windows is the only supported v1 platform. The release workflow and manual acceptance process cover Windows installers only.

| Platform | v1 status |
| --- | --- |
| Windows | Supported |
| macOS | Unsupported; development experiments only |
| Linux | Unsupported; development experiments only |

There is no committed macOS or Linux release timeline. See the [Windows release operations guide](docs/Dev%20Docs/RELEASE_OPERATIONS.md) for signing, updater, rollback, and cache-cleaning procedures.

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

## Debug logging

Kolboo's Tauri backend emits structured JSON logs.

- Default log level is `info`.
- To enable verbose logs, set `RUST_LOG` before launching.

Examples (Windows PowerShell):

- Enable extra overlay window logs:
  - `$env:RUST_LOG = "info,kolboo_lib::commands::overlay=debug"`
- Enable debug logs for everything (very noisy):
  - `$env:RUST_LOG = "debug"`

Tip: in debug builds, the overlay UI can also emit dev-only debug notes into the Rust log stream via the `ui_debug_log` command.

## CLI log access

Kolboo's CLI now includes a `logs` command group so you can grab logs quickly for troubleshooting and bug reports.

- Request logs (in-memory, structured):
  - `kolboo logs request list -n 50 -o json`
  - `kolboo logs request export -f request-logs.json --strip_text -o json`
  - `kolboo logs request clear -o human`
- App logs (rolling file logs):
  - `kolboo logs app dir -o human`
  - `kolboo logs app list -n 20 -o json`
  - `kolboo logs app show -n 200 -o json`
  - `kolboo logs app show -f kolboo.2026-02-16.log -n 500 -o json`

Notes:

- Use `--strip_text` when sharing logs externally to reduce transcript/context leakage.
- `logs app show` defaults to the latest log file when `--file` is omitted.
- For script-friendly output, prefer `-o json`.

> CI note: we do **not** build CUDA artifacts in GitHub Actions. Build CUDA locally (on a machine
> with CUDA Toolkit installed) and upload the resulting artifacts to Releases.

## License

[AGPL-3.0](LICENSE)

Kolboo is derived from Tambourine Voice and includes work from multiple contributors. See [NOTICE](NOTICE), [third-party notices](THIRD_PARTY_NOTICES.md), and the [commercial-license/AGPL explanation](https://kol.software/legal/commercial-license) before redistributing or requesting commercial terms. The public legal URL is a launch gate and may not resolve while this repository remains private.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Security

Please report security issues privately. See [SECURITY.md](SECURITY.md).

## Privacy & data

This app can record audio and may send audio/transcripts to third-party providers depending on your settings.
See [docs/User Docs/PRIVACY_AND_DATA.md](docs/User%20Docs/PRIVACY_AND_DATA.md).
