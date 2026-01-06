# Kolboo Copilot instructions

- Dovie’s rule: **don’t run builds, lint, tests, or cargo checks unless explicitly asked**.

- Common commands (don’t run unless asked):

  - `pnpm -C app dev` (Tauri dev)
  - `pnpm -C app build` (Tauri build)
  - `pnpm -C app check` (aggregates Biome/tsc/knip/vitest + Rust helpers)

- Repo layout:

  - UI (React/Vite/TS) lives in `app/src/**`.
  - Tauri backend (Rust) lives in `app/src-tauri/src/**`.
  - The core recording pipeline/state machine is `app/src-tauri/src/pipeline.rs`; app bootstrap + command registration is `app/src-tauri/src/lib.rs`.

- UI↔backend contract:

  - UI calls Rust commands via `@tauri-apps/api/core` (`invoke`) and wrappers in `app/src/lib/tauri.ts`.
  - UI state is mostly TanStack Query hooks in `app/src/lib/queries.ts`.
  - When changing any setting that affects runtime behavior, persist to the Tauri store **and** call `configAPI.syncPipelineConfig()` so the Rust `PipelineConfig` updates immediately.
  - After changing settings that overlays depend on (accent, widget position, overlay mode, etc.), emit `settings-changed` so secondary windows refresh cached settings (see `tauriAPI.update*` helpers).

- Settings conventions (very important):

  - `settings.json` is the canonical persisted store (`@tauri-apps/plugin-store`).
  - `null` often means “explicitly disabled”; missing/invalid values should fall back to defaults.
  - If you add/rename a setting, update BOTH:
    - Rust default seeding/migrations (see `ensure_default_settings(...)` in `app/src-tauri/src/lib.rs`)
    - TS normalization in `tauriAPI.getSettings()` (`app/src/lib/tauri.ts`) and any UI that reads it.

- Profiles/presets/router:

  - Per-program behavior is configured via `rewrite_program_prompt_profiles` (UI types in `app/src/lib/tauri.ts`, backend mapping in `app/src-tauri/src/pipeline.rs`).
  - Profiles can contain presets + an intent router (embeddings or LLM) that selects a preset; keep backward-compatibility for older `settings.json` shapes.

- Overlay windows:

  - Overlay is a separate Vite entry (`overlay.html`) with entrypoint `app/src/overlay-main.tsx` and UI in `app/src/OverlayApp.tsx`.
  - Overlay polls `pipeline_get_state` and listens to backend events (e.g., `overlay-hide-requested`, `overlay-audio-level`). Keep event names/payloads in sync with Rust emitters.
  - Overlay uses `PipelineState` strings; `arming` is UI-only (Rust won’t return it).

- Pipeline safety patterns:

  - The backend pipeline is a state machine; prefer explicit guard methods/transitions over ad-hoc flags.
  - Cancellation is part of the UX (escape-to-cancel is registered only while active). Avoid re-entrant shortcut registration; follow the existing lock/async pattern in `app/src-tauri/src/lib.rs`.

- Formatting/tooling:

  - TypeScript is strict (see `app/tsconfig.json`). Keep types accurate; prefer narrow unions for state/event strings.
  - Biome formats with **tabs** and double quotes (see `app/biome.json`). Avoid drive-by reformatting.

- Security/telemetry:
  - API keys are stored via the store (UI uses `tauriAPI.setApiKey(...)`). Never log secrets; redact request logs where needed.
