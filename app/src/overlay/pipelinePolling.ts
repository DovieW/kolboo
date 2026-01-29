import type {
	OverlayAnimState,
	PipelineState,
} from "../lib/overlay/overlayUiReducer";

type PipelinePollingInputs = {
	pipelineState: PipelineState;
	animState: OverlayAnimState;
	overlayMode: string | null | undefined;
	ocrStatus?: string | null;
};

/**
 * Overlay uses polling as a backstop (events are preferred).
 *
 * While active (non-idle), we poll fast to keep UI tight.
 * While OCR is running, we also poll fast to track its progress.
 * While idle, we only poll when the overlay is (likely) visible.
 */
export function getPipelinePollIntervalMs({
	pipelineState,
	animState,
	overlayMode,
	ocrStatus,
}: PipelinePollingInputs): number {
	const overlayIsVisible = overlayMode === "always" || animState !== "exit";

	if (pipelineState !== "idle") return 500;
	// Keep polling fast while OCR is in progress so we can track completion.
	if (ocrStatus === "running") return 500;
	if (overlayIsVisible) return 5000;
	return 0;
}
