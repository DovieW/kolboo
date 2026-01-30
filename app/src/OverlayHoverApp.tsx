import "./app.css";

import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useModifierKeyForwarder } from "./hooks/useModifierKeyForwarder";
import { useSettings } from "./lib/queries";
import { tauriAPI } from "./lib/tauri";

type ActiveProfileInfo = {
	profile_id: string | null;
	profile_name: string | null;
};

type SessionPresetLockInfo = {
	profile_id: string | null;
	preset_id: string | null;
};

export default function OverlayHoverApp() {
	// Forward modifier-only key events (like AltRight) to the backend.
	// WebView2 intercepts these before our keyboard hook sees them.
	useModifierKeyForwarder();

	const { data: settings } = useSettings();
	const [activeProfile, setActiveProfile] = useState<ActiveProfileInfo | null>(
		null,
	);
	const activeProfileId = activeProfile?.profile_id ?? null;

	const [sessionLock, setSessionLock] = useState<SessionPresetLockInfo>({
		profile_id: null,
		preset_id: null,
	});

	const hideTimerRef = useRef<number | null>(null);

	const scheduleHide = useCallback((delayMs: number) => {
		if (hideTimerRef.current) {
			window.clearTimeout(hideTimerRef.current);
		}
		hideTimerRef.current = window.setTimeout(() => {
			tauriAPI.scheduleHideOverlayHover(0);
			hideTimerRef.current = null;
		}, delayMs);
	}, []);

	const keepAlive = useCallback(() => {
		// Re-showing is idempotent and cancels any pending backend hide.
		tauriAPI.showOverlayHover();
	}, []);

	// IMPORTANT: do not auto-show the hover window on mount. The backend controls
	// visibility and positioning; we only refresh while the pointer is over the panel.

	// Resolve the active program profile periodically.
	useEffect(() => {
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
				// ignore
			}
		};

		void sync();
		interval = window.setInterval(sync, 1500);
		return () => {
			cancelled = true;
			if (interval) window.clearInterval(interval);
		};
	}, []);

	// Poll the current session preset lock (so both windows stay in sync).
	useEffect(() => {
		let cancelled = false;
		let interval: number | null = null;

		const sync = async () => {
			try {
				const result = await invoke<SessionPresetLockInfo>(
					"pipeline_get_session_preset_lock",
				);
				if (cancelled) return;
				setSessionLock({
					profile_id: result?.profile_id ?? null,
					preset_id: result?.preset_id ?? null,
				});
			} catch {
				// ignore
			}
		};

		void sync();
		interval = window.setInterval(sync, 600);
		return () => {
			cancelled = true;
			if (interval) window.clearInterval(interval);
		};
	}, []);

	const activeProfilePresets = useMemo(() => {
		if (!settings) return [];
		if (!activeProfileId) return [];
		const profile = settings.rewrite_program_prompt_profiles.find(
			(p) => p.id === activeProfileId,
		);
		return profile?.presets ?? [];
	}, [settings, activeProfileId]);

	const hasPresets = activeProfilePresets.length > 0;

	const routerIsEffectivelyOn = useMemo(() => {
		if (!settings) return false;
		if (!activeProfileId) return false;
		const profile = settings.rewrite_program_prompt_profiles.find(
			(p) => p.id === activeProfileId,
		);
		const r = profile?.router ?? null;
		return Boolean(r?.enabled && r.strategy !== "off");
	}, [settings, activeProfileId]);

	const rewriteIsEnabled = useMemo(() => {
		if (!settings) return false;
		if (!activeProfileId) return false;

		// Default profile uses the global rewrite toggle.
		if (activeProfileId === "default") return settings.rewrite_llm_enabled;

		const profile = settings.rewrite_program_prompt_profiles.find(
			(p) => p.id === activeProfileId,
		);
		if (!profile) return settings.rewrite_llm_enabled;
		return typeof profile.rewrite_llm_enabled === "boolean"
			? profile.rewrite_llm_enabled
			: settings.rewrite_llm_enabled;
	}, [settings, activeProfileId]);

	const shouldShow = routerIsEffectivelyOn && hasPresets && rewriteIsEnabled;

	const setSessionPresetLock = useCallback(
		async (nextPresetId: string | null) => {
			try {
				const profileIdForLock = activeProfileId ?? null;
				await invoke("pipeline_set_session_preset_lock", {
					profileId: profileIdForLock,
					presetId: nextPresetId ?? null,
				});
				// Refresh quickly.
				keepAlive();
			} catch {
				// ignore
			}
		},
		[activeProfileId, keepAlive],
	);

	// Hard rule: hover overlay should never show unless the router is on, rewrite is enabled,
	// and there are presets.
	useEffect(() => {
		if (!shouldShow) {
			tauriAPI.hideOverlayHover().catch(() => {});
		}
	}, [shouldShow]);

	if (!shouldShow) {
		return null;
	}

	return (
		<div
			className="overlay-hover-panel"
			role="dialog"
			aria-label="Preset controls"
			onMouseEnter={() => {
				keepAlive();
				if (hideTimerRef.current) {
					window.clearTimeout(hideTimerRef.current);
					hideTimerRef.current = null;
				}
			}}
			onMouseMove={() => {
				keepAlive();
			}}
			onMouseLeave={() => {
				scheduleHide(200);
			}}
			style={{
				position: "fixed",
				left: 0,
				top: 0,
				right: "auto",
				bottom: "auto",
				transform: "none",
				width: "100%",
				height: "100%",
				boxSizing: "border-box",
			}}
		>
			<div className="overlay-hover-row">
				{routerIsEffectivelyOn ? (
					<button
						type="button"
						className="overlay-hover-btn"
						data-active={!sessionLock.preset_id ? "true" : "false"}
						disabled={!activeProfileId}
						onClick={() => {
							void setSessionPresetLock(null);
						}}
					>
						Auto
					</button>
				) : null}
				{activeProfilePresets.map((p) => (
					<button
						key={p.id}
						type="button"
						className="overlay-hover-btn"
						data-active={sessionLock.preset_id === p.id ? "true" : "false"}
						disabled={!activeProfileId}
						title={p.description ?? undefined}
						onClick={() => {
							if (!routerIsEffectivelyOn && sessionLock.preset_id === p.id) {
								// When routing is off we hide the explicit "Auto" button.
								// Clicking the active preset again clears the one-shot lock.
								void setSessionPresetLock(null);
							} else {
								void setSessionPresetLock(p.id);
							}
						}}
					>
						{p.name.trim().slice(0, 20) || "Preset"}
					</button>
				))}
			</div>
		</div>
	);
}
