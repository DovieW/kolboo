import type { OverlayAnimState, PipelineState } from "../lib/overlay/overlayUiReducer";

type PipelinePollingInputs = {
	pipelineState: PipelineState;
	animState: OverlayAnimState;
	overlayMode: string | null | undefined;
};

/**
 * Overlay uses polling as a backstop (events are preferred).
 *
 * While active (non-idle), we poll fast to keep UI tight.
 * While idle, we only poll when the overlay is (likely) visible.
 */
export function getPipelinePollIntervalMs({
	pipelineState,
	animState,
	overlayMode,
}: PipelinePollingInputs): number {
	const overlayIsVisible = overlayMode === "always" || animState !== "exit";

	if (pipelineState !== "idle") return 500;
	if (overlayIsVisible) return 5000;
	return 0;
}
