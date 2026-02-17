---
applyTo: '**'
---

- Add tests when appropriate (but don't create unnecessary tests if there's such a thing).

- We have refactor docs in the Refactor folder. Whenever you are working on a task, and there is something that can be done to improve something you are working with or notice a real pain point, but it is out of scopre or too large of a change, then add it to the appropriate doc file. If it's a quick change or simply something that would help you in the moment, then do it right then and there.

- I have `pnpm dev` running an external terminal so I can see build errors already. Use VS Code Problems to make sure you didn't introduce real issues and fix all errors and warnings while you work.

- If your change overlaps with anything described in `.github/copilot-instructions.md` or `.github/instructions/**`, update those instruction files in the same PR so they stay accurate.

- Do not cut corners when making changes. When adding new things or fixing things, try to do it in a robust way.
  - Don't create massive files that do many different things.

- Always run the format commands before test/check commands.

- For local commands that invoke Rust/Cargo, set `RUSTC_WRAPPER=sccache` in the current shell first (when `sccache` is installed). If `sccache` is unavailable, clear `RUSTC_WRAPPER` and continue with plain `rustc`.
- To keep local runs responsive, also set `CARGO_BUILD_JOBS` to a conservative value (recommended: ~half logical cores, capped at 8) before Rust/Cargo commands.

- Use the smallest validating command set first, then escalate only if needed.
  - Docs-only or spec-only changes: no app test/check commands required.
  - TS/UI-only changes under `app/src/**`: run `pnpm -C app lint` then `pnpm -C app test`.
  - Rust-only changes under `app/src-tauri/**`: after setting `RUSTC_WRAPPER`/`CARGO_BUILD_JOBS` as above, run `pnpm -C app cargo:fmt` then `pnpm -C app cargo:test`.
  - TS + Rust changes (or uncertain impact): after setting `RUSTC_WRAPPER`/`CARGO_BUILD_JOBS` as above, run `pnpm -C app test:all` after formatting.
  - Run `pnpm -C app check:ci` once at the end (or when explicitly requested), not repeatedly during iteration. Set `RUSTC_WRAPPER` and `CARGO_BUILD_JOBS` first because this command runs Cargo steps.

- Avoid duplicate heavy runs during iteration.
  - If `pnpm dev` already surfaces live frontend issues, rely on VS Code Problems + targeted tests while iterating.
  - Prefer one final full validation pass right before handoff.
