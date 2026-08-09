# Contributing to Kolboo

Thanks for your interest in contributing!

## Where to start

- Check the existing issues and discussions (if enabled).
- If you’re not sure where to jump in, start with small docs fixes or UI tweaks.
- For bigger changes, please open an issue first so we can align on approach.

## Development setup

### Prerequisites

- **Node.js 24+** (Corepack available)
- **pnpm** (this repo pins a pnpm version in `app/package.json`)
- **Rust toolchain (stable)**
- **Tauri prerequisites** for your OS (see Tauri docs)

Optional (recommended):

- `sccache` for faster Rust builds (see `app/scripts/dev-sccache.mjs`)

### Install dependencies

The frontend lives under `app/`.

- Install:
  - `cd app`
  - `pnpm install --frozen-lockfile`

### Run in dev

From `app/`:

- Default:
  - `pnpm dev`

Kolboo also has Local Whisper support behind Cargo features:

- Local Whisper (CPU):
  - `pnpm dev:local-whisper`
- Local Whisper (CUDA):
  - `pnpm dev:local-whisper:cuda`

You can also pass features directly through Tauri:

- `pnpm dev -- --features local-whisper-cuda`

## Coding standards

### TypeScript / UI

- Formatting and linting is handled by **Biome** (`app/biome.json`).
- TypeScript is **strict** (`app/tsconfig.json`).
- Avoid drive-by refactors and unrelated formatting changes.

### Rust / Tauri backend

- Prefer explicit, readable state-machine transitions in the pipeline.
- Avoid logging secrets (API keys, Authorization headers, etc.).
- When changing anything that affects runtime behavior, be mindful of:
  - persisted settings (`settings.json` via the Tauri store)
  - syncing settings into the running pipeline
  - emitting settings-change events so overlay windows refresh

## Running checks locally

From `app/`:

- `pnpm typecheck`
- `pnpm lint:ci`
- `pnpm test`

### Running all tests (frontend + Rust)

If you just want “run everything that can fail in CI”, from the repo root:

- `pnpm -C app test:all`

What it does (in plain English):

- TypeScript checks your types (no output files)
- Vitest runs the UI/unit tests
- Cargo runs the Rust tests for the Tauri backend

If you only want Rust tests:

- `pnpm -C app cargo:test`

If you only want frontend tests:

- `pnpm -C app test`

For a fuller local sweep (slower):

- `pnpm check`

## Pre-commit tests (recommended)

This repo uses **Husky** to run tests automatically on every commit.

After you install dependencies once, Husky will set up the git hooks for you:

- `cd app`
- `pnpm install`

From then on, `git commit` will run:

- `pnpm -C app test:all`

If you ever need to temporarily skip hooks (use sparingly):

- `git commit --no-verify`

## Submitting a PR

- Keep PRs small and focused.
- Include context and screenshots for UI changes.
- Add or update docs if behavior changes.
- If your change affects settings shape, update both:
  - Rust default seeding/migrations
  - TS normalization/migration logic

## Reporting bugs

Please include:

- OS + version
- Kolboo version
- steps to reproduce
- logs (redacted—remove any secrets)

## License

By contributing, you agree that your contributions will be licensed under the project license (AGPL-3.0). You represent that you have the right to submit the contribution and that it does not knowingly include secrets, customer data, or code incompatible with that license.

You retain copyright in your contribution unless a separate written agreement says otherwise. Opening or merging a pull request does not silently assign copyright or grant Kol Software a separate commercial relicensing right. Preserve upstream and third-party attribution when modifying inherited work.
