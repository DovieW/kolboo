import { describe, expect, it, vi } from "vitest";
import {
	type HotkeySettingsLike,
	updateHotkeyAndReregisterShortcuts,
	updateHotkeyShortcutCardWithValidation,
	validateHotkeyNotDuplicateInCards,
} from "./hotkeyMutations";
import type { HotkeyShortcutCard } from "./hotkeys";

function createSettings(
	overrides?: Partial<HotkeySettingsLike>,
): HotkeySettingsLike {
	return {
		toggle_hotkey: { modifiers: ["ctrl", "alt"], key: "Space" },
		hold_hotkey: { modifiers: ["ctrl", "alt"], key: "Backquote" },
		paste_last_hotkey: { modifiers: ["ctrl", "alt"], key: "Period" },
		retry_hotkey: { modifiers: ["ctrl", "alt"], key: "KeyR" },
		quick_ask_hold_hotkey: { modifiers: ["ctrl", "alt"], key: "KeyQ" },
		quick_ask_toggle_hotkey: { modifiers: ["ctrl", "alt"], key: "KeyW" },
		...overrides,
	};
}

describe("updateHotkeyAndReregisterShortcuts", () => {
	it("updates, unregisters, and re-registers shortcuts", async () => {
		const settings = createSettings();

		const updateHotkey = vi.fn(async () => {});
		const unregisterShortcuts = vi.fn(async () => {});
		const registerShortcuts = vi.fn(async () => {});

		await updateHotkeyAndReregisterShortcuts({
			kind: "toggle",
			nextHotkey: { modifiers: ["ctrl"], key: "A" },
			getSettings: async () => settings,
			getPreviousHotkey: (s) => s.toggle_hotkey,
			updateHotkey,
			unregisterShortcuts,
			registerShortcuts,
		});

		expect(updateHotkey).toHaveBeenCalledTimes(1);
		expect(updateHotkey).toHaveBeenCalledWith({
			modifiers: ["ctrl"],
			key: "A",
		});
		expect(unregisterShortcuts).toHaveBeenCalledTimes(1);
		expect(registerShortcuts).toHaveBeenCalledTimes(1);
	});

	it("throws before saving when the hotkey is a duplicate", async () => {
		const settings = createSettings();

		const updateHotkey = vi.fn(async () => {});
		const unregisterShortcuts = vi.fn(async () => {});
		const registerShortcuts = vi.fn(async () => {});

		await expect(
			updateHotkeyAndReregisterShortcuts({
				kind: "toggle",
				nextHotkey: { modifiers: ["ctrl", "alt"], key: "Backquote" },
				getSettings: async () => settings,
				getPreviousHotkey: (s) => s.toggle_hotkey,
				updateHotkey,
				unregisterShortcuts,
				registerShortcuts,
			}),
		).rejects.toThrow("This shortcut is already used for the hold hotkey");

		expect(updateHotkey).not.toHaveBeenCalled();
		expect(unregisterShortcuts).not.toHaveBeenCalled();
		expect(registerShortcuts).not.toHaveBeenCalled();
	});

	it("reverts the setting if registering shortcuts fails", async () => {
		const settings = createSettings();
		const registerError = new Error("register failed");

		const updateHotkey = vi.fn(async () => {});
		const unregisterShortcuts = vi.fn(async () => {});
		const registerShortcuts = vi
			.fn<() => Promise<void>>()
			.mockRejectedValueOnce(registerError)
			.mockResolvedValueOnce(undefined);

		await expect(
			updateHotkeyAndReregisterShortcuts({
				kind: "toggle",
				nextHotkey: { modifiers: ["ctrl"], key: "A" },
				getSettings: async () => settings,
				getPreviousHotkey: (s) => s.toggle_hotkey,
				updateHotkey,
				unregisterShortcuts,
				registerShortcuts,
			}),
		).rejects.toThrow("register failed");

		// First attempt updates to next hotkey, then restore updates back to previous.
		expect(updateHotkey).toHaveBeenCalledTimes(2);
		expect(updateHotkey).toHaveBeenNthCalledWith(1, {
			modifiers: ["ctrl"],
			key: "A",
		});
		expect(updateHotkey).toHaveBeenNthCalledWith(2, settings.toggle_hotkey);

		// Unregister called once on initial attempt and once during restore.
		expect(unregisterShortcuts).toHaveBeenCalledTimes(2);
		// Register called once (fails) and once (restore).
		expect(registerShortcuts).toHaveBeenCalledTimes(2);
	});

	it("logs restore error but still rethrows the original register error", async () => {
		const settings = createSettings();
		const registerError = new Error("register failed");
		const restoreError = new Error("restore failed");

		const updateHotkey = vi.fn(async () => {});
		const unregisterShortcuts = vi.fn(async () => {});
		const registerShortcuts = vi.fn(async () => {
			throw registerError;
		});
		const logRestoreError = vi.fn();

		// Make the restore updateHotkey throw.
		updateHotkey
			.mockResolvedValueOnce(undefined) // update to next
			.mockRejectedValueOnce(restoreError); // restore to previous

		await expect(
			updateHotkeyAndReregisterShortcuts({
				kind: "toggle",
				nextHotkey: { modifiers: ["ctrl"], key: "A" },
				getSettings: async () => settings,
				getPreviousHotkey: (s) => s.toggle_hotkey,
				updateHotkey,
				unregisterShortcuts,
				registerShortcuts,
				logRestoreError,
			}),
		).rejects.toThrow("register failed");

		expect(logRestoreError).toHaveBeenCalledTimes(1);
		expect(logRestoreError).toHaveBeenCalledWith(restoreError);
	});
});

describe("validateHotkeyNotDuplicateInCards", () => {
	it("returns null when no duplicate exists", () => {
		const cards: HotkeyShortcutCard[] = [
			{
				id: "a",
				type: "toggle",
				hotkey: { modifiers: ["ctrl"], key: "A" },
			},
			{
				id: "b",
				type: "hold",
				hotkey: { modifiers: ["ctrl"], key: "B" },
			},
		];

		const error = validateHotkeyNotDuplicateInCards(
			{ modifiers: ["ctrl"], key: "C" },
			cards,
			"a",
		);

		expect(error).toBeNull();
	});

	it("returns an error when a duplicate exists", () => {
		const cards: HotkeyShortcutCard[] = [
			{
				id: "a",
				type: "toggle",
				hotkey: { modifiers: ["ctrl"], key: "A" },
			},
			{
				id: "b",
				type: "hold",
				hotkey: { modifiers: ["ctrl"], key: "B" },
			},
		];

		const error = validateHotkeyNotDuplicateInCards(
			{ modifiers: ["ctrl"], key: "B" },
			cards,
			"a",
		);

		expect(error).toBe("This shortcut is already used by another card.");
	});
});

describe("updateHotkeyShortcutCardWithValidation", () => {
	it("updates card when hotkey is unique", async () => {
		const cards: HotkeyShortcutCard[] = [
			{
				id: "a",
				type: "toggle",
				hotkey: { modifiers: ["ctrl"], key: "A" },
			},
			{
				id: "b",
				type: "hold",
				hotkey: { modifiers: ["ctrl"], key: "B" },
			},
		];

		const updateCard = vi.fn(async () => {});

		await updateHotkeyShortcutCardWithValidation({
			cardId: "a",
			nextHotkey: { modifiers: ["ctrl"], key: "C" },
			cards,
			updateCard,
		});

		expect(updateCard).toHaveBeenCalledTimes(1);
		expect(updateCard).toHaveBeenCalledWith("a", {
			modifiers: ["ctrl"],
			key: "C",
		});
	});

	it("throws when hotkey is a duplicate", async () => {
		const cards: HotkeyShortcutCard[] = [
			{
				id: "a",
				type: "toggle",
				hotkey: { modifiers: ["ctrl"], key: "A" },
			},
			{
				id: "b",
				type: "hold",
				hotkey: { modifiers: ["ctrl"], key: "B" },
			},
		];

		const updateCard = vi.fn(async () => {});

		await expect(
			updateHotkeyShortcutCardWithValidation({
				cardId: "a",
				nextHotkey: { modifiers: ["ctrl"], key: "B" },
				cards,
				updateCard,
			}),
		).rejects.toThrow("This shortcut is already used by another card.");

		expect(updateCard).not.toHaveBeenCalled();
	});
});
