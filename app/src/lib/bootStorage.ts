export const LOCAL_ACCENT_COLOR_KEY = "tv_accent_color";
export const LOCAL_SETTINGS_GUIDE_STATE_KEY = "tv_settings_guide_state";

export type SettingsGuideState = "pending" | "skipped" | "completed";

function getLocalStorage(): Storage | null {
	try {
		if (typeof window === "undefined") return null;
		if (!window.localStorage) return null;
		return window.localStorage;
	} catch {
		return null;
	}
}

export function safeLocalStorageGetItem(key: string): string | null {
	try {
		return getLocalStorage()?.getItem(key) ?? null;
	} catch {
		return null;
	}
}

export function safeLocalStorageSetItem(key: string, value: string): void {
	try {
		getLocalStorage()?.setItem(key, value);
	} catch {
		// ignore
	}
}

export function safeLocalStorageRemoveItem(key: string): void {
	try {
		getLocalStorage()?.removeItem(key);
	} catch {
		// ignore
	}
}

export function readBootGuideState(): SettingsGuideState | null {
	const raw = safeLocalStorageGetItem(LOCAL_SETTINGS_GUIDE_STATE_KEY);
	if (raw === "pending" || raw === "skipped" || raw === "completed") return raw;
	return null;
}

export function setBootGuideState(state: SettingsGuideState): void {
	safeLocalStorageSetItem(LOCAL_SETTINGS_GUIDE_STATE_KEY, state);
}

export function readBootAccentColor(): string | null {
	const raw = safeLocalStorageGetItem(LOCAL_ACCENT_COLOR_KEY);
	if (typeof raw !== "string") return null;
	if (/^#([0-9a-fA-F]{6})$/.test(raw)) return raw;
	return null;
}
