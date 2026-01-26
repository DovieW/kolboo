# Research: Windows UI Automation (UIA) Context + Insertion

**Feature**: `specs/001-uia-text-insertion/spec.md`
**Date**: 2026-01-25

This document records the key technical decisions for the UIA-first Windows text context + insertion feature.

## Sources (primary)

- Microsoft Learn (UIA threading): https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-threading
- Microsoft Learn (UIA events threading note): https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-events
- Microsoft Learn (TextPattern / TextRange): https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-textpattern
- Microsoft Learn (ValuePattern / SetValue): https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingvalue
- Microsoft Learn (performance considerations): https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-understandingperformanceissues
- Microsoft Learn (IsPassword property): https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-automation-element-propids
- Microsoft Learn (GetFocusedElement can be transient): https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation-getfocusedelement
- Microsoft Learn (UIA error codes): https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-error-codes
- Microsoft Learn (COM init: CoInitializeEx): https://learn.microsoft.com/en-us/windows/win32/api/objbase/nf-objbase-coinitializeex

Secondary (risk notes):

- Chromium UIA support status/flag: https://chromium.googlesource.com/chromium/src/+/main/docs/accessibility/ui-automation.md
- Electron accessibility support toggle: https://www.electronjs.org/docs/latest/api/app#appsetaccessibilitysupportenabledenabled-macos-windows

## Decision 1: UIA calls run on a dedicated MTA thread

**Decision**: All UIA interactions (focus, pattern queries, reads/writes) happen off the UI thread on a dedicated background thread initialized with COM MTA (`CoInitializeEx(..., COINIT_MULTITHREADED)`).

**Rationale**:

- UIA docs explicitly warn about threading concerns and recommend calling UIA from a separate thread to avoid UI deadlocks/hangs.
- COM apartment mode matters; MTA is the recommended approach for UIA client calls.

**Alternatives considered**:

- Calling UIA from the main Tauri thread: rejected due to hang risk.
- Using STA only: rejected due to lower confidence and typical guidance favoring MTA for UIA client work.

## Decision 2: Focus acquisition uses GetFocusedElement with retry

**Decision**: Acquire insertion/capture target using `IUIAutomation::GetFocusedElement()`.

If it returns `UIA_E_ELEMENTNOTAVAILABLE`, retry a small number of times.

**Rationale**:

- Microsoft docs note the focused element may be transient and that clients should retry on `UIA_E_ELEMENTNOTAVAILABLE`.

**Alternatives considered**:

- Walking from a window handle only (ElementFromHandle): useful as fallback, but not always correct if focus is inside nested controls.

## Decision 3: Safety policy blocks password/secure fields

**Decision**: If `UIA_IsPasswordPropertyId` is true, do not read context and do not insert text; fall back to “copy transcript to clipboard + show toast”.

**Rationale**:

- UIA provides an explicit password indicator. Attempting to read such fields can be blocked or unsafe.

**Alternatives considered**:

- Allow insertion without reading: rejected; spec requires blocking inserts into password fields.

## Decision 4: Editability gating uses enabled + read-only checks

**Decision**:

- For any direct UIA insertion (ValuePattern `SetValue`), require:
  - element enabled, and
  - not read-only

**Rationale**:

- ValuePattern documentation states `SetValue` only works when enabled and not read-only.

**Alternatives considered**:

- “Try SetValue and ignore errors”: rejected; we want deterministic, explainable fallbacks and safety checks.

## Decision 5: Context capture is bounded and pattern-based

**Decision**: Use UIA patterns when available:

1. If TextPattern is available, prefer:
   - selection text (when present) and/or
   - a bounded surrounding excerpt via `DocumentRange.GetText(maxChars)` (or equivalent range around insertion point)
2. If TextPattern is not available, context capture may be empty (clipboard context is a separate explicit opt-in).

Always enforce max length limits.

**Rationale**:

- UIA calls can be cross-process and expensive; performance guidance recommends avoiding large reads and minimizing calls.
- TextPattern is the standard way to retrieve text context (it is read-only).

**Alternatives considered**:

- Full-document reads: rejected (performance + privacy).
- Clipboard “copy selection” as a fallback: rejected; clipboard context is a separate explicit opt-in.

## Decision 6: Insertion uses a reliability ladder

**Decision**: Attempt insertion in this order:

1. UIA ValuePattern `SetValue` (when supported and safe)
2. Paste-based insertion (write to clipboard, issue paste, restore clipboard)
3. Typing simulation

**Rationale**:

- Different apps expose different accessibility surfaces; ValuePattern works well in some controls (e.g., native edit controls), while browsers/Electron often require paste/typing.
- A ladder maximizes success while keeping “safe fallback” behavior consistent.

**Alternatives considered**:

- Only typing simulation: rejected (less reliable in some controls, and slower).
- Only paste: rejected (clipboard side effects and blocked paste situations).

## Decision 7: Browser/Electron UIA support is inconsistent; treat it as “best effort”

**Decision**: Do not assume TextPattern/ValuePattern exists in Chromium/Electron-based editors. Make UIA context capture a best-effort; fall back gracefully.

**Rationale**:

- Chromium documents that its UIA provider is under development and may require a feature flag.
- Electron exposes an accessibility support toggle that can have performance implications.

**Alternatives considered**:

- Relying on enabling accessibility globally: rejected (out of scope; not our app’s decision for other apps, and can have perf impact).

## Decision 8: No UIAccess requirement

**Decision**: Do not require `uiAccess=true` or privileged accessibility access.

**Rationale**:

- UIAccess has signing and install-location requirements and is intended for assistive tech apps. This feature must work without it.

**Alternatives considered**:

- Making UIAccess mandatory: rejected (packaging/signing complexity; not appropriate for this app).

## Decision 9: Persist per-app capability memory

**Decision**: Store per-app insertion capability observations locally (e.g., “ValuePattern worked last time in app X”) and consult them to choose the fastest/most reliable method next time.

**Rationale**:

- Capability can vary widely by app and can change after updates; learning from outcomes improves reliability without hardcoded app lists.

**Alternatives considered**:

- Hardcoded per-app heuristics: rejected (brittle and hard to maintain).

## Open Questions (to validate during implementation)

- Exact metadata keys for “app identity” (exe path vs product name vs window class). Default recommendation: normalize by executable path when available.
- Exact UIA range strategy for “surrounding excerpt” (selection vs caret vicinity) across controls.
- Practical timeouts and retry counts that balance success with perceived speed.

## Developer note: UIA limitations

- Chromium/Electron apps may not expose TextPattern/ValuePattern reliably.
- Expect context capture to be empty in some editors; insertion should fall back to paste/typing.
- Avoid assuming accessibility support is enabled in third-party apps.
