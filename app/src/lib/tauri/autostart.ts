import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";

// The OS registration is the source of truth. Do not sync this machine-local
// choice through settings.json, account sync, or per-program profiles.
export const autostartAPI = {
	getEnabled: isEnabled,
	async setEnabled(enabled: boolean): Promise<boolean> {
		if (enabled) await enable();
		else await disable();
		return isEnabled();
	},
};
