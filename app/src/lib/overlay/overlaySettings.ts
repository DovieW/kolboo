import type { InvokeArgs, InvokeOptions } from "@tauri-apps/api/core";
import type { QueryClient } from "@tanstack/react-query";
import type { SettingsChangedPayload } from "../tauri";

type OverlaySettingsDeps = {
	applyAccentColor: (color: string | null) => void;
	reloadSettingsFromDisk: () => Promise<void>;
	queryClient: QueryClient;
	invoke: <T>(
		command: string,
		args?: InvokeArgs,
		options?: InvokeOptions,
	) => Promise<T>;
};

export function createOverlaySettingsChangedHandler(
	deps: OverlaySettingsDeps,
): (payload: SettingsChangedPayload) => Promise<void> {
	const { applyAccentColor, reloadSettingsFromDisk, queryClient, invoke } =
		deps;

	return async (payload: SettingsChangedPayload) => {
		// Apply accent immediately (without waiting on any disk reload).
		try {
			const maybeObj = payload as unknown;
			if (maybeObj && typeof maybeObj === "object") {
				const accent = (maybeObj as Record<string, unknown>).accent_color;
				if (accent === null || typeof accent === "string") {
					applyAccentColor(accent);
				}
			}
		} catch {
			// ignore (non-critical)
		}

		// In the overlay window, force a disk reload so *all* settings fields reflect
		// the latest changes made by the main window.
		try {
			await reloadSettingsFromDisk();
		} catch {
			// ignore
		}

		queryClient.invalidateQueries({ queryKey: ["settings"] });
		// Sync pipeline config when settings change
		try {
			await invoke("sync_pipeline_config");
		} catch {
			// ignore
		}
	};
}
