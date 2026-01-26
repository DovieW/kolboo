# Data Model: Windows UIA Context + Insertion

This describes the data shapes we need for UIA-first capture/insertion, and what we persist.

## Entity: `WindowsTextTargetSnapshot`

Represents the focused element at the moment we plan to capture/insert.

Fields (suggested):

- `capturedAtMs: number`
- `processId: number | null`
- `exePath: string | null` (best-effort)
- `windowTitle: string | null` (best-effort)
- `uiaRuntimeId: number[] | null` (best-effort; may not be stable across sessions)
- `isPassword: boolean | null` (null if unknown)
- `isEnabled: boolean | null`
- `isReadOnly: boolean | null`
- `supportsTextPattern: boolean`
- `supportsValuePattern: boolean`

Validation rules:

- If `isPassword === true`, insertion/capture must be blocked.
- If `supportsValuePattern === true`, require `isEnabled === true` and `isReadOnly === false` before using it.

## Entity: `WindowsTextContext`

Text content used for prompts.

Fields:

- `selectionText: string | null`
- `surroundingText: string | null`
- `source: "uia" | "clipboard" | "none"`
- `truncated: boolean`
- `maxChars: number`

Validation rules:

- Always truncate to `maxChars`.
- Never include any text when `snapshot.isPassword === true`.

## Entity: `WindowsInsertPlan`

What the backend intends to do for insertion.

Fields:

- `method: "uiaValuePattern" | "paste" | "typing" | "none"`
- `reason: string` (human-readable, safe to log)
- `allowed: boolean` (false if unsafe)

State transitions:

- Build plan from snapshot + app capability memory
- Execute plan, record outcome
- Update capability memory

## Entity (persisted): `WindowsAppCapabilityMemory`

Persisted map from “app identity” to observed capabilities.

Suggested shape:

- `version: number`
- `apps: Record<string, {
  lastSeenAtMs: number,
  preferMethod: "uiaValuePattern" | "paste" | "typing" | "none" | null,
  stats: {
    uiaValuePatternSuccess: number,
    uiaValuePatternFail: number,
    pasteSuccess: number,
    pasteFail: number,
    typingSuccess: number,
    typingFail: number,
  },
}>`

Where `string` key is ideally derived from executable path (best-effort) with a stable normalization.
