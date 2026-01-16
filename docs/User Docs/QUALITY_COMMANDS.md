# Quality commands (linting, type-checking, tests)

This page explains **every** command in Kolboo that is meant for:

- linting (finding code problems)
- formatting (making code style consistent)
- type-checking (catching type mistakes)
- dead-code detection (finding unused files/exports/dependencies)
- tests (proving things still work)

All commands below are run from the repo root, and use the `app/` package:

- format: `pnpm -C app <script>`

> Tip: If you see a command ending with `:ci`, it’s designed to be safe for CI and **should not modify files**.

---

## JavaScript/TypeScript linting and static checks

### `pnpm -C app lint`
**Runs:** `biome check --write`

**What it does:**
- Checks formatting + lint rules.
- **Automatically fixes** what it can (it will rewrite files).

**What a failure usually means:**
- There are lint errors that can’t be auto-fixed, or invalid syntax.

**What it’s meant to accomplish:**
- “Make my code match project style and fix easy issues automatically.”

**When to use it:**
- Before committing, or anytime you want to clean things up locally.

---

### `pnpm -C app lint:ci`
**Runs:** `biome lint .`

**What it does:**
- Runs Biome lint checks across the project.
- **Does not** rewrite files.

**What a failure usually means:**
- A lint rule is being broken (unused imports/vars, suspicious code patterns, etc.).

**What it’s meant to accomplish:**
- CI gate: “You must fix these issues, but CI shouldn’t edit your repo.”

**When to use it:**
- In CI.
- Locally when you want a strict check that won’t touch files.

---

### `pnpm -C app typecheck`
**Runs:** `tsc --noEmit`

**What it does:**
- Runs the TypeScript compiler for type checking.
- `--noEmit` means it does **not** output build files.

**What a failure usually means:**
- Type errors: wrong props, wrong return types, missing null checks, etc.

**What it’s meant to accomplish:**
- Catch type mistakes before runtime.

**When to use it:**
- Before pushing changes.
- Always in CI.

---

### `pnpm -C app knip`
**Runs:** `knip`

**What it does:**
- Finds “dead stuff,” depending on config:
  - unused exports
  - unused files
  - unused dependencies

**What a failure usually means:**
- Something is declared but not used anywhere, or a dependency is installed but not actually used.

**What it’s meant to accomplish:**
- Keep the project from slowly accumulating unused code and dependencies.

**When to use it:**
- CI gate and occasional cleanup.

---

## JavaScript/TypeScript tests

### `pnpm -C app test`
**Runs:** `vitest run --reporter=dot`

**What it does:**
- Runs all Vitest tests once.

**What a failure usually means:**
- A test failed, or test setup is broken.

**What it’s meant to accomplish:**
- Fast correctness check.

**When to use it:**
- Regularly during development.
- CI gate.

---

### `pnpm -C app test:watch`
**Runs:** `vitest`

**What it does:**
- Runs Vitest in watch mode.
- Re-runs relevant tests when files change.

**What it’s meant to accomplish:**
- Tight feedback loop while coding.

**When to use it:**
- While actively developing.

---

### `pnpm -C app coverage`
**Runs:** `vitest run --coverage`

**What it does:**
- Runs tests with coverage collection.
- Produces reports (text + HTML) in `app/coverage/`.
- Applies any configured thresholds.

**Current known thresholds:**
- Config is in `app/vite.config.ts`.
- Example: `src/lib/tauri.ts` has minimum thresholds (statements/branches/functions/lines).

**What a failure usually means:**
- Tests failed, **or** coverage is below a required threshold.

**What it’s meant to accomplish:**
- Make sure important code stays tested.

**When to use it:**
- When changing files with thresholds.
- When you care about coverage numbers.

---

## Rust (Tauri backend) linting/formatting/tests

> Note: These are run through `pnpm` scripts, but they execute Cargo commands against `app/src-tauri/Cargo.toml`.

### `pnpm -C app cargo:clippy`
**Runs:** `cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml`

**What it does:**
- Runs Rust’s linter (Clippy) for all targets (including tests).

**What a failure usually means:**
- Clippy found something serious enough to fail the build, or the Rust code doesn’t compile.

**What it’s meant to accomplish:**
- Catch Rust “code smells” and common bugs.

**When to use it:**
- Local quality checks.

---

### `pnpm -C app cargo:clippy:ci`
**Runs:** `cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml --target-dir src-tauri/target-ci`

**What it does:**
- Same as `cargo:clippy`, but writes build outputs to `src-tauri/target-ci`.

**What it’s meant to accomplish:**
- CI-style behavior and caching isolation.

**When to use it:**
- CI gate.

---

### `pnpm -C app cargo:clippy:local-whisper`
**Runs:** `cargo clippy ... --features local-whisper`

**What it does:**
- Runs Clippy with the `local-whisper` feature enabled.

**What it’s meant to accomplish:**
- Make sure that optional feature doesn’t rot.

---

### `pnpm -C app cargo:clippy:local-whisper:ci`
**Runs:** `cargo clippy ... --features local-whisper --target-dir src-tauri/target-ci`

**What it’s meant to accomplish:**
- CI-style clippy check for the `local-whisper` feature.

---

### `pnpm -C app cargo:clippy:all-features`
**Runs:** `cargo clippy --all-targets --all-features --manifest-path src-tauri/Cargo.toml`

**What it does:**
- Runs Clippy with *all* features enabled.

**What it’s meant to accomplish:**
- Catch feature-flag conflicts and bitrot.

**When to use it:**
- Less frequently (it’s often slower), or before releases.

---

### `pnpm -C app cargo:fmt`
**Runs:** `cargo fmt --manifest-path src-tauri/Cargo.toml`

**What it does:**
- Formats Rust code (rewrites files).

**What it’s meant to accomplish:**
- Consistent Rust formatting.

---

### `pnpm -C app cargo:fmt:check`
**Runs:** `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`

**What it does:**
- Checks Rust formatting without rewriting files.

**What it’s meant to accomplish:**
- CI formatting gate.

---

### `pnpm -C app cargo:test`
**Runs:** `cargo test --quiet --manifest-path src-tauri/Cargo.toml`

**What it does:**
- Runs Rust tests for the backend.

**What it’s meant to accomplish:**
- Backend correctness.

---

### `pnpm -C app cargo:test:ci`
**Runs:** `cargo test --quiet --manifest-path src-tauri/Cargo.toml --target-dir src-tauri/target-ci`

**What it’s meant to accomplish:**
- CI-style backend tests with isolated build outputs.

---

## Combined “do a bunch of things” commands

### `pnpm -C app cargo`
**Runs:**
- `pnpm -C app cargo:clippy`
- `pnpm -C app cargo:fmt`
- `pnpm -C app cargo:test`

**Meant to accomplish:**
- “Run all Rust quality checks locally.”

---

### `pnpm -C app test:all`
**Runs:**
- `pnpm -C app typecheck`
- `pnpm -C app test`
- `pnpm -C app cargo:test`

**Meant to accomplish:**
- Quick-ish multi-language confidence check.

---

### `pnpm -C app check`
**Runs:**
- `pnpm -C app lint` (edits files)
- `pnpm -C app typecheck`
- `pnpm -C app knip`
- `pnpm -C app test`
- `pnpm -C app cargo` (includes Rust fmt, edits files)

**Meant to accomplish:**
- Local “make everything clean” command.

---

### `pnpm -C app check:ci`
**Runs:**
- `pnpm -C app lint:ci`
- `pnpm -C app typecheck`
- `pnpm -C app knip`
- `pnpm -C app test`
- `pnpm -C app cargo:clippy:ci`
- `pnpm -C app cargo:fmt:check`
- `pnpm -C app cargo:test:ci`

**Meant to accomplish:**
- The CI gate: “Everything must pass, and nothing should modify files.”

---

### `pnpm -C app check:ci:local-whisper`
**Runs:**
- `pnpm -C app check:ci`
- then `pnpm -C app cargo:clippy:local-whisper:ci`

**Meant to accomplish:**
- CI gate plus validating the optional `local-whisper` feature.

---

## Recommended workflows (what to run when)

### Fast local loop (most of the time)
- `pnpm -C app test`
- `pnpm -C app typecheck` (if you touched types/components)

### Before opening a PR
- `pnpm -C app check:ci`

### When you want auto-fixes
- `pnpm -C app lint`

### If CI is slow
- It’s usually Rust compilation inside:
  - `cargo:clippy:ci`
  - `cargo:test:ci`

That’s why CI uses caching and `target-ci`.
