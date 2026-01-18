# Ticket: Seed a typed event contract (TS-first)

## Goal (what we want)

Reduce Rust/TS event drift by defining a typed event map on the TypeScript side and using typed helpers for a small set of high-traffic events.

- We want: a single place to see event names + payload shapes.
- So that: when an event payload changes, TypeScript breaks loudly instead of silently.

## Context (what exists today)

- Events are mostly stringly-typed (`pipeline-state-changed`, `settings-changed`, etc.).
- Overlay and main UI listen/emit these events from multiple places.

## Acceptance criteria (how we know it’s done)

- [ ] Add a TS event map (e.g. `app/src/lib/events.ts` or `app/src/lib/tauri/events.ts`) that defines event names and payload types.
- [ ] Add small typed wrappers for emit/listen that enforce the map.
- [ ] Update at least 2 call sites to use the typed wrapper (suggested: `settings-changed` and `pipeline-state-changed`).
- [ ] Keep runtime behavior unchanged (still the same event names on the wire).

## Edge cases / gotchas

- Don’t break existing listeners that still use the raw string APIs.
- Payload types should match actual Rust emitters (be conservative where needed).

## Non-goals (explicitly out of scope)

- Full Rust-side codegen or schema generation (can be a future ticket).
- Converting every event in one go.
