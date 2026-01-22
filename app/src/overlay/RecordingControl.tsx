import { useResizeObserver } from "@mantine/hooks";
import { useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useDrag } from "@use-gesture/react";
import {
	useCallback,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
} from "react";
import { applyAccentColor } from "../lib/accentColor";
import { readBootAccentColor } from "../lib/bootStorage";
import { createOverlaySettingsChangedHandler } from "../lib/overlay/overlaySettings";
import {
	type ErrorInfo,
	isPipelineState,
	type PipelineState,
} from "../lib/overlay/overlayUiReducer";
import { useSettings, useTypeText } from "../lib/queries";
import {
	type CommandErrorPayload,
	type ConnectionState,
	type IntentRouterSettings,
	type RewriteProgramPromptProfile,
	tauriAPI,
} from "../lib/tauri";
import { listenTyped } from "../lib/tauri/events";
import { useOverlayUiReducer } from "../lib/useOverlayUiReducer";
import { AudioWave, BackendAudioWave } from "./AudioWave";
import {
	type ActiveProfileInfo,
	useOverlayController,
} from "./useOverlayController";
import { useOverlayHoverGating } from "./useOverlayHoverGating";

type CommandErrorExtract = {
	message: string;
	details: string | null;
	code: string | null;
	retryable: boolean | null;
	requestId: string | null;
};

function extractCommandError(error: unknown): CommandErrorExtract {
	if (!error || typeof error !== "object") {
		return {
			message: String(error),
			details: null,
			code: null,
			retryable: null,
			requestId: null,
		};
	}

	const payload = error as CommandErrorPayload;
	const message =
		typeof payload.message === "string" ? payload.message : String(error);
	const details = typeof payload.details === "string" ? payload.details : null;
	const code = typeof payload.code === "string" ? payload.code : null;
	const retryable =
		typeof payload.retryable === "boolean" ? payload.retryable : null;
	const requestId =
		typeof payload.request_id === "string" ? payload.request_id : null;

	return {
		message,
		details,
		code,
		retryable,
		requestId,
	};
}

/**
 * Parse error message to user-friendly format
 */
function parseErrorMessage(
	message: string,
	retryable: boolean | null,
): ErrorInfo {
	const errorStr = message;
	const recoverable = retryable ?? true;

	// Missing persisted audio (retry can't run)
	if (
		errorStr.includes("Failed to read recording") ||
		errorStr.includes("Recording store") ||
		errorStr.includes("Cannot save recording")
	) {
		return { message: "No saved audio", recoverable };
	}

	// Network/API errors
	if (errorStr.includes("Network") || errorStr.includes("network")) {
		return { message: "Network error", recoverable };
	}
	if (errorStr.includes("timeout") || errorStr.includes("Timeout")) {
		return { message: "Timed out", recoverable };
	}
	if (errorStr.includes("API error") || errorStr.includes("401")) {
		return { message: "API error", recoverable };
	}
	if (errorStr.includes("rate limit") || errorStr.includes("429")) {
		return { message: "Rate limited", recoverable };
	}

	// Provider errors
	if (errorStr.includes("NoProvider") || errorStr.includes("No STT provider")) {
		return { message: "No STT provider configured", recoverable };
	}

	// Recording errors
	if (errorStr.includes("NotRecording")) {
		return { message: "Not recording", recoverable };
	}
	if (errorStr.includes("AlreadyRecording")) {
		return { message: "Already recording", recoverable };
	}
	if (errorStr.includes("RecordingTooLarge")) {
		return { message: "Recording too long", recoverable };
	}

	// Audio errors
	if (errorStr.includes("audio") || errorStr.includes("Audio")) {
		return { message: "Audio capture error", recoverable };
	}

	// Generic fallback
	// If we have a real message, keep it short-ish and let the UI tooltip show the full text.
	const trimmed = errorStr.trim();
	if (trimmed && trimmed.length <= 64) {
		return { message: trimmed, recoverable };
	}
	return { message: "Error", recoverable };
}

/**
 * Map pipeline state to connection state for UI compatibility
 */
function pipelineToConnectionState(state: PipelineState): ConnectionState {
	switch (state) {
		case "idle":
			return "idle";
		case "arming":
			return "connecting";
		case "recording":
			return "recording";
		case "routing":
		case "transcribing":
		case "rewriting":
			return "processing";
		case "error":
			return "disconnected";
		default: {
			const _exhaustive: never = state;
			return _exhaustive;
		}
	}
}

/**
 * Error indicator icon component
 */
function ErrorIcon() {
	return (
		<svg
			width="20"
			height="20"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			strokeWidth="2"
			strokeLinecap="round"
			strokeLinejoin="round"
			role="img"
			aria-label="Error"
		>
			<circle cx="12" cy="12" r="10" />
			<line x1="12" y1="8" x2="12" y2="12" />
			<line x1="12" y1="16" x2="12.01" y2="16" />
		</svg>
	);
}

function RecordingDot({ state }: { state: PipelineState }) {
	const dotState =
		state === "recording" || state === "arming"
			? "recording"
			: state === "transcribing" || state === "routing" || state === "rewriting"
				? "processing"
				: "idle";

	return (
		<div
			className="overlay-dot"
			data-state={dotState}
			role="img"
			aria-label={
				dotState === "recording"
					? "Recording"
					: dotState === "processing"
						? "Processing"
						: "Idle"
			}
		/>
	);
}

export default function RecordingControl() {
	const queryClient = useQueryClient();
	const {
		pipelineState,
		animState,
		lastError,
		lastErrorDetail: _lastErrorDetail,
		lastFailedRequestId,
		setPipelineState,
		setAnimState,
		clearError,
		setError,
	} = useOverlayUiReducer();
	const {
		state: {
			expanded,
			renderExpanded,
			holdPhaseText,
			sessionPresetId,
			activeProfile,
		},
		refs: controllerRef,
		setExpanded,
		setRenderExpanded,
		setHoldPhaseText,
		setSessionPresetId,
		setActiveProfile,
	} = useOverlayController();

	const [containerRef, rect] = useResizeObserver<HTMLDivElement>();
	const widgetRef = useRef<HTMLDivElement | null>(null);

	const setWidgetRef = useCallback(
		(el: HTMLDivElement | null) => {
			widgetRef.current = el;
			// Mantine's useResizeObserver returns a ref object.
			(containerRef as React.MutableRefObject<HTMLDivElement | null>).current =
				el;
		},
		[containerRef],
	);

	// Load settings (overlay mode + selected mic)
	const { data: settings } = useSettings();

	const onSettingsChanged = useMemo(
		() =>
			createOverlaySettingsChangedHandler({
				applyAccentColor,
				reloadSettingsFromDisk: () => tauriAPI.reloadSettingsFromDisk(),
				queryClient,
				invoke,
			}),
		[queryClient],
	);

	// Hover-revealed preset controls.
	// IMPORTANT: Do NOT resize the main overlay window on hover (it causes cursor flicker/jitter).
	// Instead, we show a dedicated hover window anchored to the overlay.
	const hoverPanelEnabled =
		pipelineState !== "error" &&
		expanded &&
		(settings?.overlay_mode === "always" ||
			settings?.overlay_mode === "recording_only");

	const activeProfileId = activeProfile?.profile_id ?? null;

	const activeProfileObj = useMemo(() => {
		if (!settings) return null;
		if (!activeProfileId) return null;
		return (
			settings.rewrite_program_prompt_profiles.find(
				(p) => p.id === activeProfileId,
			) ?? null
		);
	}, [settings, activeProfileId]);

	const activeProfilePresets = useMemo(() => {
		return activeProfileObj?.presets ?? [];
	}, [activeProfileObj]);

	const hoverHasPresets = activeProfilePresets.length > 0;

	const _sessionPresetLabel = useMemo(() => {
		if (!sessionPresetId) return "Auto";
		const preset = activeProfilePresets.find((p) => p.id === sessionPresetId);
		return preset?.name ?? "Auto";
	}, [activeProfilePresets, sessionPresetId]);

	const activeProfileRouter = useMemo(() => {
		return activeProfileObj?.router ?? null;
	}, [activeProfileObj]);

	const routerIsEffectivelyOn =
		!!activeProfileRouter &&
		activeProfileRouter.enabled &&
		activeProfileRouter.strategy !== "off";

	const rewriteIsEnabled = useMemo(() => {
		if (!settings) return false;
		if (!activeProfileId) return false;

		// Default profile inherits the global rewrite toggle.
		if (activeProfileId === "default") return settings.rewrite_llm_enabled;

		const profile = activeProfileObj;
		if (!profile) return settings.rewrite_llm_enabled;

		return typeof profile.rewrite_llm_enabled === "boolean"
			? profile.rewrite_llm_enabled
			: settings.rewrite_llm_enabled;
	}, [activeProfileObj, activeProfileId, settings]);

	const hoverAllowedByPipelineState =
		pipelineState !== "transcribing" &&
		pipelineState !== "routing" &&
		pipelineState !== "rewriting";

	const shouldShowHoverPresets =
		hoverAllowedByPipelineState &&
		routerIsEffectivelyOn &&
		hoverHasPresets &&
		rewriteIsEnabled;

	const { markOverlayShownForHoverGating, handleMouseEnter, handleMouseLeave } =
		useOverlayHoverGating({
			enabled: hoverPanelEnabled,
			shouldShowPresets: shouldShowHoverPresets,
			getWidgetElement: () => widgetRef.current,
		});

	const _toggleRouterEnabled = useCallback(async () => {
		if (!settings) return;
		if (!activeProfileId) return;

		const profiles = settings.rewrite_program_prompt_profiles;
		const idx = profiles.findIndex((p) => p.id === activeProfileId);

		// Backward compatible: if Default hasn't been migrated into the profile list yet,
		// upsert it now so it can own router/presets.
		const profile: RewriteProgramPromptProfile | null =
			idx >= 0
				? (profiles[idx] ?? null)
				: activeProfileId === "default"
					? {
							id: "default",
							name: "Default",
							program_paths: [],
							cleanup_prompt_sections: null,
							presets: [],
							default_preset_id: null,
							default_preset_description: null,
							router: null,
							active_preset_id: null,
							rewrite_llm_enabled: null,
						}
					: null;

		if (!profile) return;
		const current: IntentRouterSettings | null = profile.router ?? null;

		const nextRouter: IntentRouterSettings = (() => {
			// "Off" means "not selecting presets automatically".
			// Prefer preserving the user's configured strategy/model when possible.
			if (routerIsEffectivelyOn) {
				if (!current) return { enabled: false, strategy: "off" };
				return { ...current, enabled: false };
			}

			if (current && current.strategy !== "off") {
				return { ...current, enabled: true };
			}

			// No router configured yet: pick a sensible default so the toggle actually works.
			return {
				enabled: true,
				strategy: "embeddings",
				embedding_provider: "openai",
				embedding_model: "text-embedding-3-small",
				similarity_threshold: null,
				similarity_margin: null,
			};
		})();

		const nextProfiles =
			idx >= 0
				? profiles.map((p) =>
						p.id === activeProfileId ? { ...p, router: nextRouter } : p,
					)
				: [
						// Insert Default first so it doesn't show up as a "program profile" elsewhere.
						{ ...profile, router: nextRouter },
						...profiles,
					];

		try {
			await tauriAPI.updateRewriteProgramPromptProfiles(nextProfiles);
			await tauriAPI.emitSettingsChanged({});
		} catch (error) {
			console.error("[Overlay] Failed to toggle router:", error);
		}
	}, [activeProfileId, routerIsEffectivelyOn, settings]);

	const _setSessionPresetLock = useCallback(
		async (nextPresetId: string | null) => {
			setSessionPresetId(nextPresetId);

			const profileIdForLock = activeProfileId ?? null;

			// Best-effort: set immediately so the lock applies even when stop is
			// triggered by a global hotkey.
			try {
				await invoke("pipeline_set_session_preset_lock", {
					profileId: profileIdForLock,
					presetId: nextPresetId ?? null,
				});
			} catch {
				// ignore
			}
		},
		[activeProfileId, setSessionPresetId],
	);

	const bootAccent = useMemo(() => readBootAccentColor(), []);

	// Layout effect prevents a first-paint accent flash on reload.
	useLayoutEffect(() => {
		const effectiveAccent = settings ? settings.accent_color : bootAccent;
		applyAccentColor(effectiveAccent);
	}, [bootAccent, settings]);

	// TanStack Query hooks
	const typeTextMutation = useTypeText();

	// Emit connection state changes to other windows
	useEffect(() => {
		const connectionState = pipelineToConnectionState(pipelineState);
		tauriAPI.emitConnectionState(connectionState);
	}, [pipelineState]);

	// Poll pipeline state periodically to stay in sync.
	//
	// We prefer event-driven updates, but keep polling as a backstop.
	// Avoid polling constantly while idle/hidden to reduce backend churn.
	useEffect(() => {
		let cancelled = false;
		let inFlight = false;
		let interval: number | null = null;

		const overlayIsVisible =
			settings?.overlay_mode === "always" || animState !== "exit";

		const pollMs = (() => {
			if (pipelineState !== "idle") return 500;
			if (overlayIsVisible) return 5000;
			return 0;
		})();

		const syncState = async () => {
			if (cancelled) return;
			if (inFlight) return;
			inFlight = true;
			try {
				const state = await invoke<string>("pipeline_get_state");
				if (cancelled) return;
				if (isPipelineState(state)) {
					setPipelineState("poll", state);
				} else {
					setPipelineState("poll", "idle");
				}
			} catch (error) {
				console.error("[Pipeline] Failed to get state:", error);
			} finally {
				inFlight = false;
			}
		};

		// Sync on mount and whenever polling mode changes.
		void syncState();

		if (pollMs > 0) {
			interval = window.setInterval(syncState, pollMs);
		}

		return () => {
			cancelled = true;
			if (interval) window.clearInterval(interval);
		};
	}, [animState, pipelineState, setPipelineState, settings?.overlay_mode]);

	// Resolve the active program profile periodically while expanded so we can
	// show profile-scoped preset info in the hover panel.
	useEffect(() => {
		const shouldSync = expanded;
		if (!shouldSync) return;

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
	}, [expanded, setActiveProfile]);

	// If presets change (e.g. user deleted one), avoid keeping an invalid selection.
	useEffect(() => {
		if (!sessionPresetId) return;
		if (activeProfilePresets.some((p) => p.id === sessionPresetId)) return;
		setSessionPresetId(null);
	}, [activeProfilePresets, sessionPresetId, setSessionPresetId]);

	// If there are no presets for the active profile, the hover window should never show.
	// Hide it proactively so we don't end up with an empty tiny "dot" panel.
	useEffect(() => {
		if (!expanded) return;
		if (shouldShowHoverPresets) return;
		tauriAPI.hideOverlayHover().catch(() => {});
	}, [expanded, shouldShowHoverPresets]);

	// New recording sessions should start in Auto mode.
	useEffect(() => {
		if (pipelineState !== "recording") return;
		setSessionPresetId(null);
	}, [pipelineState, setSessionPresetId]);

	useEffect(() => {
		const prev = controllerRef.current.prevPipelineForPhaseHold;
		controllerRef.current.prevPipelineForPhaseHold = pipelineState;

		if (
			pipelineState === "transcribing" ||
			pipelineState === "routing" ||
			pipelineState === "rewriting"
		) {
			controllerRef.current.lastBusyPhase = pipelineState;
			if (controllerRef.current.holdPhaseTimer) {
				window.clearTimeout(controllerRef.current.holdPhaseTimer);
				controllerRef.current.holdPhaseTimer = null;
			}
			if (!hoverPanelEnabled || !shouldShowHoverPresets) {
				setHoldPhaseText(pipelineState);
			}
			return;
		}

		// While recording-only, we expect the window to hide after a capture cycle,
		// but `idle` can arrive slightly before `overlay-hide-requested`.
		if (
			settings?.overlay_mode === "recording_only" &&
			pipelineState === "idle" &&
			(prev === "transcribing" || prev === "routing" || prev === "rewriting")
		) {
			if (holdPhaseText !== prev) {
				setHoldPhaseText(prev);
			}
			if (controllerRef.current.holdPhaseTimer) {
				window.clearTimeout(controllerRef.current.holdPhaseTimer);
			}
			// Small grace window; hide event typically arrives quickly. If it doesn't,
			// we still don't want the overlay to look "stuck".
			controllerRef.current.holdPhaseTimer = window.setTimeout(() => {
				setHoldPhaseText(null);
				controllerRef.current.holdPhaseTimer = null;
			}, 650);
			return;
		}

		// New capture cycle (or user action) should not inherit prior phase text.
		if (pipelineState === "arming" || pipelineState === "recording") {
			controllerRef.current.lastBusyPhase = null;
			if (controllerRef.current.holdPhaseTimer) {
				window.clearTimeout(controllerRef.current.holdPhaseTimer);
				controllerRef.current.holdPhaseTimer = null;
			}
			if (holdPhaseText !== null) {
				setHoldPhaseText(null);
			}
			return;
		}

		// In always-visible mode, don't let phase text linger after returning idle.
		if (settings?.overlay_mode === "always" && pipelineState === "idle") {
			if (controllerRef.current.holdPhaseTimer) {
				window.clearTimeout(controllerRef.current.holdPhaseTimer);
				controllerRef.current.holdPhaseTimer = null;
			}
			if (holdPhaseText !== null) {
				setHoldPhaseText(null);
			}
		}
	}, [
		controllerRef,
		holdPhaseText,
		pipelineState,
		hoverPanelEnabled,
		shouldShowHoverPresets,
		setHoldPhaseText,
		settings?.overlay_mode,
	]);

	// Resize the native window for the target widget, and only then render it.
	// This avoids the "intermediate step" where the widget is wider than the window
	// (or vice versa) for a frame.
	useEffect(() => {
		if (expanded) {
			// In recording-only mode, the backend controls show/hide. Never render a blank
			// transparent window while we wait for resize observer/pipeline polling.
			if (settings?.overlay_mode === "recording_only") {
				setRenderExpanded(true);
				tauriAPI.resizeOverlay(224, 56);
				return;
			}

			// During an active capture cycle, prioritize responsiveness over avoiding a
			// one-frame clipped border: render immediately so the waveform can warm up.
			if (pipelineState !== "idle") {
				setRenderExpanded(true);
			} else {
				setRenderExpanded(false);
			}
			tauriAPI.resizeOverlay(224, 56);
			return;
		}

		// Collapse: hide expanded immediately, then shrink window.
		setRenderExpanded(false);
		tauriAPI.resizeOverlay(56, 56);
	}, [expanded, pipelineState, setRenderExpanded, settings?.overlay_mode]);

	useEffect(() => {
		if (!expanded) return;

		// Recording-only mode should always show the full widget when visible.
		if (settings?.overlay_mode === "recording_only") {
			if (!renderExpanded) setRenderExpanded(true);
			return;
		}

		// If we're active, we already rendered immediately above.
		if (pipelineState !== "idle") return;

		if (rect.width >= 220) {
			setRenderExpanded(true);
		}
	}, [
		expanded,
		pipelineState,
		rect.width,
		renderExpanded,
		setRenderExpanded,
		settings?.overlay_mode,
	]);

	// Keep expanded while active; collapse when returning to idle.
	useEffect(() => {
		const prev = controllerRef.current.prevPipelineForExpand;
		controllerRef.current.prevPipelineForExpand = pipelineState;

		// In recording-only overlay mode, we never want to show the collapsed widget.
		// The window itself is shown/hidden by the backend; the overlay should stay in
		// its full state whenever it is visible.
		if (settings?.overlay_mode === "recording_only") {
			setExpanded(true);
			return;
		}

		if (
			pipelineState === "arming" ||
			pipelineState === "recording" ||
			pipelineState === "transcribing" ||
			pipelineState === "rewriting" ||
			pipelineState === "error"
		) {
			setExpanded(true);
			return;
		}

		// Collapse immediately after finishing an active state.
		if (pipelineState === "idle" && prev !== "idle") {
			setExpanded(false);
		}
	}, [controllerRef, pipelineState, setExpanded, settings?.overlay_mode]);

	// If the user switches into recording-only mode while the window is visible,
	// immediately force expanded so we don't flash the collapsed state.
	useEffect(() => {
		if (settings?.overlay_mode === "recording_only") {
			setExpanded(true);
		}
	}, [setExpanded, settings?.overlay_mode]);

	// If we switch *out* of recording-only mode into always-visible while idle,
	// collapse back to the default logo-only state immediately (otherwise we'd stay
	// expanded until the next recording cycle flips pipelineState away from idle).
	useEffect(() => {
		if (settings?.overlay_mode !== "always") return;
		if (pipelineState !== "idle") return;
		setExpanded(false);
	}, [pipelineState, setExpanded, settings?.overlay_mode]);

	const requestAnimatedHide = useCallback(() => {
		if (controllerRef.current.exitTimer) {
			window.clearTimeout(controllerRef.current.exitTimer);
			controllerRef.current.exitTimer = null;
		}

		setAnimState("exit");
		// Keep duration in sync with CSS transition (180ms) + a tiny buffer.
		controllerRef.current.exitTimer = window.setTimeout(() => {
			invoke("hide_overlay").catch(console.error);
			// Prep for next entrance.
			setAnimState("enter");
			// Clear held phase so the next show doesn't accidentally reuse it.
			controllerRef.current.lastBusyPhase = null;
			if (controllerRef.current.holdPhaseTimer) {
				window.clearTimeout(controllerRef.current.holdPhaseTimer);
				controllerRef.current.holdPhaseTimer = null;
			}
			setHoldPhaseText(null);
			controllerRef.current.exitTimer = null;
		}, 210);
	}, [controllerRef, setAnimState, setHoldPhaseText]);

	const dismissError = useCallback(() => {
		// Reset pipeline state in backend so polling reflects reality.
		invoke("pipeline_force_reset").catch(console.error);
		clearError();

		// If we force-showed the window for an error (recording_only/never), allow the user to hide it.
		if (settings?.overlay_mode !== "always") {
			requestAnimatedHide();
		}
	}, [clearError, requestAnimatedHide, settings?.overlay_mode]);

	const requestAnimatedShow = useCallback(() => {
		if (controllerRef.current.exitTimer) {
			window.clearTimeout(controllerRef.current.exitTimer);
			controllerRef.current.exitTimer = null;
		}

		// Force a transition even if we were previously visible.
		setAnimState("enter");
		markOverlayShownForHoverGating();
		requestAnimationFrame(() => {
			setAnimState("visible");
		});
	}, [controllerRef, markOverlayShownForHoverGating, setAnimState]);

	// Entrance animation when recording starts (recording-only mode shows the window)
	useEffect(() => {
		if (settings?.overlay_mode === "always") {
			setAnimState("visible");
			return;
		}

		if (
			pipelineState === "arming" ||
			pipelineState === "recording" ||
			pipelineState === "transcribing" ||
			pipelineState === "rewriting"
		) {
			requestAnimatedShow();
		}
	}, [
		pipelineState,
		requestAnimatedShow,
		settings?.overlay_mode,
		setAnimState,
	]);

	// Backend can request a hide (so we can animate out before the window hides)
	useEffect(() => {
		let unlisten: (() => void) | undefined;

		const setup = async () => {
			unlisten = await listenTyped("overlay-hide-requested", () => {
				requestAnimatedHide();
			});
		};

		void setup();
		return () => {
			unlisten?.();
		};
	}, [requestAnimatedHide]);

	// If the overlay itself was used to record (not hotkey path), honor recording-only by
	// animating out when we return to idle.
	useEffect(() => {
		const prev = controllerRef.current.prevPipelineState;
		controllerRef.current.prevPipelineState = pipelineState;

		if (settings?.overlay_mode !== "recording_only") return;
		if (pipelineState !== "idle") return;
		if (
			prev === "recording" ||
			prev === "transcribing" ||
			prev === "rewriting" ||
			prev === "error"
		) {
			requestAnimatedHide();
		}
	}, [
		controllerRef,
		pipelineState,
		requestAnimatedHide,
		settings?.overlay_mode,
	]);

	// Start recording using the Rust pipeline
	const onStartRecording = useCallback(async () => {
		if (pipelineState !== "idle") return;

		// Clear any previous error when starting
		clearError();

		// Optimistic UX: the backend begins capturing before the UI can receive events.
		// Show "REC" immediately so the overlay matches when the user can start talking.
		setPipelineState("ui", "recording");

		try {
			await invoke("pipeline_start_recording");

			// If recording is already active, reflect it immediately.
			// This reduces the confusing "Arm" state when the backend is already capturing.
			try {
				const state = await invoke<string>("pipeline_get_state");
				if (isPipelineState(state)) {
					setPipelineState("sync", state);
				}
			} catch {
				// If polling fails, we'll still rely on event listeners / interval polling.
			}
		} catch (error) {
			console.error("[Pipeline] Failed to start recording:", error);
			const payload = extractCommandError(error);
			const errorInfo = parseErrorMessage(payload.message, payload.retryable);
			const detail = payload.details ?? payload.code ?? null;
			setError(errorInfo, detail, payload.requestId);
			setPipelineState("ui", "error");
		}
	}, [clearError, pipelineState, setError, setPipelineState]);

	// Stop recording and transcribe
	const onStopRecording = useCallback(async () => {
		if (pipelineState !== "recording") return;

		try {
			// Best-effort: set the one-shot preset lock right before we transcribe.
			// This allows the user to force a preset for *this* dictation without
			// persisting the override.
			try {
				await invoke("pipeline_set_session_preset_lock", {
					profileId:
						activeProfileId && activeProfileId !== "default"
							? activeProfileId
							: null,
					presetId: sessionPresetId ?? null,
				});
			} catch (error) {
				console.error("[Pipeline] Failed to set session preset lock:", error);
			}

			// UX: once the user stops, always show "transcribing" (even if the backend
			// ends up short-circuiting due to quiet-audio gating).
			setPipelineState("ui", "transcribing");

			const transcript = await invoke<string>("pipeline_stop_and_transcribe");

			if (transcript) {
				// Type the transcript
				try {
					await typeTextMutation.mutateAsync(transcript);
				} catch (error) {
					console.error("[Pipeline] Failed to type text:", error);
					const payload = extractCommandError(error);
					const errorInfo = parseErrorMessage(
						payload.message,
						payload.retryable,
					);
					const detail = payload.details ?? payload.code ?? null;
					setError(errorInfo, detail, payload.requestId);
				}
			}

			setPipelineState("ui", "idle");
			clearError();
			setSessionPresetId(null);
		} catch (error) {
			console.error("[Pipeline] Failed to stop and transcribe:", error);
			setPipelineState("ui", "error");

			// Show error to user
			const payload = extractCommandError(error);
			const errorInfo = parseErrorMessage(payload.message, payload.retryable);
			const detail = payload.details ?? payload.code ?? null;
			setError(errorInfo, detail, payload.requestId);
		}
	}, [
		activeProfileId,
		clearError,
		pipelineState,
		sessionPresetId,
		setSessionPresetId,
		setError,
		setPipelineState,
		typeTextMutation,
	]);

	const onRetry = useCallback(async () => {
		if (!lastFailedRequestId) return;
		try {
			setPipelineState("ui", "transcribing");
			clearError();

			// Best-effort: apply session lock to retry too.
			try {
				await invoke("pipeline_set_session_preset_lock", {
					profileId:
						activeProfileId && activeProfileId !== "default"
							? activeProfileId
							: null,
					presetId: sessionPresetId ?? null,
				});
			} catch {
				// ignore
			}

			const transcript = await invoke<string>("pipeline_retry_transcription", {
				requestId: lastFailedRequestId,
			});

			if (transcript) {
				try {
					await typeTextMutation.mutateAsync(transcript);
				} catch (error) {
					console.error("[Pipeline] Failed to type retry transcript:", error);
					const payload = extractCommandError(error);
					const errorInfo = parseErrorMessage(
						payload.message,
						payload.retryable,
					);
					const detail = payload.details ?? payload.code ?? null;
					setError(errorInfo, detail, payload.requestId);
				}
			}

			setPipelineState("ui", "idle");
			clearError();
			setSessionPresetId(null);
		} catch (error) {
			console.error("[Pipeline] Retry failed:", error);
			setPipelineState("ui", "error");
			const payload = extractCommandError(error);
			const errorInfo = parseErrorMessage(payload.message, payload.retryable);
			const detail = payload.details ?? payload.code ?? null;
			setError(errorInfo, detail, payload.requestId);
		}
	}, [
		activeProfileId,
		clearError,
		lastFailedRequestId,
		sessionPresetId,
		setSessionPresetId,
		setError,
		setPipelineState,
		typeTextMutation,
	]);

	// Hotkey event listeners
	// Listen for recording state changes from shortcuts (Rust handles the actual recording)
	useEffect(() => {
		let unlistenStart: (() => void) | undefined;
		let unlistenStop: (() => void) | undefined;

		const setup = async () => {
			// When shortcut triggers recording, just update UI state (don't call command again)
			unlistenStart = await tauriAPI.onStartRecording(() => {
				// Hotkey events can arrive slightly before the overlay receives any other
				// pipeline events; still show "REC" immediately to match actual capture.
				clearError();
				setPipelineState("hotkey", "recording");

				// In recording_only mode the backend may have just shown the window while the
				// overlay UI is still in the pre-show "enter" animation state (opacity 0)
				// from the prior hide. Force visibility immediately so the window isn't
				// effectively invisible on short recordings.
				setAnimState("visible");

				// Treat this as a "show" moment for hover gating too.
				markOverlayShownForHoverGating();
			});
			unlistenStop = await tauriAPI.onStopRecording(() => {
				// UX: once the user stops, always show "transcribing".
				setPipelineState("hotkey", "transcribing");
			});
		};

		void setup();

		return () => {
			unlistenStart?.();
			unlistenStop?.();
		};
	}, [
		clearError,
		markOverlayShownForHoverGating,
		setAnimState,
		setPipelineState,
	]);

	// Listen for pipeline events from Rust
	useEffect(() => {
		const unlisteners: (() => void)[] = [];

		const setup = async () => {
			// Canonical state update event (preferred): reduces event surface area.
			unlisteners.push(
				await listenTyped("pipeline-state-changed", (payload) => {
					const next = (payload ?? "").toString();
					if (isPipelineState(next)) {
						setPipelineState("event", next);
					}
				}),
			);

			unlisteners.push(
				await listenTyped("pipeline-cancelled", () => {
					setPipelineState("event", "idle");
					clearError();
				}),
			);

			unlisteners.push(
				await listenTyped("pipeline-reset", () => {
					setPipelineState("event", "idle");
					clearError();
				}),
			);

			// Listen for pipeline errors (e.g., transcription failures from hotkey-triggered recordings)
			unlisteners.push(
				await listenTyped("pipeline-error", (payload) => {
					console.error("[Pipeline] Error from Rust:", payload);
					setPipelineState("event", "error");

					const errorPayload = extractCommandError(payload);
					const errorInfo = parseErrorMessage(
						errorPayload.message,
						errorPayload.retryable,
					);
					const detail = errorPayload.details ?? errorPayload.code ?? null;
					setError(errorInfo, detail, errorPayload.requestId);
				}),
			);

			// Listen for successful transcription (from hotkey-triggered recordings)
			unlisteners.push(
				await listenTyped("pipeline-transcript-ready", () => {
					setPipelineState("event", "idle");
					clearError();
				}),
			);
		};

		void setup();

		return () => {
			for (const unlisten of unlisteners) {
				unlisten();
			}
		};
	}, [clearError, setError, setPipelineState]);

	// Listen for settings changes from main window
	useEffect(() => {
		let unlisten: (() => void) | undefined;

		const setup = async () => {
			unlisten = await tauriAPI.onSettingsChanged(onSettingsChanged);
		};

		void setup();
		return () => {
			unlisten?.();
		};
	}, [onSettingsChanged]);

	// Click behavior:
	// - idle + collapsed: expand and start recording immediately
	// - idle + expanded: start recording
	// - recording: stop recording
	const handleClick = useCallback(() => {
		if (pipelineState === "recording") {
			void onStopRecording();
			return;
		}

		if (pipelineState === "arming") {
			return;
		}

		if (pipelineState === "idle" || pipelineState === "error") {
			if (!expanded) {
				setExpanded(true);
			}
			void onStartRecording();
		}
	}, [expanded, onStartRecording, onStopRecording, pipelineState, setExpanded]);

	// Drag handler using @use-gesture/react
	const bindDrag = useDrag(
		({ movement: [mx, my], first, last, memo }) => {
			if (first) {
				controllerRef.current.hasDragStarted = false;
				return false;
			}

			const distance = Math.sqrt(mx * mx + my * my);
			const DRAG_THRESHOLD = 5;

			if (!memo && distance > DRAG_THRESHOLD) {
				controllerRef.current.hasDragStarted = true;
				tauriAPI.startDragging();
				return true;
			}

			if (last) {
				controllerRef.current.hasDragStarted = false;
			}

			return memo;
		},
		{ filterTaps: true },
	);

	const isLoading =
		pipelineState === "transcribing" ||
		pipelineState === "routing" ||
		pipelineState === "rewriting";
	const isArming = pipelineState === "arming";
	const isRecording = pipelineState === "recording";
	const isWaveActive = isArming || isRecording;
	const isBusy = isArming || isLoading;
	const isError = pipelineState === "error";
	const showDetailedLoading = settings?.overlay_show_detailed_loading ?? false;
	const centerPhaseText = (() => {
		if (pipelineState === "rewriting") return "rewriting...";
		if (pipelineState === "routing") return "routing...";
		if (pipelineState === "transcribing") return "transcribing...";

		// Recording-only: keep the last busy phase visible across the small idle gap
		// (before the backend hide request arrives).
		if (settings?.overlay_mode === "recording_only") {
			if (holdPhaseText === "rewriting") return "rewriting...";
			if (holdPhaseText === "routing") return "routing...";
			if (holdPhaseText === "transcribing") return "transcribing...";
		}

		// While fading out, keep showing the last busy phase (if any) to avoid a
		// one-frame flash of the waveform as the pipeline returns to idle.
		if (animState === "exit") {
			if (controllerRef.current.lastBusyPhase === "rewriting")
				return "rewriting...";
			if (controllerRef.current.lastBusyPhase === "routing")
				return "routing...";
			if (controllerRef.current.lastBusyPhase === "transcribing")
				return "transcribing...";
		}

		return null;
	})();

	const renderLeftIndicator = () => {
		if (isError) {
			return (
				<div style={{ color: "#ef4444" }}>
					<ErrorIcon />
				</div>
			);
		}

		return <RecordingDot state={pipelineState} />;
	};

	return (
		<div
			ref={setWidgetRef}
			role="application"
			{...bindDrag()}
			className="overlay-widget"
			data-anim={animState}
			onMouseEnter={handleMouseEnter}
			onMouseLeave={handleMouseLeave}
			style={{
				width: "100%",
				position: "relative",
				cursor: "grab",
				userSelect: "none",
			}}
		>
			<div className="overlay-stage">
				{/* Collapsed widget */}
				{!renderExpanded && settings?.overlay_mode !== "recording_only" ? (
					<button
						type="button"
						onClick={handleClick}
						disabled={isBusy}
						className="overlay-button overlay-button--collapsed"
						style={
							isError ? { background: "rgba(127, 29, 29, 0.92)" } : undefined
						}
					>
						<div className="overlay-icon">{renderLeftIndicator()}</div>
					</button>
				) : null}

				{/* Expanded widget */}
				{renderExpanded || settings?.overlay_mode === "recording_only" ? (
					<button
						type="button"
						onClick={handleClick}
						disabled={isBusy}
						className="overlay-button overlay-button--expanded"
						style={
							isError ? { background: "rgba(127, 29, 29, 0.92)" } : undefined
						}
					>
						<div className="overlay-icon">{renderLeftIndicator()}</div>
						<div
							className={`overlay-center${
								isError && lastError ? " overlay-center--error" : ""
							}`}
						>
							{isError && lastError ? (
								<div
									style={{ display: "flex", flexDirection: "column", gap: 4 }}
								>
									<button
										type="button"
										className="overlay-error-text"
										title={lastError.message}
										style={{ all: "unset", display: "block" }}
										onFocus={(e) => {
											e.currentTarget.scrollLeft = 0;
										}}
									>
										{lastError.message}
									</button>
								</div>
							) : centerPhaseText && showDetailedLoading ? (
								<div className="overlay-phase-text" aria-live="polite">
									{centerPhaseText}
								</div>
							) : (
								<>
									{/* Backend-driven waveform (no getUserMedia startup lag).
                      While "arming" (UI-only), keep an idle animation so the overlay
                      doesn't look dead before recording actually starts. */}
									{isWaveActive ? (
										<BackendAudioWave
											isActive={true}
											isVisible={true}
											className={isArming ? "overlay-wave--arming" : undefined}
										/>
									) : (
										<AudioWave
											isActive={false}
											isVisible={true}
											selectedMicId={settings?.selected_mic_id ?? null}
											className={isArming ? "overlay-wave--arming" : undefined}
										/>
									)}
								</>
							)}
						</div>
						<div className="overlay-meta">
							{isError ? (
								<>
									{lastFailedRequestId ? (
										<button
											type="button"
											className="overlay-pill"
											data-variant="dim"
											onClick={(e) => {
												e.stopPropagation();
												void onRetry();
											}}
										>
											Retry
										</button>
									) : null}

									<button
										type="button"
										className="overlay-pill overlay-pill--close"
										aria-label="Close"
										title="Close"
										onClick={(e) => {
											e.stopPropagation();
											dismissError();
										}}
									>
										×
									</button>
								</>
							) : null}
						</div>
					</button>
				) : null}
			</div>
		</div>
	);
}
