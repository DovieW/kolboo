import { type MutableRefObject, useCallback, useRef, useState } from "react";
import type { PipelineState } from "../lib/overlay/overlayUiReducer";

export type HoldPhaseText = "transcribing" | "routing" | "rewriting" | null;

export type ActiveProfileInfo = {
	profile_id: string | null;
	profile_name: string | null;
} | null;

export type OverlayControllerRefs = {
	hasDragStarted: boolean;
	exitTimer: number | null;
	lastBusyPhase: "transcribing" | "routing" | "rewriting" | null;
	holdPhaseTimer: number | null;
	prevPipelineForPhaseHold: PipelineState;
	prevPipelineForExpand: PipelineState;
	prevPipelineState: PipelineState;
};

type OverlayControllerState = {
	expanded: boolean;
	renderExpanded: boolean;
	holdPhaseText: HoldPhaseText;
	sessionPresetId: string | null;
	activeProfile: ActiveProfileInfo;
};

export type OverlayController = {
	state: OverlayControllerState;
	refs: MutableRefObject<OverlayControllerRefs>;
	setExpanded: (next: boolean) => void;
	setRenderExpanded: (next: boolean) => void;
	setHoldPhaseText: (next: HoldPhaseText) => void;
	setSessionPresetId: (next: string | null) => void;
	setActiveProfile: (next: ActiveProfileInfo) => void;
};

export function useOverlayController(): OverlayController {
	const refs = useRef<OverlayControllerRefs>({
		hasDragStarted: false,
		exitTimer: null,
		lastBusyPhase: null,
		holdPhaseTimer: null,
		prevPipelineForPhaseHold: "idle",
		prevPipelineForExpand: "idle",
		prevPipelineState: "idle",
	});

	const [state, setState] = useState<OverlayControllerState>({
		expanded: false,
		renderExpanded: false,
		holdPhaseText: null,
		sessionPresetId: null,
		activeProfile: null,
	});

	const setExpanded = useCallback((next: boolean) => {
		setState((prev) =>
			prev.expanded === next ? prev : { ...prev, expanded: next },
		);
	}, []);

	const setRenderExpanded = useCallback((next: boolean) => {
		setState((prev) =>
			prev.renderExpanded === next ? prev : { ...prev, renderExpanded: next },
		);
	}, []);

	const setHoldPhaseText = useCallback((next: HoldPhaseText) => {
		setState((prev) =>
			prev.holdPhaseText === next ? prev : { ...prev, holdPhaseText: next },
		);
	}, []);

	const setSessionPresetId = useCallback((next: string | null) => {
		setState((prev) =>
			prev.sessionPresetId === next ? prev : { ...prev, sessionPresetId: next },
		);
	}, []);

	const setActiveProfile = useCallback((next: ActiveProfileInfo) => {
		setState((prev) =>
			prev.activeProfile === next ? prev : { ...prev, activeProfile: next },
		);
	}, []);

	return {
		state,
		refs,
		setExpanded,
		setRenderExpanded,
		setHoldPhaseText,
		setSessionPresetId,
		setActiveProfile,
	};
}
