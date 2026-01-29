---
name: rust-best-practices
description: Describe what this skill does and when to use it. Include keywords that help agents identify relevant tasks.
---

---

name: rust-agent-development
description: Repo-compatible operating procedure + best practices for an AI coding agent working in existing Rust codebases.
version: 1.1.0
scope:
languages: [rust]
applies_to: [workspace, library, binary, proc-macro]
----------------------------------------------------

# Rust Agent Development Skill (Repo-Compatible)

## Primary objective

Make **correct, idiomatic, maintainable** Rust changes with **minimal diffs**, preserving existing behavior and public API unless explicitly asked to change them.

## Non-negotiables

* **No broken builds**: `cargo check` must pass for all relevant targets/features.
* **No new warnings**: treat warnings as errors under CI-equivalent settings.
* **Tests must pass**: run targeted tests; expand to workspace/matrix when changes are cross-cutting.
* **Avoid unnecessary dependencies**: prefer `std`; if adding a crate, justify it and keep features minimal.
* **No `unwrap()`/`expect()` in library code** (acceptable in tests/examples/one-off scripts if justified).
* **No new `unsafe`** unless explicitly requested or unavoidable; if used, document invariants and add tests.
* **Honor repo conventions**: do not fight `rustfmt.toml`, `clippy.toml`, existing `xtask`/`just`/`make` workflows.

---

## 0) Intake: determine project shape and repo conventions

Before editing, quickly infer:

1. **Crate type(s)**: binary vs library vs workspace.

   * Inspect root `Cargo.toml` and `[workspace]` members.
2. **MSRV / edition**:

   * Check `Cargo.toml` for `rust-version` and `edition`.
   * Check CI (GitHub Actions, Azure, etc.) for `rust-toolchain`, `rustup toolchain install`, or `dtolnay/rust-toolchain`.
3. **Features**:

   * List `[features]`, defaults, and any documented mutually exclusive flags.
4. **Lint/format policy**:

   * Look for `rustfmt.toml`, `clippy.toml`, crate attributes (`#![deny(warnings)]`, `#![forbid(unsafe_code)]`), and CI scripts.
5. **Task runner / CI entrypoints**:

   * Prefer repo-defined commands if present: `cargo xtask …`, `just …`, `make …`, `./ci.sh`, `./scripts/test.sh`.
6. **Error/logging & async stack**:

   * Error: `anyhow` vs custom; Logging: `tracing` vs `log`; Async runtime: `tokio`/`async-std`/none.
7. **Target platforms**:

   * Watch for WASM, embedded, `no_std`, Windows support, or cross-compile targets.

### Conservative defaults if unknown

If you cannot determine MSRV, platforms, or feature conventions quickly:

* Avoid very new Rust/library APIs.
* Do not add runtime coupling (e.g., don’t force `tokio` into a runtime-agnostic lib).
* Prefer minimal changes and use repo tools/scripts if they exist.

---

## 1) Edit loop (agent workflow)

Use this loop; do not batch large, unvalidated changes.

### Step A — small, localized edit

* Keep diffs minimal; avoid opportunistic refactors.
* If a refactor is necessary, keep behavior constant and add characterization tests first.

### Step B — fast validation (local)

Prefer repo-defined scripts first (e.g., `just test`, `cargo xtask ci`). If none exist, use:

1. `cargo fmt`
2. `cargo check`
3. `cargo clippy --all-targets -- -D warnings`
4. `cargo test`

### Step C — CI parity validation (repo-compatible)

Run the closest equivalent to CI.

* If CI uses a script/runner, use that.
* Otherwise choose a **feature/test matrix** strategy (below) and run the defined commands.

### Step D — diagnose & iterate

* Reproduce with the smallest failing command.
* Fix root cause; avoid `#[allow(...)]` unless justified and localized.
* If formatting produces large churn, confirm it’s required by repo `rustfmt.toml` and keep it.

---

## 2) Feature/Test Matrix Policy (repo-compatible)

Rust repos vary widely. Use this decision process.

### 2.1 Determine matrix mode

* **Mode A — Repo-specified**: If docs/CI define a matrix, follow it exactly.
* **Mode B — Safe default**: If repo has features but no matrix defined:

  * Run `default`.
  * Also run `--no-default-features` if the crate supports it (build/test succeeds).
  * Run `--all-features` **only** if features are composable (no mutually exclusive flags) or CI does it.
* **Mode C — Minimal-change**: For tiny, local changes, run only the smallest set needed to gain confidence; escalate if touching shared code.

### 2.2 Suggested commands per mode

**Default (most repos):**

* `cargo clippy --all-targets -- -D warnings`
* `cargo test`

**Workspace or cross-cutting changes:**

* `cargo clippy --workspace --all-targets -- -D warnings`
* `cargo test --workspace`

**Feature-aware (if safe/defined):**

* `cargo clippy --workspace --all-targets --all-features -- -D warnings`
* `cargo test --workspace --all-features`

**No-default-features (if supported):**

* `cargo check --no-default-features`
* `cargo test --no-default-features`

> If `--all-features` fails due to mutual exclusivity, do not hack around it—identify the intended feature sets and run those.

---

## 3) MSRV discipline

* **Never introduce Rust features/APIs newer than MSRV.**
* If MSRV is declared (`rust-version`), treat it as authoritative.
* If MSRV is only implied by CI toolchain, treat CI as authoritative.
* If MSRV is unknown, default to conservative constructs and avoid newly stabilized APIs.

Practical guidance:

* Prefer stable, older idioms.
* Avoid introducing brand-new std APIs without checking compatibility.
* Avoid adding dependencies that require a newer Rust unless necessary.

---

## 4) Style and API design rules

### Public API discipline

* Do not change **public** function signatures, structs, trait bounds, or module exports unless requested.
* For libraries:

  * Prefer `pub(crate)` for internal items.
  * Prefer `#[non_exhaustive]` for public enums/structs that may grow.
  * Prefer additive changes over breaking changes.

### Make invalid states unrepresentable

* Use enums/newtypes to encode invariants.
* Validate at boundaries (parsing, IO, FFI).

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserId(String);

impl UserId {
    pub fn parse(s: &str) -> Result<Self, UserIdError> {
        if s.is_empty() {
            return Err(UserIdError::Empty);
        }
        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(thiserror::Error, Debug)]
pub enum UserIdError {
    #[error("user id is empty")]
    Empty,
}
```

### Borrowing and allocation

* Prefer `&str`/`&[T]` inputs.
* Avoid gratuitous clones; if you clone, justify it.
* Prefer iterators over indexing loops.

---

## 5) Error handling policy

Choose strategy based on crate type and existing conventions.

### Binary/application crates

* `anyhow` is acceptable for top-level error plumbing.
* Add context on IO and external boundaries.

```rust
use anyhow::{Context, Result};

fn load_config(path: &std::path::Path) -> Result<String> {
    let s = std::fs::read_to_string(path)
        .with_context(|| format!("reading config file: {}", path.display()))?;
    Ok(s)
}
```

### Library crates

* Prefer a typed error (`thiserror`) and a `Result<T>` alias.

```rust
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

---

## 6) Logging / tracing policy

* Follow existing repo choice (`tracing` or `log`).
* Libraries should not initialize global subscribers/loggers.
* Prefer structured fields where available (especially with `tracing`).

---

## 7) Async policy (repo-compatible)

* If the repo already uses an async runtime, follow it.
* **Libraries** should avoid hard-coupling to a runtime unless the crate is explicitly runtime-specific.
* If you must support runtime integration, prefer feature flags and minimal surface area.

---

## 8) Testing rules

* Add tests for behavior changes and invariants.
* Unit tests for pure logic; integration tests for API surfaces.
* If you fix a bug, add a regression test.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_id_rejects_empty() {
        let err = UserId::parse("").unwrap_err();
        assert!(matches!(err, UserIdError::Empty));
    }
}
```

* If public API changes/additions occur, ensure rustdoc is updated and doc tests pass if applicable.

---

## 9) Platform and cfg discipline

* Do not introduce platform-specific APIs without `cfg` gating.
* Avoid `std::os::unix::*` unless explicitly Unix-only or properly gated.
* If WASM/no_std/embedded is present, avoid `std` assumptions and follow repo patterns.

---

## 10) Dependency policy

Before adding a dependency:

* Confirm `std` can’t reasonably solve it.
* Prefer widely used, maintained crates.
* Keep features minimal; disable default features when appropriate.
* Respect license/security tooling if present.

Optional (run only if repo already uses them):

* `cargo audit`
* `cargo deny`
* `cargo machete`

---

## 11) Common patterns (agent-safe, compilable)

### Builder with validation and defaults

```rust
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub endpoint: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Default)]
pub struct ClientConfigBuilder {
    endpoint: Option<String>,
    timeout_ms: Option<u64>,
}

impl ClientConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    pub fn timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    pub fn build(self) -> Result<ClientConfig, BuildError> {
        let endpoint = self.endpoint.ok_or(BuildError::MissingEndpoint)?;
        let timeout_ms = self.timeout_ms.unwrap_or(30_000);
        Ok(ClientConfig { endpoint, timeout_ms })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("missing endpoint")]
    MissingEndpoint,
}
```

### Collect iterator results with error propagation

```rust
pub fn parse_all<I>(items: I) -> Result<Vec<u32>, std::num::ParseIntError>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    items
        .into_iter()
        .map(|s| s.as_ref().parse::<u32>())
        .collect::<Result<Vec<_>, _>>()
}
```

---

## 12) Output expectations (what you should report)

When finished:

* Summarize what changed (files + intent).
* List commands you ran and outcomes (fmt/clippy/test, feature sets).
* Note any tradeoffs, limitations, or follow-ups.
* If you added deps/features, justify them explicitly.

---

## 13) Quick command reference

Prefer repo scripts/xtasks first. Otherwise:

* Format: `cargo fmt`
* Check: `cargo check`
* Lint (strict): `cargo clippy --all-targets -- -D warnings`
* Test: `cargo test`
* Workspace: `cargo test --workspace`
* Doc tests: `cargo test --doc`
