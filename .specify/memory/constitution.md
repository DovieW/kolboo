<!--
Sync Impact Report

- Version change: 1.0.1 → 1.1.0
- Modified principles:
	- Deterministic tests (no real network) → Deterministic validation and no hidden dependencies
	- UI ↔ backend contract stays in sync → End-to-end contracts stay generated and synchronized
	- Settings are canonical, migrated, and applied immediately → Settings, migrations, and runtime sync are first-class
	- Secrets never leak → User trust, privacy, and secret boundaries are default
	- Tight diffs, strict tooling, CI gate → Small changes, strict tooling, and documented drift
- Added principles:
	- Pipeline state-machine safety
- Added sections:
	- Spec, Plan, and Task Requirements
	- expanded Quality Gates
	- expanded Workflow & Review
- Removed sections: none
- Templates requiring updates:
	- ✅ .specify/templates/plan-template.md
	- ✅ .specify/templates/spec-template.md
	- ✅ .specify/templates/tasks-template.md
	- ✅ .specify/templates/checklist-template.md
	- ✅ .specify/templates/agent-file-template.md (reviewed; no change required)
	- ✅ .specify/templates/commands/*.md (folder absent; no updates required)
	- ✅ README.md / CONTRIBUTING.md / SECURITY.md / .github/copilot-instructions.md (reviewed; no change required)
- Follow-up TODOs: none
-->

# Kolboo Constitution

## Core Principles

### User trust, privacy, and secret boundaries are default

Kolboo handles microphone audio, transcripts, prompts, provider responses, API keys, and
authorization material. Every feature MUST classify the sensitive data it touches before
implementation starts.

- Audio, transcripts, prompts, provider responses, request logs, API keys, tokens, raw
	authorization headers, and org/policy identifiers MUST be treated as sensitive.
- Network use MUST be explicit in the feature spec or plan. Local-only behavior MUST remain
	local-only unless the user deliberately enables a provider, cloud service, or telemetry path.
- Secrets MUST be stored through the app’s approved secret-storage path, not hardcoded, not
	checked in, and not written to routine logs.
- Logs, crash reports, screenshots, exported diagnostics, and test fixtures MUST redact secrets
	and minimize transcript/audio content.
- Legacy plaintext secret fallback behavior MAY exist only as a migration bridge and MUST be
	documented where user-facing privacy expectations depend on it.

Rationale: a dictation app can capture highly personal data; trust is lost quickly and restored
slowly.

### Deterministic validation and no hidden dependencies

Automated validation MUST be repeatable on a fresh contributor machine without external
accounts, real provider calls, real payment flows, or physical audio devices.

- Tests MUST NOT make real network calls by default.
- Tests MUST NOT require real API keys, organization membership, paid accounts, or provider
	quotas by default.
- Tests that require keys, hardware, cloud services, or manual setup MUST be ignored/manual by
	default and include instructions for running them deliberately.
- Behavior changes MUST add or update focused tests unless the plan documents why the change is
	docs-only, wiring-only, or otherwise already covered.
- Timing-sensitive tests MUST control time or state directly; sleeping and flaky timing windows
	MUST NOT be used as proof of correctness.

Rationale: reliable tests are cheaper than mystery regressions and much cheaper than haunted CI.

### End-to-end contracts stay generated and synchronized

Kolboo’s UI and backend are separate systems joined by Tauri commands, events, generated types,
schemas, settings shapes, and provider/model contracts. Contract changes MUST be updated across
every layer in the same change.

- Tauri command additions/renames/removals MUST update Rust registration/handlers, TypeScript
	invoke wrappers, exported TypeScript types, tests, and user-facing call sites.
- Tauri event additions/renames/removals MUST update Rust emitters, generated event/type files,
	listeners, tests, and overlay windows that consume the event.
- Schema-generated files MUST be regenerated and checked when Rust/TypeScript contracts change.
- Provider/model identifiers and capability metadata MUST remain backward-compatible with older
	settings unless a migration is included.
- A plan that touches both `app/src/**` and `app/src-tauri/src/**` MUST include an end-to-end
	validation path from UI action through Rust behavior and back to visible UI/event state.

Rationale: typecheck cannot catch contracts that are correct on one side and stale on the other.

### Settings, migrations, and runtime sync are first-class

Settings are a compatibility contract, not incidental UI state.

- `settings.json` via the Tauri store is the canonical persisted source for non-secret local
	settings and cached policy/config state.
- Secret/session material MUST use approved secure storage unless explicitly documented as a
	legacy migration fallback.
- `null` often means explicitly disabled; missing or invalid values MUST fall back to defaults
	without erasing intentional disabled state.
- Any added, renamed, removed, or reshaped setting MUST update Rust default seeding/migrations,
	TypeScript normalization/migrations, tests, and relevant documentation.
- Runtime-affecting setting changes MUST persist the value and immediately sync the running
	pipeline configuration.
- Overlay-affecting setting changes MUST emit the settings-change signal needed by secondary
	windows to refresh cached state.

Rationale: settings bugs become upgrade bugs, and upgrade bugs look like user data loss.

### Pipeline state-machine safety

Recording, transcription, cancellation, hotkeys, overlays, and background work MUST use explicit
state transitions instead of ad-hoc flags.

- Pipeline behavior MUST preserve the existing state-machine invariants in the Rust backend.
- Cancellation and escape-to-cancel behavior MUST be idempotent and safe when invoked repeatedly.
- Global shortcut registration MUST avoid re-entrant registration/unregistration races.
- Background tasks MUST have clear ownership, cleanup, and error propagation paths.
- UI-only transient states MAY exist, but they MUST be mapped deliberately to Rust pipeline
	states and backend events.

Rationale: audio and hotkey workflows fail in the weird gaps between states; explicit transitions
keep the weird contained.

### Small changes, strict tooling, and documented drift

Changes MUST stay reviewable and validated with the smallest command set that proves the work,
then with the full merge gate before merge.

- Diffs MUST be focused. Drive-by refactors and unrelated formatting are not allowed.
- If a useful refactor is too large or out of scope, record it in the repo’s refactor docs instead
	of smuggling it into the feature.
- Formatting commands MUST run before test/check commands for touched areas.
- TypeScript strictness, Biome, Rust formatting, Clippy expectations, schema checks, generated
	contract checks, and Vitest/Cargo tests MUST remain clean for affected areas.
- Before merge, the repo’s CI gate MUST pass unless the PR explicitly documents an approved
	temporary exception.

Rationale: small, clean changes are faster to review, easier to revert, and less likely to break
the dictation loop people rely on.

## Spec, Plan, and Task Requirements

- Specs MUST identify sensitive data, external services/network use, settings or persisted state,
	contract surfaces, and user-visible failure modes when applicable.
- Plans MUST include a Constitution Check with pass/fail notes for privacy/secrets,
	deterministic validation, contract sync, settings/migrations, pipeline safety, and tooling.
- Tasks MUST include concrete validation work for behavior changes, settings migrations, contract
	generation/checks, documentation updates, and refactor-doc updates when applicable.
- Feature artifacts MUST keep user stories independently testable; shared infrastructure MUST be
	separated from story-specific work.
- Any unavoidable constitution violation MUST be listed in the plan’s Complexity Tracking table
	with the rejected simpler alternative.

## Quality Gates

- No new TypeScript, Rust, Biome, Clippy, schema, generated-contract, or VS Code Problems errors
	or warnings in touched areas.
- Local validation MUST use the smallest relevant command set first:
	- docs/spec-only changes: no app validation required unless docs reference generated behavior;
	- UI-only changes under `app/src/**`: format/lint, then UI tests;
	- Rust-only changes under `app/src-tauri/**`: Rust format, then Rust tests;
	- cross-cutting changes: combined test set, then final CI gate before handoff or merge.
- Commands that invoke Cargo locally MUST use `sccache` when available and conservative Cargo
	build jobs to keep the machine responsive.
- New or changed tests MUST be deterministic and MUST use fake inputs instead of real providers,
	networks, API keys, audio devices, or timing sleeps.
- Any user-facing behavior change MUST include a short plain-language explanation in the PR,
	release note, changelog entry, or relevant docs.

## Workflow & Review

- Each feature SHOULD be deliverable as an independently testable MVP slice before optional work
	is added.
- Reviews MUST verify every applicable MUST rule in this constitution, not only code style.
- When UI and backend are both touched, review MUST trace at least one path from UI action to Rust
	command/event behavior and back to visible UI state.
- When settings are touched, review MUST trace defaulting, migration, persistence, runtime sync,
	and secondary-window refresh behavior.
- When secrets, transcripts, audio, logs, or org/session state are touched, review MUST verify
	redaction, storage location, and user-facing privacy expectations.
- Documentation updates are required when behavior, settings, commands, events, privacy/security
	expectations, or provider/model support changes.

## Governance

- This constitution is the top-level authority for development practices in this repo. More
	specific instruction files MAY add stricter rules, but they MUST NOT weaken these rules.
- Any PR review MUST check for constitution compliance, especially MUST statements.
- Amendments:
	1. Make the change in `.specify/memory/constitution.md`.
	2. Include a Sync Impact Report at the top of the file.
	3. State the version bump rationale and migration/review impact.
	4. Update dependent templates, command prompts, runtime guidance docs, and contributor docs in
		the same change when they reference modified rules.
- Versioning policy:
	- MAJOR: removed or redefined principles in a backward-incompatible way.
	- MINOR: new principle, new mandatory section, or materially expanded governance.
	- PATCH: clarifications, wording fixes, typo fixes, or non-semantic refinements.
- Ratification date is the original adoption date. Last amended date MUST use ISO format and
	change whenever the constitution content changes.

**Version**: 1.1.0 | **Ratified**: 2026-01-25 | **Last Amended**: 2026-04-30
