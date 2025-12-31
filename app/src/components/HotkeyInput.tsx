import { Button, Kbd, Select } from "@mantine/core";
import { useEffect, useMemo, useRef, useState } from "react";
import { useRecordHotkeys } from "react-hotkeys-hook";
import type { HotkeyConfig } from "../lib/tauri";
import { tauriAPI } from "../lib/tauri";

interface HotkeyInputProps {
  label: string;
  description?: string;
  value: HotkeyConfig | null;
  onChange: (config: HotkeyConfig | null) => void;
  disabled?: boolean;
  // Coordinated recording state (managed by parent)
  isRecording?: boolean;
  onStartRecording?: () => void;
  onStopRecording?: () => void;
}

// Known modifier keys (lowercase, as returned by react-hotkeys-hook)
const MODIFIER_KEYS = new Set(["ctrl", "alt", "shift", "meta", "mod"]);

// Keys that can be used alone without modifiers (function keys, etc.)
const STANDALONE_KEYS = new Set([
  "f1",
  "f2",
  "f3",
  "f4",
  "f5",
  "f6",
  "f7",
  "f8",
  "f9",
  "f10",
  "f11",
  "f12",
  "insert",
  "delete",
  "home",
  "end",
  "pageup",
  "pagedown",
  "scrolllock",
  "pause",
  "printscreen",

  // Media keys (supported by tauri-plugin-global-shortcut on desktop platforms)
  "mediaplaypause",
  "medianexttrack",
  "mediaprevtrack",
  "mediaprevioustrack",
  "mediastop",
  "mediaselect",
  "mediarecord",
  "mediafastforward",
  "mediarewind",

  // Volume keys
  "volumeup",
  "volumedown",
  "volumemute",

  // Browser keys
  "browserback",
  "browserforward",
  "browserrefresh",
  "browserstop",
  "browsersearch",
  "browserfavorites",
  "browserhome",

  // Launch keys (common on extended keyboards)
  "launchmail",
  "launchmediaplayer",
  "launchapp1",
  "launchapp2",

  // Display / hardware keys (common on laptops)
  "brightnessup",
  "brightnessdown",
  "keyboardbrightnessup",
  "keyboardbrightnessdown",
  "keyboardbrightnesstoggle",
  "keyboardilluminationup",
  "keyboardilluminationdown",
  "keyboardilluminationtoggle",

  // Misc hardware keys
  "calculator",
  "eject",
  "micmute",
]);

// Keys that are awkward/unreliable to capture from a WebView keyboard event.
// We still let users pick them explicitly.
const SPECIAL_KEY_OPTIONS: Array<{ label: string; value: string }> = [
  // Modifier-only (Windows-only; requires native hook)
  { label: "Modifier (Windows): Right Alt (AltGr)", value: "AltRight" },

  // Media
  { label: "Media: Play/Pause", value: "MediaPlayPause" },
  { label: "Media: Next Track", value: "MediaNextTrack" },
  { label: "Media: Previous Track", value: "MediaPrevTrack" },
  { label: "Media: Stop", value: "MediaStop" },
  { label: "Media: Select", value: "MediaSelect" },
  { label: "Media: Record", value: "MediaRecord" },
  { label: "Media: Fast Forward", value: "MediaFastForward" },
  { label: "Media: Rewind", value: "MediaRewind" },

  // Volume
  { label: "Volume: Mute", value: "VolumeMute" },
  { label: "Volume: Up", value: "VolumeUp" },
  { label: "Volume: Down", value: "VolumeDown" },

  // Browser
  { label: "Browser: Back", value: "BrowserBack" },
  { label: "Browser: Forward", value: "BrowserForward" },
  { label: "Browser: Refresh", value: "BrowserRefresh" },
  { label: "Browser: Stop", value: "BrowserStop" },
  { label: "Browser: Search", value: "BrowserSearch" },
  { label: "Browser: Favorites", value: "BrowserFavorites" },
  { label: "Browser: Home", value: "BrowserHome" },

  // Launch / app keys
  { label: "Launch: Mail", value: "LaunchMail" },
  { label: "Launch: Media Player", value: "LaunchMediaPlayer" },
  { label: "Launch: App 1", value: "LaunchApp1" },
  { label: "Launch: App 2", value: "LaunchApp2" },

  // Display / hardware
  { label: "Display: Brightness Up", value: "BrightnessUp" },
  { label: "Display: Brightness Down", value: "BrightnessDown" },
  { label: "Keyboard: Backlight Up", value: "KeyboardBrightnessUp" },
  { label: "Keyboard: Backlight Down", value: "KeyboardBrightnessDown" },
  { label: "Keyboard: Backlight Toggle", value: "KeyboardBrightnessToggle" },

  // Misc
  { label: "Hardware: Microphone Mute", value: "MicMute" },
  { label: "Hardware: Calculator", value: "Calculator" },
  { label: "Hardware: Eject", value: "Eject" },

  // Lock/power-ish keys (often unsupported as global shortcuts; still selectable)
  { label: "System: Sleep", value: "Sleep" },
  { label: "System: Wake Up", value: "WakeUp" },
  { label: "System: Power", value: "Power" },
];

/**
 * Map from react-hotkeys-hook key names to Tauri shortcut key names.
 * react-hotkeys-hook returns lowercase keys, Tauri expects specific formats.
 */
const KEY_NAME_MAP: Record<string, string> = {
  // Punctuation and special characters
  ".": "Period",
  ",": "Comma",
  "/": "Slash",
  "\\": "Backslash",
  ";": "Semicolon",
  "'": "Quote",
  "[": "BracketLeft",
  "]": "BracketRight",
  "`": "Backquote",
  "-": "Minus",
  "=": "Equal",
  // Named keys (react-hotkeys-hook returns these in lowercase)
  space: "Space",
  backspace: "Backspace",
  tab: "Tab",
  enter: "Enter",
  escape: "Escape",
  delete: "Delete",
  insert: "Insert",
  home: "Home",
  end: "End",
  pageup: "PageUp",
  pagedown: "PageDown",
  // Arrow keys
  arrowup: "ArrowUp",
  arrowdown: "ArrowDown",
  arrowleft: "ArrowLeft",
  arrowright: "ArrowRight",
  up: "ArrowUp",
  down: "ArrowDown",
  left: "ArrowLeft",
  right: "ArrowRight",
  // Function keys
  f1: "F1",
  f2: "F2",
  f3: "F3",
  f4: "F4",
  f5: "F5",
  f6: "F6",
  f7: "F7",
  f8: "F8",
  f9: "F9",
  f10: "F10",
  f11: "F11",
  f12: "F12",
  // Numpad
  numpad0: "Numpad0",
  numpad1: "Numpad1",
  numpad2: "Numpad2",
  numpad3: "Numpad3",
  numpad4: "Numpad4",
  numpad5: "Numpad5",
  numpad6: "Numpad6",
  numpad7: "Numpad7",
  numpad8: "Numpad8",
  numpad9: "Numpad9",
  numpadadd: "NumpadAdd",
  numpadsubtract: "NumpadSubtract",
  numpadmultiply: "NumpadMultiply",
  numpaddivide: "NumpadDivide",
  numpaddecimal: "NumpadDecimal",
  numpadenter: "NumpadEnter",
  // Special named keys that might come through
  backquote: "Backquote",
  period: "Period",
  comma: "Comma",
  slash: "Slash",
  semicolon: "Semicolon",
  quote: "Quote",
  bracketleft: "BracketLeft",
  bracketright: "BracketRight",
  backslash: "Backslash",
  minus: "Minus",
  equal: "Equal",

  // Media keys
  mediaplaypause: "MediaPlayPause",
  medianexttrack: "MediaNextTrack",
  mediaprevtrack: "MediaPrevTrack",
  mediaprevioustrack: "MediaPrevTrack",
  mediastop: "MediaStop",
  mediaselect: "MediaSelect",
  mediarecord: "MediaRecord",
  mediafastforward: "MediaFastForward",
  mediarewind: "MediaRewind",

  // Volume keys
  volumeup: "VolumeUp",
  volumedown: "VolumeDown",
  volumemute: "VolumeMute",

  // Browser keys
  browserback: "BrowserBack",
  browserforward: "BrowserForward",
  browserrefresh: "BrowserRefresh",
  browserstop: "BrowserStop",
  browsersearch: "BrowserSearch",
  browserfavorites: "BrowserFavorites",
  browserhome: "BrowserHome",

  // Launch keys
  launchmail: "LaunchMail",
  launchmediaplayer: "LaunchMediaPlayer",
  launchapp1: "LaunchApp1",
  launchapp2: "LaunchApp2",

  // Display / hardware keys
  brightnessup: "BrightnessUp",
  brightnessdown: "BrightnessDown",
  keyboardbrightnessup: "KeyboardBrightnessUp",
  keyboardbrightnessdown: "KeyboardBrightnessDown",
  keyboardbrightnesstoggle: "KeyboardBrightnessToggle",
  keyboardilluminationup: "KeyboardIlluminationUp",
  keyboardilluminationdown: "KeyboardIlluminationDown",
  keyboardilluminationtoggle: "KeyboardIlluminationToggle",

  // Misc hardware keys
  micmute: "MicMute",
  calculator: "Calculator",
  eject: "Eject",

  // System keys
  sleep: "Sleep",
  wakeup: "WakeUp",
  power: "Power",
};

/**
 * Convert a key from react-hotkeys-hook format to Tauri format
 */
function formatKeyForTauri(key: string): string {
  // Check if we have an explicit mapping
  const mapped = KEY_NAME_MAP[key.toLowerCase()];
  if (mapped) {
    return mapped;
  }

  // For single letters/numbers, uppercase them
  if (key.length === 1) {
    return key.toUpperCase();
  }

  // For other keys, capitalize first letter of each word (e.g., "capslock" -> "CapsLock")
  return key
    .split(/(?=[A-Z])|[-_]/)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1).toLowerCase())
    .join("");
}

/**
 * Convert recorded keys Set to HotkeyConfig
 */
function keysToConfig(keys: Set<string>): HotkeyConfig | null {
  const keysArray = Array.from(keys);
  const modifiers: string[] = [];
  let mainKey: string | null = null;

  for (const key of keysArray) {
    if (MODIFIER_KEYS.has(key.toLowerCase())) {
      modifiers.push(key.toLowerCase());
    } else {
      // The non-modifier key (should only be one)
      mainKey = key;
    }
  }

  // Allow standalone keys (like F3) without modifiers
  if (mainKey !== null && STANDALONE_KEYS.has(mainKey.toLowerCase())) {
    return {
      modifiers,
      key: formatKeyForTauri(mainKey),
    };
  }

  // For other keys, require at least one modifier
  if (modifiers.length === 0 || mainKey === null) {
    return null;
  }

  return {
    modifiers,
    key: formatKeyForTauri(mainKey),
  };
}

/**
 * Format a key for display (e.g., "ctrl" -> "Ctrl", "Space" -> "Space")
 */
function formatKeyForDisplay(key: string): string {
  return key.charAt(0).toUpperCase() + key.slice(1);
}

function hotkeyToDisplayParts(value: HotkeyConfig): string[] {
  return value.modifiers
    .concat([value.key])
    .map((part) => formatKeyForDisplay(part));
}

export function HotkeyInput({
  label,
  description,
  value,
  onChange,
  disabled,
  isRecording: externalIsRecording,
  onStartRecording,
  onStopRecording,
}: HotkeyInputProps) {
  const [keys, { start, stop, isRecording: internalIsRecording }] =
    useRecordHotkeys();

  // Keep the last non-empty key set around so a user can click "Save" after releasing.
  const lastNonEmptyKeysRef = useRef<Set<string>>(new Set());
  const [captureError, setCaptureError] = useState<string | null>(null);
  const [specialKeySelection, setSpecialKeySelection] = useState<string | null>(
    null
  );

  const specialKeyValues = useMemo(
    () => new Set(SPECIAL_KEY_OPTIONS.map((o) => o.value)),
    []
  );

  // Use external state if provided, otherwise use internal
  const isRecording = externalIsRecording ?? internalIsRecording;

  // Keep the dropdown synced with the current value (when it matches a known special key).
  useEffect(() => {
    if (isRecording) return;
    if (!value) {
      setSpecialKeySelection(null);
      return;
    }

    const next =
      value.modifiers.length === 0 && specialKeyValues.has(value.key)
        ? value.key
        : null;
    setSpecialKeySelection(next);
  }, [isRecording, value, specialKeyValues]);

  // Unregister global shortcuts when recording starts, re-register when done.
  // Important: multiple <HotkeyInput/> instances mount in the settings UI.
  // If each one calls registerShortcuts() on mount (or in StrictMode double-invoked
  // effects), Tauri will error with "HotKey already registered".
  const wasRecordingRef = useRef(false);
  useEffect(() => {
    // Transition: not recording -> recording
    if (isRecording && !wasRecordingRef.current) {
      wasRecordingRef.current = true;
      tauriAPI.unregisterShortcuts().catch(console.error);
      return;
    }

    // Transition: recording -> not recording
    if (!isRecording && wasRecordingRef.current) {
      wasRecordingRef.current = false;
      tauriAPI.registerShortcuts().catch(console.error);
    }
  }, [isRecording]);

  // Handle Escape key to cancel recording
  useEffect(() => {
    if (!isRecording) return;

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        stop();
        onStopRecording?.();
      }
    };

    document.addEventListener("keydown", handleEscape);
    return () => document.removeEventListener("keydown", handleEscape);
  }, [isRecording, stop, onStopRecording]);

  // Watch for key changes and update when we have a valid combination
  useEffect(() => {
    if (!isRecording) return;
    if (keys.size === 0) return;

    // Keep a snapshot for manual save.
    lastNonEmptyKeysRef.current = new Set(keys);
    if (captureError) setCaptureError(null);

    // Check if Escape was pressed (handled separately)
    if (keys.has("escape")) {
      return;
    }

    const config = keysToConfig(keys);
    if (config) {
      onChange(config);
      stop();
      onStopRecording?.();
    }
  }, [keys, isRecording, onChange, stop, onStopRecording]);

  // Sync internal recording state with external state
  useEffect(() => {
    if (externalIsRecording === true && !internalIsRecording) {
      start();
    } else if (externalIsRecording === false && internalIsRecording) {
      stop();
    }
  }, [externalIsRecording, internalIsRecording, start, stop]);

  const handleClick = () => {
    if (disabled) return;

    if (isRecording) {
      // Clicking again cancels
      stop();
      onStopRecording?.();
    } else {
      setCaptureError(null);
      setSpecialKeySelection(null);
      start();
      onStartRecording?.();
    }
  };

  const handleCancelRecording = () => {
    stop();
    onStopRecording?.();
  };

  const handleSaveRecording = () => {
    const snapshot =
      lastNonEmptyKeysRef.current.size > 0 ? lastNonEmptyKeysRef.current : keys;

    // Ignore Escape-only snapshots.
    const filtered = new Set(
      Array.from(snapshot).filter((k) => k !== "escape")
    );
    if (filtered.size === 0) {
      setCaptureError("No key captured yet.");
      return;
    }

    const config = keysToConfig(filtered);
    if (!config) {
      setCaptureError(
        "That key can't be used as a hotkey by itself. Try adding Ctrl/Alt/Shift, or pick a special key below."
      );
      return;
    }

    onChange(config);
    stop();
    onStopRecording?.();
  };

  const handlePickSpecialKey = (value: string | null) => {
    setSpecialKeySelection(value);
    if (!value) return;

    // Special keys are intended to be used alone.
    onChange({ modifiers: [], key: value });

    // If we're currently recording, stop cleanly.
    if (isRecording) {
      stop();
      onStopRecording?.();
    }
  };

  // Build live preview of current keys being pressed
  const livePreview = Array.from(keys)
    .filter((k) => k !== "escape")
    .map((k) => formatKeyForDisplay(k));

  const displayParts = value ? hotkeyToDisplayParts(value) : null;

  return (
    <div className="hotkey-field">
      <p className="settings-label">{label}</p>
      {description && <p className="settings-description">{description}</p>}

      <div
        role="button"
        tabIndex={disabled ? -1 : 0}
        aria-disabled={disabled ? true : undefined}
        onClick={handleClick}
        onKeyDown={(e) => {
          if (disabled) return;
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            handleClick();
          }
        }}
        className={`hotkey-display ${isRecording ? "capturing" : ""}`}
        style={{
          width: "100%",
          marginTop: 8,
          cursor: disabled ? "not-allowed" : "pointer",
          opacity: disabled ? 0.5 : 1,
        }}
      >
        <div className="hotkey-display-left">
          {isRecording ? (
            livePreview.length > 0 ? (
              livePreview.map((part) => <Kbd key={part}>{part}</Kbd>)
            ) : (
              <span className="hotkey-capturing-hint">
                Press a key or combination…
              </span>
            )
          ) : displayParts ? (
            displayParts.map((part) => <Kbd key={part}>{part}</Kbd>)
          ) : (
            <Kbd className="hotkey-placeholder">Unassigned</Kbd>
          )}
        </div>

        <div className="hotkey-actions">
          {isRecording ? (
            <>
              <span className="hotkey-hint">Esc cancels</span>
              <Button
                size="xs"
                variant="filled"
                onClick={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  handleSaveRecording();
                }}
              >
                Save
              </Button>
              <Button
                size="xs"
                variant="subtle"
                color="gray"
                onClick={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  handleCancelRecording();
                }}
              >
                Cancel
              </Button>
            </>
          ) : (
            <>
              <span className="hotkey-hint">
                {displayParts ? "Click to change" : "Click to set"}
              </span>
              {value && (
                <button
                  type="button"
                  disabled={disabled}
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    onChange(null);
                  }}
                  className="hotkey-clear"
                >
                  Clear
                </button>
              )}
            </>
          )}
        </div>
      </div>

      {captureError && <div className="hotkey-error">{captureError}</div>}

      <div
        className="hotkey-special-row"
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="hotkey-special-meta">
          <div className="hotkey-special-label">Special key</div>
        </div>

        <Select
          placeholder="None"
          data={SPECIAL_KEY_OPTIONS}
          value={specialKeySelection}
          onChange={handlePickSpecialKey}
          searchable
          clearable
          size="xs"
          disabled={disabled}
          w={260}
        />
      </div>
    </div>
  );
}
