import { useEffect } from "react";
import {
	isPipelineState,
	type OverlayAnimState,
	type PipelineState,
	type PipelineStateSource,
} from "../lib/overlay/overlayUiReducer";
import { type OverlayPipelineState, ocrAPI } from "../lib/tauri";
import { getPipelinePollIntervalMs } from "./pipelinePolling";

type UseOverlayPipelineStatePollingInputs = {
	pipelineState: PipelineState;
	animState: OverlayAnimState;
	overlayMode: string | null | undefined;
	ocrStatus?: string | null;
	setPipelineState: (source: PipelineStateSource, next: PipelineState) => void;
	setOverlayState: (next: OverlayPipelineState) => void;
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
	ocrStatus,
	setPipelineState,
	setOverlayState,
}: UseOverlayPipelineStatePollingInputs) {
	useEffect(() => {
		let cancelled = false;
		let inFlight = false;
		let interval: number | null = null;

		const pollMs = getPipelinePollIntervalMs({
			pipelineState,
			animState,
			overlayMode,
			ocrStatus,
		});

		const syncState = async () => {
			if (cancelled) return;
			if (inFlight) return;
			inFlight = true;
			try {
				const state = await ocrAPI.getOverlayState();
				if (cancelled) return;
				setOverlayState(state);
				if (isPipelineState(state.pipeline_state)) {
					setPipelineState("poll", state.pipeline_state);
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
	}, [
		animState,
		ocrStatus,
		overlayMode,
		pipelineState,
		setOverlayState,
		setPipelineState,
	]);
}
