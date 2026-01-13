import type { HotkeyConfig } from "./tauri";

// ============================================================================
// DEFAULT HOTKEY CONSTANTS - Single source of truth for all default hotkeys
// These must match the Rust defaults in settings.rs
// ============================================================================

/** Default modifiers for the toggle hotkey (none) */
export const DEFAULT_HOTKEY_MODIFIERS: string[] = [];

const IS_WINDOWS =
	typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent);

/** Default key for toggle recording.
 * - Windows: modifier-only hotkey (Right Alt / AltGr)
 * - Other: F3 (portable, supported by global shortcut plugin)
 */
export const DEFAULT_TOGGLE_KEY = IS_WINDOWS ? "AltRight" : "F3";

// ============================================================================

/** Default toggle hotkey config */
export const DEFAULT_TOGGLE_HOTKEY: HotkeyConfig = {
	modifiers: DEFAULT_HOTKEY_MODIFIERS,
	key: DEFAULT_TOGGLE_KEY,
};

/** Default hold-to-record hotkey config (unset) */
export const DEFAULT_HOLD_HOTKEY: HotkeyConfig | null = null;

/** Default paste last transcription hotkey config (unset) */
export const DEFAULT_PASTE_LAST_HOTKEY: HotkeyConfig | null = null;

/** Default retry last recording hotkey config (unset) */
export const DEFAULT_RETRY_HOTKEY: HotkeyConfig | null = null;

/** Default Quick Ask hold hotkey config (unset) */
export const DEFAULT_QUICK_ASK_HOLD_HOTKEY: HotkeyConfig | null = null;

/** Default Quick Ask toggle hotkey config (unset) */
export const DEFAULT_QUICK_ASK_TOGGLE_HOTKEY: HotkeyConfig | null = null;
