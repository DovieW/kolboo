import type { HotkeyConfig } from "./tauri";

// ============================================================================
// DEFAULT HOTKEY CONSTANTS - Single source of truth for all default hotkeys
// These must match the Rust defaults in settings.rs
// ============================================================================

/** Default modifiers for the toggle hotkey (none) */
export const DEFAULT_HOTKEY_MODIFIERS: string[] = [];

/** Default key for toggle recording (F3) */
export const DEFAULT_TOGGLE_KEY = "F3";

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
