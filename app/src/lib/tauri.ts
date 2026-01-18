import { tauriAPI as tauriCommandsAPI } from "./tauri/commands";
import { tauriSettingsAPI } from "./tauri/settings";

export {
	createHotkeyDuplicateSchema,
	type HotkeyConfig,
	HotkeyConfigSchema,
	hotkeyIsSameAs,
	validateHotkeyNotDuplicate,
} from "./hotkeys";
export {
	audioSettingsTestAPI,
	backupAPI,
	configAPI,
	dataAPI,
	llmAPI,
	logsAPI,
	recordingsAPI,
	sttAPI,
} from "./tauri/commands";
export {
	defaultHoldHotkey,
	defaultPasteLastHotkey,
	defaultQuickAskHoldHotkey,
	defaultQuickAskToggleHotkey,
	defaultRetryHotkey,
	defaultToggleHotkey,
	tauriSettingsAPI,
} from "./tauri/settings";
export * from "./tauri/types";

export const tauriAPI = {
	...tauriCommandsAPI,
	...tauriSettingsAPI,
};
