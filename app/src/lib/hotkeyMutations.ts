import { type HotkeyConfig, validateHotkeyNotDuplicate } from "./hotkeys";

export type HotkeyKind =
	| "toggle"
	| "hold"
	| "paste_last"
	| "retry"
	| "quick_ask_hold"
	| "quick_ask_toggle";

export interface HotkeySettingsLike {
	toggle_hotkey: HotkeyConfig | null;
	hold_hotkey: HotkeyConfig | null;
	paste_last_hotkey: HotkeyConfig | null;
	retry_hotkey: HotkeyConfig | null;
	quick_ask_hold_hotkey: HotkeyConfig | null;
	quick_ask_toggle_hotkey: HotkeyConfig | null;
}

function getAllHotkeys(settings: HotkeySettingsLike) {
	return {
		toggle: settings.toggle_hotkey,
		hold: settings.hold_hotkey,
		paste_last: settings.paste_last_hotkey,
		retry: settings.retry_hotkey,
		quick_ask_hold: settings.quick_ask_hold_hotkey,
		quick_ask_toggle: settings.quick_ask_toggle_hotkey,
	};
}

export async function updateHotkeyAndReregisterShortcuts<
	TSettings extends HotkeySettingsLike,
>(params: {
	kind: HotkeyKind;
	nextHotkey: HotkeyConfig | null;
	getSettings: () => Promise<TSettings>;
	getPreviousHotkey: (settings: TSettings) => HotkeyConfig | null;
	updateHotkey: (hotkey: HotkeyConfig | null) => Promise<void>;
	unregisterShortcuts: () => Promise<void>;
	registerShortcuts: () => Promise<void>;
	logRestoreError?: (error: unknown) => void;
}): Promise<void> {
	const settings = await params.getSettings();
	const previous = params.getPreviousHotkey(settings);

	// Validate no duplicate (unless unsetting)
	if (params.nextHotkey) {
		const error = validateHotkeyNotDuplicate(
			params.nextHotkey,
			getAllHotkeys(settings),
			params.kind,
		);
		if (error) throw new Error(error);
	}

	await params.updateHotkey(params.nextHotkey);
	await params.unregisterShortcuts();

	try {
		await params.registerShortcuts();
	} catch (error) {
		// Defensive: don't leave the user with no registered shortcuts.
		// Revert setting and restore previous registrations.
		try {
			await params.updateHotkey(previous);
			await params.unregisterShortcuts();
			await params.registerShortcuts();
		} catch (restoreError) {
			params.logRestoreError?.(restoreError);
		}
		throw error;
	}
}
