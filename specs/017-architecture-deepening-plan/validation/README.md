# Architecture Deepening Validation

This directory stores evidence for the `017-architecture-deepening-plan` implementation.

Use these files as the source of truth while completing slices:

- `coverage-evidence.md` — in-scope modules, commands, coverage results, regression-defect log, and accepted risks.
- `edge-case-matrix.md` — mapping from documented edge cases to deterministic automated tests.
- `slice-checklist.md` — per-slice completion and safe-stop checklist.
- `provider-family-decisions.md` — two-adapter proof, deferral, or rejection decisions for provider-family seams.
- `rust-coverage-interface.md` — expected Rust coverage helper behavior before the helper is implemented.

Default validation must remain deterministic: no real network calls, API keys, audio devices, screenshots, timing sleeps, paid accounts, or user interaction.
