# Slice Completion Checklist

Use this checklist for every architecture-deepening slice before marking the slice complete in `tasks.md`.

## Required for every slice

- [x] Module-interface contract is satisfied.
- [x] Existing user-visible behavior is preserved or deliberate changes are documented.
- [x] Edge-case matrix rows for the slice map to deterministic automated tests.
- [x] Every changed/new in-scope module has 100% statement, branch, and function coverage for reachable behavior.
- [x] Any defect found during implementation has a regression test that fails before the fix and passes after the fix.
- [x] No default validation requires real network calls, API keys, paid accounts, audio hardware, screenshots, timing sleeps, or user interaction.
- [x] Formatting ran before tests/checks.
- [x] Smallest relevant tests/checks passed.
- [x] Generated schemas/types/events were regenerated or explicitly confirmed not touched.
- [x] VS Code Problems show no new errors or warnings in touched files.
- [x] Coverage evidence is recorded in `coverage-evidence.md`.
- [x] Safe-stop/rollback note remains valid.

## Safe-stop notes by slice

| Slice                        | Safe-stop strategy                                                                                | Status   |
| ---------------------------- | ------------------------------------------------------------------------------------------------- | -------- |
| US8 Coverage Gate            | Stop after evidence templates and helper contracts exist; no production behavior changes.         | Complete |
| US1 OCR Session              | Keep `SharedPipeline` methods delegating to old behavior until state wrapper tests pass.          | Complete |
| US2 Settings View            | Stop after read-only defaults/views and drift tests; do not switch writes until parity is proven. | Complete |
| US3 Runtime Sync Policy      | Stop after policy table tests; migrate call sites incrementally.                                  | Complete |
| US4 Routing Decision         | Adapt router functions to new decision type before switching Transcription Flow.                  | Complete |
| US5 Profile Resolution       | Extract matching/effective behavior without changing call order; compare fixtures.                | Complete |
| US6 Local Provider Lifecycle | Wrap existing cache/load helpers before consolidating behavior.                                   | Complete |
| US7 Provider-Family Seam     | Record defer/reject when two-adapter proof is absent; do not introduce pass-through seams.        | Complete |
