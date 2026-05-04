# Kolboo Copilot instructions

- Keep instructions in sync:

  - If your change is significant enough (or it overlaps with guidance here or in `.github/instructions/**`), update the relevant instruction file(s) in the same PR.
  - Typical triggers: renamed/moved files, changed recommended commands, updated settings behavior, added/renamed Tauri commands/events, changed testing/CI expectations.

- Common commands:

  - `pnpm -C app dev` (Tauri dev)
  - `pnpm -C app build` (Tauri build)
  - `pnpm -C app check` (aggregates Biome/tsc/knip/vitest + Rust helpers)
  - `pnpm -C app check:ci` (CI gate; preferred before merging)

- Local Rust build cache (important for agent-run commands):

  - Before running any command that invokes Cargo/Rust locally (`pnpm -C app dev`, `build`, `cargo:*`, `test:all`, `check`, `check:ci`), set `RUSTC_WRAPPER=sccache` in the current shell when `sccache` is available.
  - PowerShell guard (preferred): if `sccache` exists, set wrapper; otherwise clear `RUSTC_WRAPPER` so Cargo falls back to plain `rustc`.
  - To avoid saturating the whole machine, also set a conservative `CARGO_BUILD_JOBS` for local runs (recommended: about half logical cores, capped around 8).
  - This improves incremental performance and reduces repeated recompilation cost across runs.

- Repo layout:

  - UI (React/Vite/TS) lives in `app/src/**`.
  - Tauri backend (Rust) lives in `app/src-tauri/src/**`.
  - The core recording pipeline/state machine is `app/src-tauri/src/pipeline.rs`; app bootstrap + command registration is `app/src-tauri/src/lib.rs`.

- UI↔backend contract:

  - UI calls Rust commands via `@tauri-apps/api/core` (`invoke`) and wrappers in `app/src/lib/tauri.ts`.
    - The wrappers are split into modules under `app/src/lib/tauri/**`:
      - `tauri/commands.ts` (invoke wrappers)
      - `tauri/settings.ts` (settings read/write + normalization/migrations)
      - `tauri/types.ts` (shared TS types)
  - UI state is mostly TanStack Query hooks in `app/src/lib/queries.ts`.
  - When changing any setting that affects runtime behavior, persist to the Tauri store **and** call `configAPI.syncPipelineConfig()` so the Rust `PipelineConfig` updates immediately.
  - After changing settings that overlays depend on (accent, widget position, overlay mode, etc.), emit `settings-changed` so secondary windows refresh cached settings (see `tauriAPI.update*` helpers).
  - Secondary windows should respond to `settings-changed` by reloading/invalidation only; do **not** call `sync_pipeline_config` from overlay listeners because the owning settings mutation path already applies the Runtime Sync Policy.

- Settings conventions (very important):

  - `settings.json` is the canonical persisted store for non-secret settings/cache (`@tauri-apps/plugin-store`).
  - API keys and session/refresh secrets belong in OS secure storage via `app/src-tauri/src/secrets.rs`; legacy `settings.json` API-key fallback exists only for migration/backward compatibility.
  - `null` often means “explicitly disabled”; missing/invalid values should fall back to defaults.
  - If you add/rename a setting, update BOTH:
    - Rust default seeding/migrations (see `ensure_default_settings(...)` in `app/src-tauri/src/lib.rs`)
    - TS normalization/migrations in the settings layer (`app/src/lib/tauri/settings.ts`) and any UI that reads it.

- Profiles/presets/router:

  - Per-program behavior is configured via `rewrite_program_prompt_profiles` (UI types in `app/src/lib/tauri/types.ts`, backend mapping in `app/src-tauri/src/pipeline.rs`).
  - Profiles can contain presets + an intent router (embeddings or LLM) that selects a preset; keep backward-compatibility for older `settings.json` shapes.

- Overlay windows:

  - Overlay is a separate Vite entry (`overlay.html`) with entrypoint `app/src/overlay-main.tsx` and UI in `app/src/OverlayApp.tsx`.
  - Overlay polls `pipeline_get_state` and listens to backend events (e.g., `overlay-hide-requested`, `overlay-audio-level`). Keep event names/payloads in sync with Rust emitters.
  - Overlay uses `PipelineState` strings; `arming` is UI-only (Rust won’t return it).

- Pipeline safety patterns:

  - The backend pipeline is a state machine; prefer explicit guard methods/transitions over ad-hoc flags.
  - Cancellation is part of the UX (escape-to-cancel is registered only while active). Avoid re-entrant shortcut registration; follow the existing lock/async pattern in `app/src-tauri/src/lib.rs`.

- Spec Kit/git workflow:

  - Do not auto-run branch-changing Spec Kit hooks/scripts unless the user explicitly accepts. The active `before_specify` hook is optional because `speckit.git.feature` creates/switches branches internally.

- Formatting/tooling:

  - TypeScript is strict (see `app/tsconfig.json`). Keep types accurate; prefer narrow unions for state/event strings.
  - Biome formats with **tabs** and double quotes (see `app/biome.json`). Avoid drive-by reformatting.

- Security/telemetry:
  - API keys are written through `tauriAPI.setApiKey(...)` to OS secure storage, with legacy `settings.json` fallback only when reading/migrating old installs. Never log secrets; redact request logs where needed.

<!-- SPECKIT START -->
For the active Spec Kit architecture-deepening initiative, read
`specs/017-architecture-deepening-plan/plan.md` for technologies, project
structure, validation commands, sequencing, and quality gates.

Implemented slice conventions from this initiative:

- OCR session ownership is centralized in `app/src-tauri/src/pipeline/ocr_session_state.rs`.
- Settings runtime side effects are classified by `app/src/lib/tauri/settingsSync.ts`.
- Overlay settings-change refresh behavior lives in `app/src/lib/overlay/overlaySettings.ts` and intentionally does not perform runtime pipeline sync.
- Routing strategy outputs flow through the strategy-independent `RoutingDecision` type in `app/src-tauri/src/pipeline/routing.rs`.
- Profile behavior is split between `app/src-tauri/src/pipeline/profile_matcher.rs` and `app/src-tauri/src/pipeline/profile_resolution.rs`.
- Local provider cache/readiness/bypass decisions live in `app/src-tauri/src/pipeline/local_provider_lifecycle.rs`.
- Do not add provider-family seams unless `specs/017-architecture-deepening-plan/validation/provider-family-decisions.md` records a two-adapter proof and deletion-test pass.
<!-- SPECKIT END -->
