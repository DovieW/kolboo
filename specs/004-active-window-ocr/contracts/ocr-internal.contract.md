# Contract: OCR (internal Rust ↔ TypeScript)

This document describes the **internal** contract between the Rust/Tauri backend and the TypeScript UI for Active Window OCR context.

It is intentionally focused on:

- overlay polling payloads (UI state)
- manual OCR trigger commands
- events used for safe failure UX

> Note: The external OCR provider HTTP contract is documented separately in `contracts/ocr-service.openapi.yaml`.

## Goals

- Make OCR robust across flows (Quick Ask / Rewrite / Quick Replace).
- Avoid "phantom" OCR states caused by pipeline resets.
- Keep UI and backend in sync (typed payloads + event names).

## Concepts

### Session

A **session** represents one user invocation of a tool (Quick Ask / Rewrite / Quick Replace).

- `session_id` is a UUID string.
- OCR jobs belong to a session.

### OCR Job

For this feature, we track at least one OCR job:

- `purpose = "active_window_context"`

## Overlay polling contract

The overlay polls the backend for a composite state payload.

### `pipeline_get_overlay_state` response (planned additions)

Add the following fields (names illustrative; keep consistent with existing type names):

- `session_id: string | null`
- `ocr: {
    status: "not_started" | "running" | "done" | "failed" | "cancelled",
    mode_effective: "off" | "auto" | "manual",
    purpose: "active_window_context",
    started_at: string | null,        // ISO 8601 UTC
    finished_at: string | null,       // ISO 8601 UTC
    failed_reason: string | null      // sanitized
  }`

Why `session_id` matters:

- UI actions (manual trigger) should target the active session.
- Logs become correlatable: "OCR started" → "OCR attached" in the same session.

## Manual OCR trigger command

### `pipeline_trigger_active_window_ocr`

**Purpose:** Start OCR for the current session when OCR mode is manual.

**Request (planned):**

- `session_id?: string` (optional; if provided and it does not match the current session, the backend should no-op and return a status indicating mismatch)

**Response (planned):**

- `{
    session_id: string | null,
    ocr_status: "not_started" | "running" | "done" | "failed" | "cancelled"
  }`

## Failure UX event

### Event: `overlay-ocr-context-unavailable`

Emitted when OCR context cannot be obtained in a user-friendly way.

Payload:

- `{
    message?: string
  }`

Rules:

- Message must be calm and non-technical.
- Do not include URLs, stack traces, API keys, or raw provider responses.

## Versioning / compatibility

- If we add `session_id` fields, update both:
  - Rust emitters/commands
  - TypeScript event maps + invoke wrappers + types

- Tests should validate that UI-facing types match Rust payload shapes.
