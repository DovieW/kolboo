export type AnimatedHideGateState = {
	lastRequestAt: number | null;
};

export type AnimatedHideGateParams = {
	now: number;
	animState: "enter" | "visible" | "exit";
	state: AnimatedHideGateState;
	pipelineActive: boolean;
	cooldownMs?: number;
};

export type AnimatedHideGateResult = {
	accept: boolean;
	nextState: AnimatedHideGateState;
};

/**
 * Prevent repeated animated-hide requests from thrashing overlay UI state.
 *
 * Why this exists:
 * - The backend may emit "overlay-hide-requested" close to other state transitions.
 * - The overlay UI may also decide to animate out when it returns to idle.
 * - If multiple triggers happen in a tight window, the overlay can appear to blink/flicker
 *   without any native window show/hide logs (because the exit timer may keep getting reset).
 */
export function applyAnimatedHideGate({
	now,
	animState,
	state,
	pipelineActive,
	cooldownMs = 350,
}: AnimatedHideGateParams): AnimatedHideGateResult {
	// If the pipeline is active, never accept an animated hide.
	if (pipelineActive) {
		return { accept: false, nextState: state };
	}

	// If we're already exiting, ignore duplicates.
	if (animState === "exit") {
		return { accept: false, nextState: state };
	}

	if (state.lastRequestAt != null && now - state.lastRequestAt < cooldownMs) {
		return { accept: false, nextState: state };
	}

	return {
		accept: true,
		nextState: { lastRequestAt: now },
	};
}
