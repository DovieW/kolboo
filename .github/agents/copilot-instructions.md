# kolboo Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-14

## Active Technologies
- TypeScript (strict) + Rust (Tauri) for desktop; TypeScript runtime contracts for cloud gateway/control-plane scaffolding + React/Vite, TanStack Query, `@tauri-apps/api`, Tauri Rust backend pipeline/state-machine, Cloudflare Worker patterns (JWT + rate limit + deterministic status) (013-managed-inference-proxy)
- Tauri store (`settings.json`) for local settings/cache, Supabase Postgres-backed entitlement/usage counters in cloud, request-log metadata surfaces (013-managed-inference-proxy)

- TypeScript (strict) + Rust (Tauri) + React/Vite, Mantine, TanStack Query, Tauri command/event system, store-backed settings layer (012-cloud-policy-packs)
- Tauri store (`settings.json`) for persisted policy cache and policy metadata (`policy_state` + effective policy payload) (012-cloud-policy-packs)
- TypeScript (strict) + Rust (Tauri) + React/Vite, Mantine, TanStack Query, `@tauri-apps/api`, Tauri Rust command/event system, Sentry SDKs used by desktop surfaces (011-login-org-enrollment)
- Tauri store (`settings.json`) for non-secret cached state + OS secure storage for auth/session tokens (011-login-org-enrollment)
- Rust (Tauri v2 backend) + TypeScript (strict) UI + Tauri, tauri-plugin-cli, React/Vite (010-backend-cli)
- Tauri store (`settings.json`) for persisted settings and profiles (010-backend-cli)
- TypeScript (strict) + Rust (Tauri) + React/Vite, Mantine UI, TanStack Query, Tauri + @tauri-apps/plugin-store (009-paste-smart-toggle)
- Tauri store (`settings.json`) (009-paste-smart-toggle)
- TypeScript (strict) + Rust (Tauri) + React/Vite, Tauri, TanStack Query (008-hotkey-shortcut-cards)
- Tauri store (`settings.json`) (008-hotkey-shortcut-cards)
- TypeScript (strict) + Rust (Tauri) + React/Vite, Tauri, TanStack Query (007-quick-ask-dismiss)
- Tauri store (`settings.json`) (007-quick-ask-dismiss)
- TypeScript (strict) + Rust (Tauri desktop app) + React/Vite (UI), Tauri (backend), `tauri-plugin-store` (settings), `tokio` (async), Windows APIs via `windows` crate (already used) (005-fix-overlay-visibility)
- Tauri store (`settings.json`) for overlay settings (mode, widget position, monitor target) (005-fix-overlay-visibility)
- TypeScript (React/Vite, strict) + Rust (Tauri v2) (004-active-window-ocr)
- TypeScript (strict) + React 19 + Vite 7 (UI); Rust 2021 + Tauri 2 (backend) + UI: `@tanstack/react-query`, Mantine; Backend: `windows` crate (Win32 bindings), `tokio`, `enigo`, `arboard`, `tauri-plugin-store` (003-uia-text-insertion)
- ... and 5 more

## Project Structure

```text
app/
├── src/
│   ├── components/settings/            # Policy visibility/enforcement UX
│   └── lib/
│       ├── tauri/settings.ts           # normalization + policy-aware persistence
│       ├── tauri/policy.ts             # policy command wrappers
│       ├── tauri/types.ts              # PolicyState and related types
│       └── queries.ts                  # policy fetch/apply/diagnostics queries
└── src-tauri/src/
  ├── commands/policy.rs              # Tauri policy sync/diagnostics commands
  ├── policy.rs                       # policy validation, application, cache logic
  └── lib.rs                          # command registration + startup wiring

specs/012-cloud-policy-packs/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
└── contracts/
```

## Commands

pnpm -C app lint
pnpm -C app test
pnpm -C app cargo:test
pnpm -C app check:ci

## Code Style

- TypeScript (strict) + Rust (Tauri): Follow existing project conventions
- Rust (Tauri v2 backend) + TypeScript (strict) UI: Follow existing project conventions
- TypeScript (strict) + Rust (Tauri desktop app): Follow existing project conventions
- TypeScript (React/Vite, strict) + Rust (Tauri v2): Follow existing project conventions
- TypeScript (strict) + React 19 + Vite 7 (UI); Rust 2021 + Tauri 2 (backend): Follow existing project conventions

## Recent Changes
- 013-managed-inference-proxy: Added TypeScript (strict) + Rust (Tauri) for desktop; TypeScript runtime contracts for cloud gateway/control-plane scaffolding + React/Vite, TanStack Query, `@tauri-apps/api`, Tauri Rust backend pipeline/state-machine, Cloudflare Worker patterns (JWT + rate limit + deterministic status)

- 012-cloud-policy-packs: Added TypeScript (strict) + Rust (Tauri) + React/Vite, Mantine, TanStack Query, Tauri command/event system, store-backed settings layer
- 011-login-org-enrollment: Added TypeScript (strict) + Rust (Tauri) + React/Vite, Mantine, TanStack Query, `@tauri-apps/api`, Tauri Rust command/event system, Sentry SDKs used by desktop surfaces

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->

