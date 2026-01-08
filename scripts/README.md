# Scripts

Small helper scripts for building Kolboo locally.

## `build-windows-artifacts.ps1`

Builds Kolboo on **Windows** and collects outputs in the same artifact folder structure used by the (Windows) CI workflow.

### What it builds

By default, it produces two builds:

- **default** (no Local Whisper): `pnpm build`
- **local-whisper** (CPU Local Whisper): `pnpm tauri build -- --features local-whisper`

Optionally, it can also build:

- **local-whisper-cuda** (CUDA Local Whisper): `pnpm tauri build -- --features local-whisper-cuda`

### Output folders

Artifacts are collected under `artifacts/` at the repo root:

- `artifacts/windows-default/`
- `artifacts/windows-local-whisper/`
- `artifacts/windows-local-whisper-cuda/` (only when `-Cuda` is used)

Each folder will try to contain:

- a `bundle/` directory copied from `app/src-tauri/target/release/bundle/`
- any `*.exe` found in `app/src-tauri/target/release/`

### Prerequisites

- Windows
- **Node.js** (Corepack available)
- **Rust** (`cargo` in PATH)
- `pnpm` (the script will activate the repo-pinned pnpm version using Corepack)

Optional (recommended):

- `sccache` (the script will use it if present)

For CUDA builds (`-Cuda`):

- NVIDIA CUDA Toolkit installed (your machine can have multiple versions installed; this script uses whatever your environment resolves)

### Usage

From the repo root:

- Build default + local-whisper (CPU):

  - `./scripts/build-windows-artifacts.ps1`

- Also build local-whisper-cuda:

  - `./scripts/build-windows-artifacts.ps1 -Cuda`

- Clean `artifacts/` and `app/src-tauri/target/` first:

  - `./scripts/build-windows-artifacts.ps1 -Clean -Cuda`

- Skip dependency installation (if you already ran `pnpm install` in `app/`):
  - `./scripts/build-windows-artifacts.ps1 -SkipInstall -Cuda`

### Parameters

- `-Cuda`: also build the `local-whisper-cuda` variant
- `-SkipInstall`: skip `pnpm install --frozen-lockfile`
- `-Clean`: delete the output artifacts folder and `app/src-tauri/target/` before building
- `-ArtifactsDir <path>`: override the artifacts output directory (default: `artifacts`)

### Notes

- This script is intended for producing Release artifacts you can upload to GitHub Releases.
- CUDA runtime compatibility depends on what CUDA toolkit/runtime you build against and what your users have installed. The CPU build remains the “works everywhere” fallback.
