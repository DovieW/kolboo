import {
	isPipelineState,
	type PipelineState,
	type PipelineStateSource,
} from "../lib/overlay/overlayUiReducer";
import { useBackendEvent } from "../lib/tauri/useBackendEvent";

type UseOverlayPipelineEventsInputs = {
	setPipelineState: (source: PipelineStateSource, next: PipelineState) => void;
	clearError: () => void;
	onPipelineErrorPayload: (payload: unknown) => void;
};

export function useOverlayPipelineEvents({
	setPipelineState,
	clearError,
	onPipelineErrorPayload,
}: UseOverlayPipelineEventsInputs) {
	useBackendEvent("pipeline-state-changed", (next) => {
		if (isPipelineState(next)) setPipelineState("event", next);
	});
	useBackendEvent("pipeline-cancelled", () => {
		setPipelineState("event", "idle");
		clearError();
	});
	useBackendEvent("pipeline-reset", () => {
		setPipelineState("event", "idle");
		clearError();
	});
	useBackendEvent("pipeline-error", (payload) => {
		onPipelineErrorPayload(payload);
		setPipelineState("event", "error");
	});
	useBackendEvent("pipeline-transcript-ready", () => {
		setPipelineState("event", "idle");
		clearError();
	});
}
