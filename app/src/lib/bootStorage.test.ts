import { afterEach, describe, expect, it, vi } from "vitest";
import {
	LOCAL_ACCENT_COLOR_KEY,
	LOCAL_SETTINGS_GUIDE_STATE_KEY,
	readBootAccentColor,
	readBootGuideState,
	safeLocalStorageRemoveItem,
	safeLocalStorageSetItem,
	setBootGuideState,
} from "./bootStorage";

function createFakeLocalStorage(initial?: Record<string, string>) {
	const store = new Map<string, string>(
		Object.entries(initial ?? {}) as Array<[string, string]>,
	);

	return {
		getItem: (key: string) => store.get(key) ?? null,
		setItem: (key: string, value: string) => {
			store.set(key, value);
		},
		removeItem: (key: string) => {
			store.delete(key);
		},
	};
}

afterEach(() => {
	vi.unstubAllGlobals();
});

describe("bootStorage", () => {
	describe("readBootGuideState", () => {
		it("returns null when window is missing", () => {
			expect(readBootGuideState()).toBeNull();
		});

		it("returns null when the stored value is invalid", () => {
			vi.stubGlobal("window", {
				localStorage: createFakeLocalStorage({
					[LOCAL_SETTINGS_GUIDE_STATE_KEY]: "wat",
				}),
			});

			expect(readBootGuideState()).toBeNull();
		});

		it("returns the stored guide state when valid", () => {
			vi.stubGlobal("window", {
				localStorage: createFakeLocalStorage({
					[LOCAL_SETTINGS_GUIDE_STATE_KEY]: "pending",
				}),
			});

			expect(readBootGuideState()).toBe("pending");
		});

		it("can set the guide state safely", () => {
			const localStorage = createFakeLocalStorage();
			vi.stubGlobal("window", { localStorage });

			setBootGuideState("completed");
			expect(localStorage.getItem(LOCAL_SETTINGS_GUIDE_STATE_KEY)).toBe(
				"completed",
			);
		});
	});

	describe("readBootAccentColor", () => {
		it("returns null when window is missing", () => {
			expect(readBootAccentColor()).toBeNull();
		});

		it("returns null when the stored value is not a hex color", () => {
			vi.stubGlobal("window", {
				localStorage: createFakeLocalStorage({
					[LOCAL_ACCENT_COLOR_KEY]: "red",
				}),
			});

			expect(readBootAccentColor()).toBeNull();
		});

		it("returns the stored accent color when valid", () => {
			vi.stubGlobal("window", {
				localStorage: createFakeLocalStorage({
					[LOCAL_ACCENT_COLOR_KEY]: "#a1B2c3",
				}),
			});

			expect(readBootAccentColor()).toBe("#a1B2c3");
		});
	});

	describe("safeLocalStorageSetItem/removeItem", () => {
		it("does not throw when window is missing", () => {
			expect(() => safeLocalStorageSetItem("k", "v")).not.toThrow();
			expect(() => safeLocalStorageRemoveItem("k")).not.toThrow();
		});
	});
});
