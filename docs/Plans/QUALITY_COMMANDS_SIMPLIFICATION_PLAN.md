# Plan: simplify + standardize quality commands

## Why this plan exists

Kolboo currently has a lot of quality-related commands (linting, formatting, typechecking, tests, Rust clippy/fmt/test, CI variants, and feature-flag variants). This is normal for a mixed TS + Rust repo, but it creates cognitive overhead:

- People don’t know which commands are “the real ones”.
- Similar commands use inconsistent naming patterns.
- CI-vs-local variations add lots of script entries.

This plan proposes a way to **simplify what humans need to remember**, while keeping the existing low-level commands available for debugging.

## Goals

- Reduce the number of commands people need to know by heart.
- Standardize naming so it’s obvious what a command does.
- Preserve the existing behaviors (no breaking changes initially).
- Keep CI behavior explicit and reproducible.

## Non-goals

- Changing what CI actually checks.
- Removing functionality (e.g., dropping clippy or knip).
- Moving tools to different toolchains (e.g., replacing Biome/Vitest/Clippy).

## Current state (summary)

Today `app/package.json` contains:

- JS/TS: `lint`, `lint:ci`, `typecheck`, `knip`, `test`, `test:watch`, `coverage`
- Rust: `cargo:clippy`, `cargo:clippy:ci`, `cargo:fmt`, `cargo:fmt:check`, `cargo:test`, `cargo:test:ci`, etc.
- Meta: `check`, `check:ci`, `test:all`, plus `check:ci:local-whisper`

A key detail: CI Rust commands use `--target-dir src-tauri/target-ci`.

## Proposal A (recommended): keep scripts, add a small “blessed surface area”

Add a minimal set of "human-facing" aliases, and treat the rest as implementation details.

### New high-level commands (aliases)

- `pnpm -C app fix`
	- Meaning: “auto-fix formatting/lint where safe”.
	- Intended to run the *writing* commands (Biome check --write, cargo fmt).

- `pnpm -C app verify`
	- Meaning: “run the main no-edit quality gate locally”.
	- Intended to run the same steps as CI gate (or very close).

- `pnpm -C app verify:ci`
	- Meaning: “exact CI gate”.
	- This is essentially today’s `check:ci`.

- `pnpm -C app verify:features` (optional)
	- Meaning: “also validate feature builds (e.g., local-whisper clippy)”.

### Why this is the lowest-risk approach

- No one’s workflows break (existing commands stay).
- Docs can point to `fix` and `verify` so most contributors only learn two commands.
- CI stays explicit.

## Proposal B: standardize naming (more invasive, optional second phase)

After aliases exist and are documented, optionally rename scripts to a consistent taxonomy:

- `lint` / `lint:check`
- `format` / `format:check`
- `test` / `test:watch` / `test:coverage`
- `typecheck`
- `rust:lint` (clippy)
- `rust:format` / `rust:format:check`
- `rust:test`
- `verify` (aggregates)

This reduces confusion but requires a migration period and updates to docs/CI.

## Proposal C: reduce CI script duplication by using env (not recommended yet)

Instead of `cargo:*:ci` scripts that bake in `--target-dir`, CI could set:

- `CARGO_TARGET_DIR=src-tauri/target-ci`

and then reuse the plain scripts.

Pros: fewer scripts.
Cons: behavior becomes “hidden” in workflow environment rather than explicit in `package.json`. Harder for contributors to reproduce.

## Rollout steps (no behavior changes initially)

1. Add alias scripts:
	- `fix`
	- `verify`
	- `verify:ci`
	- optional: `verify:features`

2. Update documentation:
	- `docs/User Docs/QUALITY_COMMANDS.md` should highlight the aliases as “the main ones”.
	- Keep the long list for reference.

3. Optional: add a short section to `docs/Plans/TESTING_AND_QUALITY_PLAN.md` pointing to the aliases.

4. After a couple weeks (or once everyone’s used to it), consider Proposal B rename cleanup.

## Acceptance criteria

- A new contributor can be told: “Run `pnpm -C app fix` and `pnpm -C app verify`” and that’s enough for 95% of work.
- CI continues to run the same checks as before.
- No existing scripts are removed in phase 1.

## Risks / tradeoffs

- Too much renaming too fast can break muscle memory.
- Hiding CI-specific behavior via env vars can reduce reproducibility.
- If we keep both old and new names forever, the list can still look large; the real win is documenting the small set as the primary interface.
