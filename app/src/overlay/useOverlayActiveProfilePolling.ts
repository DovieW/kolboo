import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
import type { ActiveProfileInfo } from "./useOverlayController";

type UseOverlayActiveProfilePollingInputs = {
	enabled: boolean;
	setActiveProfile: (next: ActiveProfileInfo) => void;
};

/**
 * Resolve the active program profile periodically while expanded so we can
 * show profile-scoped preset info.
 */
export function useOverlayActiveProfilePolling({
	enabled,
	setActiveProfile,
}: UseOverlayActiveProfilePollingInputs) {
	useEffect(() => {
		if (!enabled) return;

		let cancelled = false;
		let interval: number | null = null;

		const sync = async () => {
			try {
				const result = await invoke<ActiveProfileInfo>(
					"pipeline_get_active_profile_for_foreground_app",
				);
				if (cancelled) return;
				setActiveProfile({
					profile_id: result?.profile_id ?? null,
					profile_name: result?.profile_name ?? null,
				});
			} catch {
				// Best-effort. Overlay can still function without this.
			}
		};

		void sync();
		interval = window.setInterval(sync, 1500);

		return () => {
			cancelled = true;
			if (interval) window.clearInterval(interval);
		};
	}, [enabled, setActiveProfile]);
}
