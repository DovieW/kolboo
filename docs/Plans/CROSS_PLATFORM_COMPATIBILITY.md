# Cross-platform compatibility roadmap

This is the living plan for **platform-specific behavior** in Kolboo.

Purpose:

- Track features that currently behave differently across Windows/macOS/Linux.
- Record constraints (permissions, OS APIs, Wayland limitations, etc.).
- Outline staged implementation milestones so we can improve parity over time.

This is *not* a promise that every feature can be perfectly supported on every platform; some things are fundamentally limited (notably on Wayland).

## Principles

1. **Best-effort parity, predictable fallbacks**
   - Prefer correctness where possible.
   - Otherwise fall back deterministically (avoid random “last monitor” behavior).

2. **No surprise permissions**
   - If a feature requires Accessibility / Screen Recording / similar permissions, we must be explicit in UI copy and error handling.

3. **Physical-pixel window placement**
   - For overlays, prefer physical coordinates to avoid DPI double-scaling issues (especially on Windows, and mixed-DPI multi-monitor setups).

4. **Never off-screen**
   - Always clamp to monitor bounds with a small safety margin.

## Known cross-platform work items

### 1) Overlay monitor targeting (`overlay_monitor_target`)

**What it is:** A global setting that chooses which monitor always-on-top overlay windows appear on.

- Setting key: `overlay_monitor_target`
- Values: `main` | `cursor` | `active_window`
- Affects windows:
  - `overlay` (recording widget)
  - `quick_ask` (full-monitor transparent overlay)

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
