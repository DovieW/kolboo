# Testing and validation

**Status:** Current

**Last reviewed:** 2026-08-09

This is the canonical desktop testing guide. The scripts in `app/package.json` and the workflows under `.github/workflows/` remain executable sources of truth if this guide drifts.

## Development cadence

Kolboo uses a fast, risk-based validation loop. During ordinary feature and bug-fix work, run the narrowest tests plus lint, typechecking, or generated-contract checks for the subsystem changed. Aim to keep this local loop near five minutes, then commit and continue while longer CI jobs run asynchronously.

Use the full local gate at a coherent milestone, before handing work to real users, or when a shared architecture, release, migration, security, billing, quota, or generated-contract boundary changes. Do not expand a focused change to repair unrelated warnings or failures; record them and keep the current scope clear.

Tests should protect important behavior, risky boundaries, or known regressions. Prefer unit, integration, contract, and component tests over end-to-end automation. Use a short manual smoke check for hardware, permissions, packaging, or platform behavior that cheaper automation cannot represent reliably.

## Toolchain

- Node.js 24 or newer
- pnpm 10.26.2 through the `packageManager` field
- stable Rust with `rustfmt` and `clippy`
- `cargo-llvm-cov` 0.9.1 and the `llvm-tools-preview` Rust component
- `sccache` on every development platform
- `mold` on Linux
- platform-specific Tauri build prerequisites

Install JavaScript dependencies from `app/`:

```sh
pnpm install --frozen-lockfile
pnpm setup:check
```

Rust/Tauri commands launched through the package scripts use `sccache` with
`CARGO_INCREMENTAL=0` automatically. Linux builds also use `mold`; the setup
check fails with the platform-specific installation command when either required
accelerator is missing. Keep direct Cargo invocations for troubleshooting only so
normal development does not silently bypass the shared build environment.

## Fast focused checks

Run the narrowest checks that cover a change:

```sh
pnpm lint:ci
pnpm typecheck
pnpm test
pnpm coverage
pnpm coverage:patch
pnpm knip
```

Rust-focused commands:

```sh
pnpm cargo:fmt:check
pnpm cargo:deadcode
pnpm cargo:clippy
pnpm cargo:test
pnpm cargo:deaddeps
```

The strict dead-code denial currently runs on Windows, where the Windows UIA and modifier-only shortcut production paths are reachable. On Linux and macOS this command reports an explicit skip; those targets still run ordinary Clippy and tests. Do not interpret a non-Windows skip as proof that target-specific dead code is absent.

Generated-contract checks:

```sh
pnpm schemas:check
pnpm tauri-events:check
pnpm types:check
```

## Full local gate

From `app/`:

```sh
pnpm check:ci
```

This runs non-mutating lint, TypeScript, Knip, generated schema/event/type checks, frontend tests, Rust dead-code/clippy/format checks, and Rust tests. It can be expensive because the Tauri dependency graph is large.

This is a checkpoint command, not a requirement after every small change.

Use this additional gate when changing local Whisper behavior:

```sh
pnpm check:ci:local-whisper
```

## CI gates

The `Check` workflow runs independent jobs for:

- JavaScript and Rust dependency audits;
- frontend lint, typecheck, Knip, and unit tests;
- declared frontend coverage thresholds;
- generated contracts;
- Rust formatting, dead dependencies, dead code, clippy, and tests.

The Windows workflow separately validates Windows behavior and builds development artifacts. Linux and macOS jobs should be added as platform support work proceeds; a cross-compiled or conditionally compiled path is not equivalent to a native platform acceptance run.

## Test boundaries

Automated tests should be deterministic by default:

- no real provider keys;
- no live network dependency;
- no real microphone or desktop permission requirement;
- temporary directories for storage tests;
- mocked HTTP providers for request/response contracts;
- generated contracts for Rust/TypeScript command, event, and payload alignment.

Manual tests remain necessary for audio hardware, global shortcuts, text insertion, permissions, overlays, packaging, secure storage, updates, and full managed-user rehearsals.

## End-to-end browser policy

Do not add Playwright coverage for every screen, visual variation, or setting. Add an automated browser journey only when it protects a critical real-user outcome, crosses boundaries cheaper tests cannot cover, has deterministic fixtures, and will produce actionable failures.

Keep the golden-path suite small:

- account sign-in and onboarding;
- a Personal user initiating a managed request;
- an authorized operator locating and supporting that request;
- release or updater behavior once public distribution becomes active.

Cover permutations beneath those journeys with API integration and rendered component tests. During private-cohort development, a documented manual rehearsal is acceptable when browser automation would be disproportionately expensive or flaky.

## Coverage policy

Coverage thresholds are enforced for declared frontend files and must not be lowered to make a change pass. In addition, pull requests require 100% coverage of changed executable lines and functions. Frontend branch records on changed lines must also be fully covered. Rust branch coverage remains excluded because LLVM's support is unstable.

The patch gate intentionally applies to new behavior rather than pretending the
legacy repository already has 100% global coverage. It fails closed when a
changed production source file is missing from coverage reports. Tests,
declarations, generated contracts, and trivial renderer entrypoints are excluded
explicitly; do not expand that list merely to make a patch pass. Coverage proves
execution, not correctness, so changed behavior still needs meaningful assertions
and the appropriate unit, component, integration, or native-platform test.

When the remote merge base predates `.coverage-patch-v1`, enforcement starts at
the first policy-introduction commit on the branch, not at HEAD and not only
after merging to master. This grandfathers pre-policy code, **not subsequent
committed or working-tree changes**. Untracked production files are included.
Use `coverage:patch` for a fresh sequential frontend/Rust run: V8 clears its
report directory, so running the two writers concurrently can delete Rust LCOV.

On 2026-09-24 the maintainer authorized necessary exceptions for the accumulated
desktop patch. `app/scripts/patch-coverage-exceptions.json` records exact Rust
line/function locations, whole-source SHA-256 fingerprints, rationale and
related test evidence. There are no frontend or branch waivers and no blanket
file/directory exclusions. A source edit that still needs an exception fails
until it is tested or explicitly re-reviewed; do not regenerate these snapshots
from a failing report. Remove entries as native integration coverage becomes
available, and re-review them before a public release or toolchain upgrade.

These exceptions cover concrete Wry/OS clipboard/window-manager integration,
late cancellation/poisoned-lock defensive paths, target-gated declarations,
and selected LLVM generic-instantiation duplicates. A compiler-instance waiver
requires both executed source lines and an executed Rust function record at
the same definition. It cannot excuse an unexecuted function body. Native
acceptance (real recording, paste/focus, overlay visibility and each supported
desktop's program picker) remains required; related unit tests do not prove
that integration. Tests have been moved out of inline production modules where
coverage was incorrectly counting test-only panic sentinels as production gaps.

The 2026-09-24 audit passed with 1,646 covered changed executable lines, 405
explicit native/race line exceptions across 25 source snapshots, plus the
Tauri annotation exception below. It ran 851 frontend and 924 Rust tests;
57 frontend tests and 13 native/optional Rust tests remain skipped/ignored.
This is **100% of non-exempt patch code**, not 100% global or native-platform
coverage. Exceptions are printed separately in every gate result.

An additional explicitly approved exception (2026-09-23) covers counterless Tauri async
wrapper metadata at the standalone `#[tauri::command]` annotation of
`commands/history.rs::get_history_activity` (observed with rustc 1.98.1 and
Tauri 2.11.5). The gate names this exception in its output. It requires the exact
source declaration, zero-hit Rust closure records on the annotation, no branch
records there, and a covered command function on the following line. Unknown
record shapes fail closed. This annotation exception never waives command
bodies or their closures. IPC tests cover
successful serialization, invalid arguments, storage errors, and content-safe
responses. Recheck and remove this exception when upgrading Rust or Tauri;
it is not permission to exempt other generated wrappers.

Rust coverage evidence is available through:

```sh
pnpm cargo:coverage
```

Coverage supports risk assessment; it does not replace platform and integration acceptance.

## Dependency security

```sh
pnpm audit
```

High or critical advisories must be fixed. If no upstream fix exists and the dependency is still necessary, document a time-limited exception with exploitability, mitigation, owner, and expiry.

## Cache cleanup

Inspect Rust cache impact without deleting anything:

```sh
pnpm clean:rust-cache
```

Apply the exact reported cleanup only when the output has been reviewed:

```sh
pnpm clean:rust-cache:apply
```

The target directories can consume tens of gigabytes and are reproducible build output, but deleting them makes the next Rust build substantially slower.

## Adding tests

Prioritize tests for:

1. crashes, data loss, auth, settings migration, and state-machine safety;
2. provider/model request contracts, timeouts, cancellation, and errors;
3. Community/BYOK continuity and managed-feature gating;
4. platform capability/fallback behavior;
5. user-visible state and mutation side effects;
6. regressions found through Sentry or real users.

Keep test ownership near the production module. Create a new planning document only when a bounded implementation slice genuinely needs design work.
