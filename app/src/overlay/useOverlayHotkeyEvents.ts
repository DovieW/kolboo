import type {
	OverlayAnimState,
	PipelineState,
	PipelineStateSource,
} from "../lib/overlay/overlayUiReducer";
import { useBackendEvent } from "../lib/tauri/useBackendEvent";

type UseOverlayHotkeyEventsInputs = {
	clearError: () => void;
	setPipelineState: (source: PipelineStateSource, next: PipelineState) => void;
	setAnimState: (next: OverlayAnimState) => void;
	markOverlayShownForHoverGating: () => void;
};

/** Hotkey events arrive before polling; never leave capture transparent. */
export function useOverlayHotkeyEvents({
	clearError,
	setPipelineState,
	setAnimState,
	markOverlayShownForHoverGating,
}: UseOverlayHotkeyEventsInputs) {
	useBackendEvent("recording-start", () => {
		clearError();
		setPipelineState("hotkey", "recording");
		setAnimState("visible");
		markOverlayShownForHoverGating();
	});
	useBackendEvent("recording-stop", () => {
		setPipelineState("hotkey", "transcribing");
	});
}
