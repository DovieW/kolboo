import { Button, Kbd, Loader, Select } from "@mantine/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useRecordHotkeys } from "react-hotkeys-hook";
import type { HotkeyConfig } from "../lib/tauri";
import { tauriAPI } from "../lib/tauri";

interface HotkeyInputProps {
	label: string;
	description?: string;
	value: HotkeyConfig | null;
	onChange: (config: HotkeyConfig | null) => void;
	disabled?: boolean;
	isSaving?: boolean;
	// Coordinated recording state (managed by parent)
	isRecording?: boolean;
	onStartRecording?: () => void;
	onStopRecording?: () => void;
}

// Known modifier keys (lowercase, as returned by react-hotkeys-hook)
const MODIFIER_KEYS = new Set(["ctrl", "alt", "shift", "meta", "mod"]);

const IS_WINDOWS =
	typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent);

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
	"capslock",
	"scrolllock",
	"numlock",
	"pause",
	"printscreen",

	// Function keys beyond F12 (often not emitted by WebViews)
	"f13",
	"f14",
	"f15",
	"f16",
	"f17",
	"f18",
	"f19",
	"f20",
	"f21",
	"f22",
	"f23",
	"f24",

	// Media keys (supported by tauri-plugin-global-shortcut on desktop platforms)
	"mediaplaypause",
	"medianexttrack",
	"mediaprevtrack",
	"mediaprevioustrack",
	"mediatracknext",
	"mediatrackprevious",
	"mediastop",
	"mediaplay",
	"mediapause",

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

	// Windows Copilot key (when emitted as a distinct key; otherwise captured via special dropdown)
	"copilot",
]);

// Keys that are awkward/unreliable to capture from a WebView keyboard event.
// We still let users pick them explicitly.
const SPECIAL_KEY_OPTIONS: Array<{
	label: string;
	value: string;
	disabled?: boolean;
}> = [
	// Modifier-only (Windows-only; requires native hook)
	{ label: "Modifier (Windows): Right Alt (AltGr)", value: "AltRight" },

	// Windows-only: Copilot key (implemented via native hook; typically maps to the Win+C system shortcut)
	{
		label: "System (Windows): Copilot key",
		value: "Copilot",
		disabled: !IS_WINDOWS,
	},

	// Common non-printable keys
	{ label: "System: Caps Lock", value: "CapsLock" },
	{ label: "System: Num Lock", value: "NumLock" },
	{ label: "System: Scroll Lock", value: "ScrollLock" },
	{ label: "System: Print Screen", value: "PrintScreen" },
	{ label: "System: Pause / Break", value: "Pause" },
	{ label: "Navigation: Insert", value: "Insert" },
	{ label: "Navigation: Delete", value: "Delete" },
	{ label: "Navigation: Home", value: "Home" },
	{ label: "Navigation: End", value: "End" },
	{ label: "Navigation: Page Up", value: "PageUp" },
	{ label: "Navigation: Page Down", value: "PageDown" },

	// Function keys (often not emitted by WebViews)
	{ label: "Function: F13", value: "F13" },
	{ label: "Function: F14", value: "F14" },
	{ label: "Function: F15", value: "F15" },
	{ label: "Function: F16", value: "F16" },
	{ label: "Function: F17", value: "F17" },
	{ label: "Function: F18", value: "F18" },
	{ label: "Function: F19", value: "F19" },
	{ label: "Function: F20", value: "F20" },
	{ label: "Function: F21", value: "F21" },
	{ label: "Function: F22", value: "F22" },
	{ label: "Function: F23", value: "F23" },
	{ label: "Function: F24", value: "F24" },

	// Media
	{ label: "Media: Play/Pause", value: "MediaPlayPause" },
	{ label: "Media: Play", value: "MediaPlay" },
	{ label: "Media: Pause", value: "MediaPause" },
	{ label: "Media: Next Track", value: "MediaTrackNext" },
	{ label: "Media: Previous Track", value: "MediaTrackPrevious" },
	{ label: "Media: Stop", value: "MediaStop" },
	// These exist on some keyboards but are not supported by the current
	// global-hotkey string parser used by tauri-plugin-global-shortcut.
	{
		label: "Media: Select (unsupported)",
		value: "MediaSelect",
		disabled: true,
	},
	{
		label: "Media: Record (unsupported)",
		value: "MediaRecord",
		disabled: true,
	},
	{
		label: "Media: Fast Forward (unsupported)",
		value: "MediaFastForward",
		disabled: true,
	},
	{
		label: "Media: Rewind (unsupported)",
		value: "MediaRewind",
		disabled: true,
	},

	// Volume
	{ label: "Volume: Mute", value: "VolumeMute" },
	{ label: "Volume: Up", value: "VolumeUp" },
	{ label: "Volume: Down", value: "VolumeDown" },

	// Browser
	{
		label: "Browser: Back (unsupported)",
		value: "BrowserBack",
		disabled: true,
	},
	{
		label: "Browser: Forward (unsupported)",
		value: "BrowserForward",
		disabled: true,
	},
	{
		label: "Browser: Refresh (unsupported)",
		value: "BrowserRefresh",
		disabled: true,
	},
	{
		label: "Browser: Stop (unsupported)",
		value: "BrowserStop",
		disabled: true,
	},
	{
		label: "Browser: Search (unsupported)",
		value: "BrowserSearch",
		disabled: true,
	},
	{
		label: "Browser: Favorites (unsupported)",
		value: "BrowserFavorites",
		disabled: true,
	},
	{
		label: "Browser: Home (unsupported)",
		value: "BrowserHome",
		disabled: true,
	},

	// Launch / app keys
	{ label: "Launch: Mail (unsupported)", value: "LaunchMail", disabled: true },
	{
		label: "Launch: Media Player (unsupported)",
		value: "LaunchMediaPlayer",
		disabled: true,
	},
	{ label: "Launch: App 1 (unsupported)", value: "LaunchApp1", disabled: true },
	{ label: "Launch: App 2 (unsupported)", value: "LaunchApp2", disabled: true },

	// Display / hardware
	{
		label: "Display: Brightness Up (unsupported)",
		value: "BrightnessUp",
		disabled: true,
	},
	{
		label: "Display: Brightness Down (unsupported)",
		value: "BrightnessDown",
		disabled: true,
	},
	{
		label: "Keyboard: Backlight Up (unsupported)",
		value: "KeyboardBrightnessUp",
		disabled: true,
	},
	{
		label: "Keyboard: Backlight Down (unsupported)",
		value: "KeyboardBrightnessDown",
		disabled: true,
	},
	{
		label: "Keyboard: Backlight Toggle (unsupported)",
		value: "KeyboardBrightnessToggle",
		disabled: true,
	},

	// Misc
	{ label: "Hardware: Microphone Mute", value: "MicMute" },
	{ label: "Hardware: Calculator", value: "Calculator" },
	{ label: "Hardware: Eject", value: "Eject" },

	// Lock/power-ish keys (often unsupported as global shortcuts; still selectable)
	{ label: "System: Sleep (unsupported)", value: "Sleep", disabled: true },
	{ label: "System: Wake Up (unsupported)", value: "WakeUp", disabled: true },
	{ label: "System: Power (unsupported)", value: "Power", disabled: true },

	// Context Menu / Application key (requested, but not supported by global-hotkey parser)
	{
		label: "Keyboard: Context Menu (unsupported)",
		value: "ContextMenu",
		disabled: true,
	},
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
	capslock: "CapsLock",
	numlock: "NumLock",
	scrolllock: "ScrollLock",
	printscreen: "PrintScreen",
	pause: "Pause",
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
	f13: "F13",
	f14: "F14",
	f15: "F15",
	f16: "F16",
	f17: "F17",
	f18: "F18",
	f19: "F19",
	f20: "F20",
	f21: "F21",
	f22: "F22",
	f23: "F23",
	f24: "F24",
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
	mediaplay: "MediaPlay",
	mediapause: "MediaPause",
	medianexttrack: "MediaTrackNext",
	mediatracknext: "MediaTrackNext",
	mediaprevtrack: "MediaTrackPrevious",
	mediaprevioustrack: "MediaTrackPrevious",
	mediatrackprevious: "MediaTrackPrevious",
	mediastop: "MediaStop",
	// NOTE: Some media keys exist on keyboards but are not supported by the current
	// global-hotkey parser; we still map them for display purposes.
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

	// Windows Copilot key (if it ever comes through via WebView)
	copilot: "Copilot",
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

function hotkeyConfigEquals(
	a: HotkeyConfig | null | undefined,
	b: HotkeyConfig | null | undefined,
): boolean {
	if (a === b) return true;
	if (!a || !b) return false;
	if (a.key !== b.key) return false;
	if (a.modifiers.length !== b.modifiers.length) return false;

	// Modifiers may be serialized in different orders; treat them as a set.
	const aMods = [...a.modifiers].map((m) => m.toLowerCase()).sort();
	const bMods = [...b.modifiers].map((m) => m.toLowerCase()).sort();
	return aMods.every((m, i) => m === bMods[i]);
}

export function HotkeyInput({
	label,
	description,
	value,
	onChange,
	disabled,
	isSaving,
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
		null,
	);

	// While a hotkey update is being persisted to settings.json, keep an optimistic
	// value around so the UI doesn't briefly flash back to "Unassigned".
	const [optimisticValue, setOptimisticValue] = useState<
		HotkeyConfig | null | undefined
	>(undefined);

	const specialKeyValues = useMemo(
		() => new Set(SPECIAL_KEY_OPTIONS.map((o) => o.value)),
		[],
	);

	// Use external state if provided, otherwise use internal
	const isRecording = externalIsRecording ?? internalIsRecording;

	// Clear optimistic state once the external value catches up.
	useEffect(() => {
		if (optimisticValue === undefined) return;
		if (hotkeyConfigEquals(value, optimisticValue)) {
			setOptimisticValue(undefined);
		}
	}, [value, optimisticValue]);

	const emitChange = useCallback(
		(next: HotkeyConfig | null) => {
			setOptimisticValue(next);
			onChange(next);
		},
		[onChange],
	);

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
			emitChange(config);
			stop();
			onStopRecording?.();
		}
	}, [keys, isRecording, captureError, emitChange, stop, onStopRecording]);

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
			Array.from(snapshot).filter((k) => k !== "escape"),
		);
		if (filtered.size === 0) {
			setCaptureError("No key captured yet.");
			return;
		}

		const config = keysToConfig(filtered);
		if (!config) {
			setCaptureError(
				"That key can't be used as a hotkey by itself. Try adding Ctrl/Alt/Shift, or pick a special key below.",
			);
			return;
		}

		emitChange(config);
		stop();
		onStopRecording?.();
	};

	const handlePickSpecialKey = (value: string | null) => {
		setSpecialKeySelection(value);
		if (!value) return;

		// Special keys are intended to be used alone.
		emitChange({ modifiers: [], key: value });

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

	const effectiveValue =
		optimisticValue !== undefined ? optimisticValue : value;
	const displayParts = effectiveValue
		? hotkeyToDisplayParts(effectiveValue)
		: null;

	return (
		<div className="hotkey-field">
			<p className="settings-label">{label}</p>
			{description && <p className="settings-description">{description}</p>}

			{/* biome-ignore lint/a11y/useSemanticElements: cannot use a real <button> here because this container includes nested interactive controls (Save/Cancel/etc.) */}
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
								{isSaving ? (
									<span
										style={{
											display: "inline-flex",
											alignItems: "center",
											gap: 6,
										}}
									>
										<Loader size="xs" color="gray" />
										Saving…
									</span>
								) : displayParts ? (
									"Click to change"
								) : (
									"Click to set"
								)}
							</span>
							{effectiveValue && (
								<button
									type="button"
									disabled={disabled}
									onClick={(e) => {
										e.preventDefault();
										e.stopPropagation();
										emitChange(null);
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

			{/* biome-ignore lint/a11y/noStaticElementInteractions: wrapper only prevents click-through while interacting with the Select */}
			<div
				className="hotkey-special-row"
				onMouseDown={(e) => e.stopPropagation()}
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

			{(specialKeySelection === "AltRight" ||
				effectiveValue?.key === "AltRight") && (
				<div style={{ marginTop: 6 }}>
					<span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
						Note: Right Alt is AltGr on many keyboard layouts. It can be
						unreliable on some Windows setups and may interfere with typing
						special characters. If it doesn’t work well, consider using a key
						like F3 or Ctrl+Space.
					</span>
				</div>
			)}

			{(specialKeySelection === "Copilot" ||
				effectiveValue?.key === "Copilot") && (
				<div style={{ marginTop: 6 }}>
					<span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
						Note: On Windows, the Copilot key is typically treated as the system
						shortcut Win+C. If you bind it here, Kolboo will intercept that
						shortcut so Windows Copilot doesn’t pop up. This may also override
						Win+C when pressed normally.
					</span>
				</div>
			)}
		</div>
	);
}
