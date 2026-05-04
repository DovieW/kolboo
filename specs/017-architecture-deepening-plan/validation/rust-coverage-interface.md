# Rust Coverage Helper Interface

US8 implements `app/scripts/rust-coverage.mjs`. This document defines the required behavior before the helper exists.

## Goals

- Run deterministic Rust coverage for changed/new in-scope modules.
- Use `cargo llvm-cov` when available.
- Fail clearly when coverage tooling is unavailable.
- Never require real network, API keys, audio devices, screenshots, sleeps, or user interaction.
- Work from `app/` through package scripts.

## Required package script

`app/package.json` should expose:

- `cargo:coverage`: runs `node scripts/rust-coverage.mjs`

## Required helper behavior

- Accept optional module/path filters for in-scope slices.
- Set or respect conservative `CARGO_BUILD_JOBS`.
- Respect `RUSTC_WRAPPER=sccache` when already configured.
- Avoid overwriting caller environment with secrets or printing sensitive values.
- Emit command summaries suitable for `coverage-evidence.md`.
- Exit non-zero if coverage cannot be measured for an in-scope Rust slice.

## Suggested command shape

```text
cargo llvm-cov --manifest-path src-tauri/Cargo.toml --summary-only
```

Exact arguments may evolve during US8 implementation, but the helper must keep validation deterministic and evidence-friendly.
