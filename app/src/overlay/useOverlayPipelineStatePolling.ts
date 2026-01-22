import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
import {
	isPipelineState,
	type PipelineState,
	type PipelineStateSource,
	type OverlayAnimState,
} from "../lib/overlay/overlayUiReducer";
import { getPipelinePollIntervalMs } from "./pipelinePolling";

type UseOverlayPipelineStatePollingInputs = {
	pipelineState: PipelineState;
	animState: OverlayAnimState;
	overlayMode: string | null | undefined;
	setPipelineState: (source: PipelineStateSource, next: PipelineState) => void;
};

/**
 * Poll pipeline state periodically to stay in sync.
 *
 * We prefer event-driven updates, but keep polling as a backstop.
 */
export function useOverlayPipelineStatePolling({
	pipelineState,
	animState,
	overlayMode,
	setPipelineState,
}: UseOverlayPipelineStatePollingInputs) {
	useEffect(() => {
		let cancelled = false;
		let inFlight = false;
		let interval: number | null = null;

		const pollMs = getPipelinePollIntervalMs({
			pipelineState,
			animState,
			overlayMode,
		});

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
	}, [animState, overlayMode, pipelineState, setPipelineState]);
}
