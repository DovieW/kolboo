# kolboo Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-01-25

## Active Technologies
- TypeScript (strict) + React 19 + Vite 7 (UI); Rust 2021 + Tauri 2 (backend) + UI: `@tanstack/react-query`, Mantine; Backend: `windows` crate (Win32 bindings), `tokio`, `enigo`, `arboard`, `tauri-plugin-store` (001-uia-text-insertion)
- Tauri store (`settings.json`) for settings and locally persisted “App Capability Memory”. (001-uia-text-insertion)
- TypeScript (strict) + Rust (Tauri desktop app) + React/Vite (UI), Tauri (backend), `tauri-plugin-store` (settings), `tokio` (async), Windows APIs via `windows` crate (already used) (005-fix-overlay-visibility)
- Tauri store (`settings.json`) for overlay settings (mode, widget position, monitor target) (005-fix-overlay-visibility)
- TypeScript (strict) + Rust (Tauri) + React/Vite, Tauri, TanStack Query (007-quick-ask-dismiss)
- Rust (Tauri v2 backend) + TypeScript (strict) UI + Tauri, tauri-plugin-cli, React/Vite (010-backend-cli)
- Tauri store (`settings.json`) for persisted settings and profiles (010-backend-cli)
- TypeScript (strict) + Rust (Tauri) + React/Vite, TanStack Query, Mantine UI, Tauri, `@tauri-apps/plugin-store` (001-phase0-enterprise-posture)
- Tauri store (`settings.json`) + local policy state/cache metadata (001-phase0-enterprise-posture)



## Project Structure

```text
src/
tests/
```

## Commands

# Add commands for 

## Code Style

General: Follow standard conventions

## Recent Changes
- 001-phase0-enterprise-posture: Added TypeScript (strict) + Rust (Tauri) + React/Vite, TanStack Query, Mantine UI, Tauri, `@tauri-apps/plugin-store`
- 010-backend-cli: Added Rust (Tauri v2 backend) + TypeScript (strict) UI + Tauri, tauri-plugin-cli, React/Vite
- 008-hotkey-shortcut-cards: Added TypeScript (strict) + Rust (Tauri) + React/Vite, Tauri, TanStack Query



<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
