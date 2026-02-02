import { z } from "zod";

export interface HotkeyConfig {
	modifiers: string[];
	key: string;
}

// Zod schema for HotkeyConfig validation
export const HotkeyConfigSchema = z.object({
	modifiers: z.array(z.string()),
	key: z.string().min(1, "Key is required"),
});

export function normalizeHotkeyConfig(
	value: unknown,
	fallback: HotkeyConfig | null,
): HotkeyConfig | null {
	// Explicit null means "disabled".
	if (value === null) return null;

	// Missing/invalid means fallback to default.
	const result = HotkeyConfigSchema.safeParse(value);
	return result.success ? result.data : fallback;
}

export function hotkeyIsSameAs(a: HotkeyConfig, b: HotkeyConfig): boolean {
	if (a.key.toLowerCase() !== b.key.toLowerCase()) return false;
	if (a.modifiers.length !== b.modifiers.length) return false;
	return a.modifiers.every((mod) =>
		b.modifiers.some((other) => mod.toLowerCase() === other.toLowerCase()),
	);
}

export type HotkeyType =
	| "toggle"
	| "hold"
	| "paste_last"
	| "retry"
	| "quick_ask_hold"
	| "quick_ask_toggle";

const HOTKEY_LABELS: Record<HotkeyType, string> = {
	toggle: "toggle",
	hold: "hold",
	paste_last: "paste last",
	retry: "retry",
	quick_ask_hold: "Quick Ask hold",
	quick_ask_toggle: "Quick Ask toggle",
};

export interface HotkeyShortcutCard {
	id: string;
	type: HotkeyType;
	hotkey: HotkeyConfig | null;
}

export const HotkeyShortcutCardSchema = z.object({
	id: z.string().min(1),
	type: z.enum([
		"toggle",
		"hold",
		"paste_last",
		"retry",
		"quick_ask_hold",
		"quick_ask_toggle",
	]),
	hotkey: HotkeyConfigSchema.nullable(),
});

export const HotkeyShortcutCardsSchema = z.array(HotkeyShortcutCardSchema);

export function createHotkeyShortcutId(): string {
	if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
		return crypto.randomUUID();
	}

	const randomPart = Math.random().toString(36).slice(2, 10);
	return `hotkey-${Date.now().toString(36)}-${randomPart}`;
}

/**
 * Create a Zod schema for validating a hotkey doesn't conflict with existing hotkeys
 */
export function createHotkeyDuplicateSchema(
	allHotkeys: Record<HotkeyType, HotkeyConfig | null>,
	excludeType: HotkeyType,
) {
	return HotkeyConfigSchema.superRefine((hotkey, ctx) => {
		for (const [type, existing] of Object.entries(allHotkeys)) {
			if (type === excludeType) continue;
			if (!existing) continue;

			if (hotkeyIsSameAs(hotkey, existing)) {
				ctx.addIssue({
					code: "custom",
					message: `This shortcut is already used for the ${
						HOTKEY_LABELS[type as HotkeyType]
					} hotkey`,
				});
				return;
			}
		}
	});
}

/**
 * Validate that a hotkey doesn't conflict with other hotkeys
 * Returns error message if invalid, null if valid
 */
export function validateHotkeyNotDuplicate(
	newHotkey: HotkeyConfig | null,
	allHotkeys: {
		toggle: HotkeyConfig | null;
		hold: HotkeyConfig | null;
		paste_last: HotkeyConfig | null;
		retry: HotkeyConfig | null;
		quick_ask_hold: HotkeyConfig | null;
		quick_ask_toggle: HotkeyConfig | null;
	},
	excludeType: HotkeyType,
): string | null {
	if (!newHotkey) return null;
	const schema = createHotkeyDuplicateSchema(allHotkeys, excludeType);
	const result = schema.safeParse(newHotkey);
	if (!result.success) {
		return result.error.issues[0]?.message ?? "Invalid hotkey";
	}
	return null;
}
