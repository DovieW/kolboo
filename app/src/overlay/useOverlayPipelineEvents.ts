import { useEffect } from "react";
import {
	isPipelineState,
	type PipelineState,
	type PipelineStateSource,
} from "../lib/overlay/overlayUiReducer";
import { listenTyped } from "../lib/tauri/events";

type UseOverlayPipelineEventsInputs = {
	setPipelineState: (source: PipelineStateSource, next: PipelineState) => void;
	clearError: () => void;
	onPipelineErrorPayload: (payload: unknown) => void;
};

/**
 * Listen for pipeline events from the Rust backend.
 */
export function useOverlayPipelineEvents({
	setPipelineState,
	clearError,
	onPipelineErrorPayload,
}: UseOverlayPipelineEventsInputs) {
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

			unlisteners.push(
				await listenTyped("pipeline-error", (payload) => {
					onPipelineErrorPayload(payload);
					setPipelineState("event", "error");
				}),
			);

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
	}, [clearError, onPipelineErrorPayload, setPipelineState]);
}
