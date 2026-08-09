# Testing and validation

**Status:** Current

**Last reviewed:** 2026-08-09

This is the canonical desktop testing guide. The scripts in `app/package.json` and the workflows under `.github/workflows/` remain executable sources of truth if this guide drifts.

## Toolchain

- Node.js 24 or newer
- pnpm 10.26.2 through the `packageManager` field
- stable Rust with `rustfmt` and `clippy`
- platform-specific Tauri build prerequisites

Install JavaScript dependencies from `app/`:

```sh
pnpm install --frozen-lockfile
```

## Fast focused checks

Run the narrowest checks that cover a change:

```sh
pnpm lint:ci
pnpm typecheck
pnpm test
pnpm coverage
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

## Coverage policy

Coverage thresholds are enforced for declared frontend files and must not be lowered to make a change pass. Add focused tests for uncovered branches or explicitly exclude generated/entrypoint code when it has no meaningful executable behavior.

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
