# Cross-platform compatibility roadmap

> **Active engineering plan:** Windows remains the currently supported tester platform while Linux and macOS are being built and validated. This work does not create a public release promise or delivery date.

This is the living plan for **platform-specific behavior** in Kolboo.

Purpose:

- Track features that currently behave differently across Windows/macOS/Linux.
- Record constraints (permissions, OS APIs, Wayland limitations, etc.).
- Outline staged implementation milestones so we can improve parity over time.

This is not a promise of perfect parity. Platform limitations—especially Wayland global-input/window restrictions and macOS permissions—must produce explicit, deterministic fallbacks.

## Principles

1. **Best-effort parity, predictable fallbacks**
   - Prefer correctness where possible.
   - Otherwise fall back deterministically (avoid random “last monitor” behavior).

2. **No surprise permissions**
   - If a feature requires Accessibility / Screen Recording / similar permissions, we must be explicit in UI copy and error handling.

3. **Single-owner physical-pixel window placement**
   - The frontend requests semantic compact/expanded layout only. Rust owns the complete native size and position transaction.
   - Calculate each rectangle from the selected monitor's usable work area, a stable anchor, and one DPI conversion. Never derive a new rectangle from the last reported window rectangle.

4. **Never off-screen or under system UI**
   - Always clamp to the monitor work area so taskbars, docks, and menu bars are respected.

5. **Capabilities before assumptions**
   - Detect platform/session capabilities at runtime where behavior can vary.
   - Do not present a control as working when the current platform cannot implement it.

6. **Native validation**
   - Add native Linux and macOS CI jobs early.
   - Do not call a platform supported until packaging and the manual acceptance matrix pass on real hardware or a representative native runner.

## Platform workstreams

### Build and packaging

- Add native default-feature lint, test, and build jobs for Linux and macOS.
- Document required system packages, architectures, bundle formats, install/uninstall, and cache behavior.
- Keep platform release channels private while public distribution is deferred.

Linux development prerequisites, build commands, and current X11/Wayland behavior are documented in [Linux development](../How%20Tos/LINUX_DEVELOPMENT.md). The `Linux Build` workflow now compiles the native default-feature binary on Ubuntu; installable packaging and the acceptance matrix remain open.

Private macOS artifact builds and the first native acceptance pass are documented in [macOS development](../How%20Tos/MACOS_DEVELOPMENT.md). The workflow is manual-only and produces ad-hoc-signed test bundles. An Apple Silicon bundle passed CI construction, signing, and architecture checks, but native acceptance is paused until a Mac is available. Signing, notarization, native behavior, and platform support remain open.

### Audio capture

- Verify device enumeration, default-device changes, sample formats, resampling, meters, hot-mic behavior, and device-loss recovery.
- Test PipeWire/PulseAudio behavior on Linux and CoreAudio behavior on macOS.
- Show actionable permission/device errors rather than generic recording failures.

### Global shortcuts and text insertion

- Separate Windows hooks/input insertion from macOS and Linux adapters.
- Document macOS Accessibility permission requirements.
- Support X11 where global input is available and provide honest Wayland fallbacks when compositor protocols do not allow equivalent behavior.

### Permissions and secure storage

- Add permission-state diagnostics for microphone, Accessibility, Screen Recording, notifications, and startup registration.
- Verify macOS Keychain and Linux secret-service behavior, including unavailable/locked-store errors.
- Never fall back to silent plaintext secret storage.

### Windows, tray, startup, and updates

- Validate overlay focus/always-on-top behavior, tray lifecycle, startup registration, notifications, and updater behavior per platform.
- Keep updater/public distribution separate from the platform-support acceptance decision while publication is deferred.

### Acceptance matrix

For each platform, record native evidence for:

- install, first launch, and permissions;
- account-free Community/local/BYOK dictation;
- managed Personal dictation;
- shortcuts, overlay, output insertion, tray, and startup;
- secure storage and account session recovery;
- logs, Sentry crash/error capture, and support-safe request correlation;
- upgrade/rollback behavior appropriate to the private tester channel.

## Known cross-platform work items

### 1) Overlay monitor targeting (`overlay_monitor_target`)

**What it is:** A global setting that chooses which monitor always-on-top overlay windows appear on.

- Setting key: `overlay_monitor_target`
- Values: `main` | `cursor` | `active_window`
- Affects windows:
  - `overlay` (recording widget)
  - `quick_ask` (answer panel)

The recording widget currently uses a backend-owned anchor layout. Compact and expanded sizes are constants, bottom-center is the default, and reapplying the same layout is idempotent. The hover and Quick Ask panels share the same work-area geometry helpers.

#### Current behavior

- **Windows:**
  - `main` → primary monitor (Tauri `primary_monitor()`)
  - `cursor` → monitor containing mouse cursor (Win32 `GetCursorPos`)
  - `active_window` → monitor containing foreground window (Win32 `GetForegroundWindow` + `GetWindowRect`)

- **macOS / Linux:**
  - `cursor` and `active_window` currently fall back to:
    `current_monitor()` → `primary_monitor()` → first available monitor.

Backend reference: `app/src-tauri/src/commands/overlay.rs` (`resolve_target_monitor(...)`).

#### Platform plan

##### Windows (keep as-is)

- Keep Win32 implementations.
- Continue using physical pixels.

##### macOS

- `cursor`:
  - Preferred: CoreGraphics global cursor location (no Accessibility permission)
    - e.g., `CGEventCreate(NULL)` + `CGEventGetLocation(...)`.
  - Validate coordinate conversion vs Tauri monitor geometry (multi-monitor, negative origins).

- `active_window` (best-effort):
  - Preferred: Accessibility API (AXUIElement) to get focused window bounds.
  - Handle missing permissions gracefully and fall back.
  - Add UI copy: “Active window may require Accessibility permission on macOS.”

##### Linux

Linux splits into **X11** vs **Wayland**:

- Main overlay placement:
  - X11/XWayland: use the same work-area anchor controller as Windows and macOS.
  - Native Wayland: standard `xdg-shell` toplevels cannot request absolute global placement. Kolboo automatically selects XWayland when available and documents the compositor-placement fallback when it is not.

- `cursor`:
  - X11: implement global pointer query.
  - Wayland: likely not possible → keep fallback + document limitation.

- `active_window`:
  - X11: implement `_NET_ACTIVE_WINDOW` + window geometry.
  - Wayland: typically not possible → keep fallback + document limitation.

#### Implementation milestones

1. Add debug logging on fallback paths (gated behind existing debug/diagnostic toggle).
2. Implement macOS `cursor` targeting.
3. Implement Linux X11 `cursor` targeting; detect Wayland and fall back.
4. Implement macOS `active_window` best-effort with permission-aware fallback.
5. Implement Linux X11 `active_window` targeting.

#### Manual test checklist

- Two monitors with different DPI/scaling.
- Monitor arrangement including negative origins (left-of-primary).
- Rapid switching of active window between monitors.

Verify for each platform:

- `main`: overlay + quick ask appear on primary.
- `cursor`: move cursor to each monitor, trigger overlay/quick ask → appears on that monitor.
- `active_window`: focus a window on each monitor, trigger → appears on that monitor.

## Add new items here

When you find a platform-specific behavior that needs parity work, add a new numbered section above with:

- What it is + where it lives (files/symbols)
- Current behavior by platform
- Constraints/permissions
- Milestones + test checklist
