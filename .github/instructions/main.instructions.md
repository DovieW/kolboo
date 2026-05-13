---
applyTo: '**'
---

- We have refactor docs in the Refactor folder. Whenever you are working on a task, and there is something that can be done to improve something you are working with or notice a real pain point, but it is out of scopre or too large of a change, then add it to the appropriate doc file. If it's a quick change or simply something that would help you in the moment, then do it right then and there.

- If your change overlaps with anything described in `.github/copilot-instructions.md` or `.github/instructions/**`, update those instruction files in the same PR so they stay accurate.

- Do not cut corners when making changes. When adding new things or fixing things, try to do it in a robust way.
  - Don't create massive files that do many different things.

- Default to feature delivery once the existing seam ownership is healthy. Do not start speculative refactors unless the user asks for architecture work or the current task clearly fails the deletion test.

- When touching an area that already has a deep Module/Interface split, prefer extending the current ownership line instead of adding a new pass-through helper, generic manager, or frameworky wrapper.

- Only introduce a new seam when it creates real leverage/locality:
  - deleting it would clearly re-spread complexity across multiple callers, or
  - at least two concrete adapters/callers need the shared Interface.
  Otherwise keep behavior feature-local or provider-local.

- If you find a real refactor opportunity that is out of scope, record it in `docs/Refactors/*.md` with specific files and pain points instead of widening the current task.

- Always run the format commands before test/check commands.

- Use the smallest validating command set first, then escalate only if needed.
  - Docs-only or spec-only changes: no app test/check commands required.
  - TS/UI-only changes under `app/src/**`: run `pnpm -C app lint` then `pnpm -C app test`.
  - Rust-only changes under `app/src-tauri/**`: after setting `RUSTC_WRAPPER`/`CARGO_BUILD_JOBS` as above, run `pnpm -C app cargo:fmt` then `pnpm -C app cargo:test`.
  - If the task is specifically about Rust dead code or unused Cargo dependencies, also run `pnpm -C app cargo:deadcode`; run `pnpm -C app cargo:deaddeps` too when `cargo-machete` is installed locally (GitHub Rust CI installs it explicitly).
  - TS + Rust changes (or uncertain impact): after setting `RUSTC_WRAPPER`/`CARGO_BUILD_JOBS` as above, run `pnpm -C app test:all` after formatting.
  - Run `pnpm -C app check:ci` once at the end (or when explicitly requested), not repeatedly during iteration. Set `RUSTC_WRAPPER` and `CARGO_BUILD_JOBS` first because this command runs Cargo steps.
