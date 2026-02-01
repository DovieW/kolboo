# kolboo Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-01-25

## Active Technologies
- TypeScript (strict) + React 19 + Vite 7 (UI); Rust 2021 + Tauri 2 (backend) + UI: `@tanstack/react-query`, Mantine; Backend: `windows` crate (Win32 bindings), `tokio`, `enigo`, `arboard`, `tauri-plugin-store` (001-uia-text-insertion)
- Tauri store (`settings.json`) for settings and locally persisted “App Capability Memory”. (001-uia-text-insertion)
- TypeScript (strict) + Rust (Tauri desktop app) + React/Vite (UI), Tauri (backend), `tauri-plugin-store` (settings), `tokio` (async), Windows APIs via `windows` crate (already used) (001-fix-overlay-visibility)
- Tauri store (`settings.json`) for overlay settings (mode, widget position, monitor target) (001-fix-overlay-visibility)
- TypeScript (strict) + Rust (Tauri) + React/Vite, Tauri, TanStack Query (001-quick-ask-dismiss)



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
- 001-quick-ask-dismiss: Added TypeScript (strict) + Rust (Tauri) + React/Vite, Tauri, TanStack Query
- 001-fix-overlay-visibility: Added TypeScript (strict) + Rust (Tauri desktop app) + React/Vite (UI), Tauri (backend), `tauri-plugin-store` (settings), `tokio` (async), Windows APIs via `windows` crate (already used)
- 001-uia-text-insertion: Added TypeScript (strict) + React 19 + Vite 7 (UI); Rust 2021 + Tauri 2 (backend) + UI: `@tanstack/react-query`, Mantine; Backend: `windows` crate (Win32 bindings), `tokio`, `enigo`, `arboard`, `tauri-plugin-store`



<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
