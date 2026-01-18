# Prevent Rust/TS contract drift (Plan)

This plan is for Dovie (and future-us) to keep the TypeScript frontend and Rust backend in sync so we don’t ship builds where:

- Rust adds/renames fields, but the UI types lag behind
- Rust renames an event string, but the UI still listens to the old name
- Schemas in `app/src-tauri/gen/schemas/**` silently go stale

The goal is to make drift **fail fast in CI** with deterministic tests.

## Goal (plain English)

Make it hard to accidentally change Rust-side settings/events/command shapes without also updating the TS contract.

In practice: if Rust changes something “contract-y”, CI should go red with a clear message that tells Dovie exactly what to update.

## What exists today (baseline)

Already implemented (from current working changes):

- Shared TS types have been centralized in `app/src/lib/tauri.ts` (instead of scattered per-component).
- Multiple components now import event payload types from `app/src/lib/tauri.ts`.
- A contract test exists at `app/src/lib/settingsContract.test.ts` which checks:
	- **Settings key parity**: Rust-seeded default keys (parsed from `set_default("key", ...)` in `app/src-tauri/src/lib.rs`) vs keys returned by `tauriAPI.getSettings()`.
	- **Schema parity**: a set of TS types are checked against JSON schemas in `app/src-tauri/gen/schemas/**`.
	- **Enum value parity**: TS unions contain values present in schema enums.
	- **Null-payload event schemas** are enforced for “empty” events.

This is a great start, but it assumes the JSON schemas are up-to-date.

## Coverage inventory (what’s enforced vs what’s missing)

This section answers the practical question: “Which parts of the codebase are we *already* protecting, and which parts still need to be added?”

### Already enforced (today)

**A) Settings key drift**

- ✅ `app/src/lib/settingsContract.test.ts` compares:
	- Rust settings keys seeded via `set_default("key", ...)` in `app/src-tauri/src/lib.rs`
	- vs keys returned by `tauriAPI.getSettings()` in `app/src/lib/tauri.ts`

**B) Payload/response drift (TS types vs checked-in Rust JSON schemas)**

- ✅ A large set of TS types are checked against schema files under `app/src-tauri/gen/schemas/**`.
- ✅ Enforced examples (not exhaustive, but representative):
	- Settings shapes: `ProxySettings`, `HotkeyConfig`, `IntentRouterSettings`
	- Profiles/presets: `RewritePreset`, `RewriteProgramPromptProfile`
	- Logging/history: `RequestLog`, `SystemEvent`, `HistoryPageQuery`, `HistoryPageResult`, `HistoryDelete*`
	- Audio/overlay events: `OverlayAudioLevelPayload`, `MicTestAudioLevelPayload`
	- Pipeline events: `PipelineStateEvent`, `PipelineErrorPayload`, `pipeline-*-started` (null payloads)
	- Provider/model info + costs: `AvailableProvidersResponse`, `ModelPricing`, `CostSummary`, `CostByProvider`
	- Quick Ask events: `QuickAskStartedPayload`, `QuickAskAnswerPayload`
	- LLM helper command responses: `TestLlmRewriteResponse`, `IterateRewritePromptResponse`, etc.

**C) Local TS payload type duplication reduction**

- ✅ Some components stopped defining their own payload interfaces and now import types from `app/src/lib/tauri.ts`.

### Still missing (needs to be added)

**1) “Schemas are fresh” gate** (highest priority)

- ✅ Added a schema refresh gate that regenerates `app/src-tauri/gen/schemas/**` and fails CI if they differ.
- `pnpm -C app schemas:generate` regenerates schemas.
- `pnpm -C app schemas:check` runs the generation + `git diff --exit-code`.
- `schemas:check` is wired into `pnpm -C app check:ci`.

Where to add:

- A new script in `app/package.json` (e.g. `schemas:check`) and a CI hook via `pnpm -C app check:ci` (or called by it).

**2) Event-name drift guard (string name drift)**

- ✅ We now validate that Rust and TS agree on event *names*.
- Added `app/src/lib/tauri/events.ts` as a TS source of truth.
- Added `app/src/lib/contracts/eventsNameContract.test.ts` to compare Rust emit strings vs `EventMap` keys.

Where to add:

- TS source of truth: `app/src/lib/tauri/events.ts` (or similar)
- Test: `app/src/lib/contracts/eventsNameContract.test.ts` that parses Rust emit calls.

**3) Typed event map + typed listen/emit helpers**

- ✅ There is now a single TS `EventMap` in `app/src/lib/tauri/events.ts`.
- ✅ Typed helpers (`listenTyped` / `emitTyped`) are available and used in a couple of call sites.

Where to add:

- `app/src/lib/tauri/events.ts` exporting `EventMap`
- Wrapper helpers in `app/src/lib/tauri.ts` or `app/src/lib/tauri/events.ts`

**4) Tighten what the settings key drift check parses**

- ✅ Updated to include `store.set("...")` migrations inside `ensure_default_settings(...)`.
- (Still intentionally dumb parsing.)
This can miss settings keys that are:
	- migrated/renamed (not “default seeded”),
	- conditionally set,
	- or introduced in other Rust modules.

Where to add:

- Either expand parsing to include other patterns (e.g. `store.set("key"`) inside `ensure_default_settings(...)`),
- or maintain an explicit “settings contract keys list” in Rust that can be exported.

**5) Split and organize the contract tests**

- ⚠️ `app/src/lib/settingsContract.test.ts` is huge.
- It should be split so Dovie can update small pieces without wading through thousands of lines.

Where to add:

- New folder: `app/src/lib/contracts/**`.

**6) Explicitly decide what schemas are *not* part of the TS contract**

- ✅ Contract tests now explicitly list the schemas they validate, so platform/manifest artifacts stay out by default.

Where to add:

- A small allowlist/denylist in the schema tests (with comments).

## Main problem to solve next

Now that schemas are enforced in CI, the next risk is **event-name drift** (string event names) and lack of a typed TS event map.

## Plan steps

### 1) Add a “schemas are fresh” CI check

**Outcome:** If Rust types/events/commands change, and schemas weren’t regenerated + committed, CI fails with a diff.

Preferred implementation:

- Add a pnpm script (example name): `pnpm -C app schemas:check`
- That script:
	1. Regenerates schemas (by running the existing Rust schema export binaries / build scripts).
	2. Runs `git diff --exit-code app/src-tauri/gen/schemas`.

Acceptance criteria:

- [x] On a clean tree, `schemas:check` passes.
- [x] If a schema file is stale, `schemas:check` fails and the diff is limited to `app/src-tauri/gen/schemas/**`.

Notes for Dovie:

- This should be deterministic and offline (no network, no API keys).
- If schema generation is slow, we can scope it to only export “contract schemas” (settings/events/command payloads).

### 2) Add event-name drift protection (not just payload drift)

**Outcome:** If Rust renames an emitted event string, TS gets a failing test even if payload shapes didn’t change.

Approach:

- Create a TS source-of-truth list for event names (or an `EventMap` object):
	- Proposed new file: `app/src/lib/tauri/events.ts`
	- `export type EventMap = { "pipeline-state-changed": PipelineStateEvent; ... }`
- Add a test that parses Rust sources to extract emitted event names.
	- Start with the hotspots: `app/src-tauri/src/lib.rs` and any known emit helper modules.
	- Compare extracted Rust event names vs `keyof EventMap`.

Acceptance criteria:

- [x] Renaming an event string in Rust breaks the test with a clear “missing/extra events” list.
- [x] Adding a new event requires adding it to `EventMap` (or explicitly allowlisting it in the test).

### 3) Add typed TS wrappers for emitting/listening

**Outcome:** New UI code stops doing `listen<unknown>(...)` + casting; it uses one typed contract.

- Implement:
	- `listenTyped<K extends keyof EventMap>(name: K, cb: (payload: EventMap[K]) => void)`
	- `emitTyped<K extends keyof EventMap>(name: K, payload: EventMap[K])`
- Migrate a small set of call sites as proof:
	- Quick Ask: `quick-ask-started`, `quick-ask-answer`
	- Overlay audio meter event

Acceptance criteria:

- [x] At least 2 high-value event callsites use typed wrappers (Quick Ask + overlay audio meter).
- [x] No more local “duplicate interface” payload types for those events.

### 4) Split the contract test file (keep it maintainable)

**Outcome:** The contract tests stay readable and easy to update, not a 2,000-line “do not touch” file.

Split suggestion:

- `app/src/lib/contracts/settingsKeysContract.test.ts`
	- Rust `set_default` keys vs `tauriAPI.getSettings()` keys
- `app/src/lib/contracts/schemas/settingsSchemas.test.ts`
	- Proxy settings, hotkeys, router settings, presets, profiles, etc.
- `app/src/lib/contracts/schemas/eventsSchemas.test.ts`
	- Event payload schemas (including null payloads)
- `app/src/lib/contracts/schemas/commandsSchemas.test.ts`
	- Command response schemas (LLM responses etc.)

Acceptance criteria:

- [x] Same checks as today, but split into smaller files under `app/src/lib/contracts/**`.
- [x] Tests still run under `pnpm -C app test` and remain deterministic.

### 5) Expand coverage for known drift hotspots (only where it’s worth it)

**Outcome:** CI catches the stuff that historically breaks Dovie.

Priority targets:

- `RequestLog` + related entry subtypes
- Profile/preset/router shapes used by prompt settings
- Settings migrations / legacy fallback keys

Acceptance criteria:

- [x] Adding a field in Rust for these shapes requires updating TS types/tests (covered by the split schema contract tests).

## How we verify (backpressure)

- Minimum proving command during iteration:
	- `pnpm -C app test`
- Final gate (what CI cares about):
	- `pnpm -C app check:ci`

## Non-goals (explicitly out of scope)

- Full automatic TS type generation from Rust schemas (nice-to-have, but bigger).
- Refactoring Rust module layout (`lib.rs` / `pipeline.rs`) beyond what’s required for schema export + parsing.

## Notes / gotchas

- Parsing Rust for `set_default("key"...)` and `emit("event"...)` should stay intentionally “dumb” and stable.
	- It doesn’t need to understand Rust; it just needs to catch obvious drift.
- Some keys/events may be intentionally backend-only or legacy.
	- Keep a small allowlist in tests with comments explaining why.
- The event-name parser expects `emit_to_quick_ask(app_handle, "event-name", ...)` and extracts the second argument.
	- If that helper signature changes, update the regex in `app/src/lib/contracts/eventsNameContract.test.ts`.
