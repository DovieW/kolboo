# Testing and validation

**Status:** Current

**Last reviewed:** 2026-09-30

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

On 2026-09-30 the maintainer approved the necessary native-boundary exceptions
for persistent desktop input. The reviewed snapshots cover Wry dispatch, real
clipboard/keyring/keyboard calls, four defensive Closed-signal subscription
failure lines, and duplicate LLVM instances with executed source definitions.
The portal's identity, keyboard-only request, restoration/token rotation,
denial, closure, delivery cleanup, and post-approval preparation are exercised
through the real ashpd client against an isolated `dbus-daemon`; no desktop
permission, network service or user clipboard is used. Pipeline output-lease
tests reject delayed output after a newer operation begins. Tests live outside
production modules so test-only forbidden-callback sentinels aren't waived.

The maintainer selected Linux-only native acceptance for the next release;
Windows/macOS native testing is deferred, not certified by Linux coverage.
On September 30 the refreshed package was installed and restarted on the
IdeaPad. Its running executable hash matched the extracted package; overlay
frontend readiness and rendered main-window controls were verified. Actual KDE
approval, repeated paste and grant restoration after app restart remain pending
user interaction; startup and portal-version checks do not establish those.
This exception approval does not assert that native acceptance has occurred
or authorize release publication.

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

On 2026-10-06 the maintainer approved the same narrowly checked counterless
metadata exception for the standalone annotations of
`commands/licensing/email_code.rs::license_request_email_code` and
`license_verify_email_code`. The isolated Wry account fixture executes both
generated IPC handlers, malformed-input rejection, and the command bodies with
synthetic credentials and loopback HTTP. Executable authentication behavior,
including cancellation and failed secure-storage writes, is not exempt.
The same fixture pauses real loopback entitlement/catalog responses to assert
that consumed-code tokens are already secure, interrupted commands recover via
startup's refresh path, and logout prevents late cache/model commits. These
checks use explicit request barriers rather than timing sleeps.

On 2026-10-01 the maintainer explicitly approved extending the existing native
recording/permission-callback wiring exception for the superseded-History and
request-scoped warning fixes. The helper behavior has deterministic regression
tests plus isolated real-Wry checks; no History transition or clipboard-decision
logic is exempted. The old autostart-registration exception was removed because
isolated production startup now covers it.

Rust coverage evidence is available through:

```sh
pnpm cargo:coverage
```

Coverage supports risk assessment; it does not replace platform and integration acceptance.

The wallet-read recovery tests exercise encrypted-session retry bounds and auth
read-failure preservation without a real wallet. The private native-window
process also runs the real secret-storage entry points with synthetic credentials
and an in-memory credential backend. This now covers the previously exempted
desktop-input token load/save wiring, so its `secrets.rs` coverage exception has
been removed. No new read-recovery or session-preservation code is exempted.

Linux Rust coverage also runs `pnpm cargo:test:window-native -- --coverage`.
This explicitly opted-in integration test uses real GTK/Tauri windows and
Openbox in Xvfb, a private D-Bus session and temporary application storage.
It verifies isolated clipboard selection after copy approval/denial, fractional-DPI sizing, resize/maximize persistence, actual tray-path
window recreation, storage failures and production startup/close. The production
startup process runs in a network namespace without external connectivity; no
developer environment files, account, recordings or API calls are used. Its
profiles are merged into the same Rust LCOV report, without new coverage waivers.
Normal `cargo:test` stays headless; Windows/macOS still require native acceptance.
The window-storage failure injection selects the exact registered window Store
by identity, never the first Store in unordered resource iteration. It also
asserts that the separate settings Store remains accessible. Native polling
timeouts name the operation so a geometry failure is diagnosable in CI logs.

On Ubuntu/Kubuntu install the optional acceptance prerequisites:

```sh
sudo apt-get install xvfb openbox x11-utils xauth
pnpm cargo:test:window-native
```

Coverage fails rather than silently skipping this boundary when its prerequisites
or unprivileged network namespaces are unavailable. Failed-run artifacts remain
in the printed temporary directory for diagnosis. The first instrumented desktop
binary compilation may take longer than the window assertions themselves.
CI uses a privileged network-namespace launcher that immediately drops back to
the runner UID/GID and preserves only the named display, temporary-storage and
LLVM-profile environment variables (including the runner's HOME). It does not
disable the host's namespace security policy. Full offline startup also needs
the tray runtime (`libayatana-appindicator3-dev` on Ubuntu). Failed startup logs
are printed from the synthetic fixture, never the user's desktop/data.
`KOLBOO_NATIVE_WINDOW_RUNNER` selects this trusted launcher, never an offline-test
bypass.

The merged Linux LCOV report uses binutils `c++filt --format=rust --no-verbose`
to identify the same Rust function across test/desktop build hashes. It retains
source files, definition locations, concrete generic types and closure indices;
unexecuted variants still fail the gate. This deduplicates build artifacts, not
source functions or missing lines. Regression tests enforce that distinction.

## Dependency security

```sh
pnpm run audit
```

Use `run audit` explicitly to invoke both JavaScript and Rust audits; bare
`pnpm audit` runs pnpm's JavaScript audit only.

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
