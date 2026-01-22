# TODO (code-grounded backlog)

This backlog is based on reading the current **code and configuration** in this repo (primarily `app/src-tauri/src/**` and `app/src/**`). It intentionally **does not use existing documentation** as a source of truth.

Legend:

- **P0**: likely bug/crash/security/data loss
- **P1**: high-impact improvement (stability, UX, privacy, perf)
- **P2**: nice-to-have / cleanup

## P1 — Security & privacy hardening

## P1 — Performance & responsiveness

## P1 — Architecture & maintainability

### Continue splitting `app/src-tauri/src/lib.rs`

- **Where:** `app/src-tauri/src/lib.rs`
- **Current state:** event payload types + a couple of shared helpers are now in dedicated modules.
- **Todo:**
  - Optional: if/when we need to touch the recording-stop flow again, consider moving `stop_recording` into its own module (keep a small wrapper/re-export in `lib.rs`).
    - This is purely for maintainability (not required for correctness).
    - See `docs/Refactors/2_MEDIUM.md` for notes on a low-risk way to do it.
  - Split remaining responsibilities into focused modules (tray, shortcuts lifecycle, window mgmt, pipeline session, retention, etc.).

### Centralize settings schema + migrations

- **Where:** frontend normalization/migration logic in `app/src/lib/tauri.ts` and backend default seeding logic.
- **Problem:** duplication increases drift risk.
- **Todo:**
  - Define a single settings schema source (JSON Schema or Rust struct + generated TS types).
  - Add explicit settings versioning + migrations.
  - Add a startup “settings doctor” command: validate + normalize.

## P1 — UX & product improvements

### Expand output-mode options

---

## P2 — Observability & debugging

---

## P2 — Tooling, CI/CD, and repo hygiene

---

## Transparency & in-app disclosures (derived from code behavior)

---

## Provider-specific enhancements

