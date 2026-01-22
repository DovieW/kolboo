import { useEffect } from "react";
import type {
	OverlayAnimState,
	PipelineState,
	PipelineStateSource,
} from "../lib/overlay/overlayUiReducer";
import { tauriAPI } from "../lib/tauri";

type UseOverlayHotkeyEventsInputs = {
	clearError: () => void;
	setPipelineState: (source: PipelineStateSource, next: PipelineState) => void;
	setAnimState: (next: OverlayAnimState) => void;
	markOverlayShownForHoverGating: () => void;
};

/**
 * Listen for recording hotkey events (backend controls the actual recording).
 */
export function useOverlayHotkeyEvents({
	clearError,
	setPipelineState,
	setAnimState,
	markOverlayShownForHoverGating,
}: UseOverlayHotkeyEventsInputs) {
	useEffect(() => {
		let unlistenStart: (() => void) | undefined;
		let unlistenStop: (() => void) | undefined;

		const setup = async () => {
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
	}, [clearError, markOverlayShownForHoverGating, setAnimState, setPipelineState]);
}
