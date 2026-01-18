import { useCallback, useReducer } from "react";

export type PipelineState =
	| "idle"
	| "arming"
	| "recording"
	| "routing"
	| "transcribing"
	| "rewriting"
	| "error";

export function isPipelineState(value: string): value is PipelineState {
	return (
		value === "idle" ||
		// NOTE: "arming" is a UI-only state; Rust will never return it.
		value === "arming" ||
		value === "recording" ||
		value === "routing" ||
		value === "transcribing" ||
		value === "rewriting" ||
		value === "error"
	);
}

export interface ErrorInfo {
	message: string;
	recoverable: boolean;
}

export type PipelineStateSource = "event" | "hotkey" | "poll" | "sync" | "ui";

export type OverlayAnimState = "enter" | "visible" | "exit";

export type OverlayUiState = {
	pipelineState: PipelineState;
	animState: OverlayAnimState;

	lastError: ErrorInfo | null;
	lastErrorDetail: string | null;
	lastFailedRequestId: string | null;

	// When we get a non-poll state update (hotkey/event/ui), suppress poll updates
	// for a short time to avoid flicker/races.
	ignorePollUntilTs: number;
};

export type OverlayUiAction =
	| {
			type: "PIPELINE_SET";
			source: PipelineStateSource;
			next: PipelineState;
			at: number;
	  }
	| {
			type: "ANIM_SET";
			next: OverlayAnimState;
	  }
	| {
			type: "ERROR_CLEAR";
	  }
	| {
			type: "ERROR_SET";
			info: ErrorInfo;
			detail: string | null;
			requestId: string | null;
	  };

export const PIPELINE_POLL_SUPPRESS_MS = 1500;

const initialOverlayUiState: OverlayUiState = {
	pipelineState: "idle",
	animState: "visible",
	lastError: null,
	lastErrorDetail: null,
	lastFailedRequestId: null,
	ignorePollUntilTs: 0,
};

export function overlayUiReducer(
	state: OverlayUiState,
	action: OverlayUiAction,
): OverlayUiState {
	switch (action.type) {
		case "PIPELINE_SET": {
			// Poll is a backstop; ignore if we recently received a more authoritative signal.
			if (
				action.source === "poll" &&
				action.at < state.ignorePollUntilTs &&
				action.next !== state.pipelineState
			) {
				return state;
			}

			return {
				...state,
				pipelineState: action.next,
				ignorePollUntilTs:
					action.source === "poll"
						? state.ignorePollUntilTs
						: action.at + PIPELINE_POLL_SUPPRESS_MS,
			};
		}
		case "ANIM_SET":
			return { ...state, animState: action.next };
		case "ERROR_CLEAR":
			return {
				...state,
				lastError: null,
				lastErrorDetail: null,
				lastFailedRequestId: null,
			};
		case "ERROR_SET":
			return {
				...state,
				lastError: action.info,
				lastErrorDetail: action.detail,
				lastFailedRequestId: action.requestId,
			};
		default: {
			const _exhaustive: never = action;
			return _exhaustive;
		}
	}
}

export type OverlayUiController = {
	pipelineState: PipelineState;
	animState: OverlayAnimState;
	lastError: ErrorInfo | null;
	lastErrorDetail: string | null;
	lastFailedRequestId: string | null;
	setPipelineState: (source: PipelineStateSource, next: PipelineState) => void;
	setAnimState: (next: OverlayAnimState) => void;
	clearError: () => void;
	setError: (
		info: ErrorInfo,
		detail: string | null,
		requestId: string | null,
	) => void;
};

/*
Transition table:
Scenario | UI behavior
Hotkey before pipeline-state-changed | Show hotkey state immediately; event update may replace it; poll updates are briefly suppressed.
Poll returns stale state | Ignore poll update inside suppression window when it would flip state.
Recording-only hides right after going idle | Allow idle state during exit animation; keep last busy phase text until hide completes.
*/
export function useOverlayUiReducer(): OverlayUiController {
	const [ui, dispatchUi] = useReducer(overlayUiReducer, initialOverlayUiState);
	const {
		pipelineState,
		animState,
		lastError,
		lastErrorDetail,
		lastFailedRequestId,
	} = ui;

	const setPipelineState = useCallback(
		(source: PipelineStateSource, next: PipelineState) => {
			dispatchUi({ type: "PIPELINE_SET", source, next, at: Date.now() });
		},
		[],
	);

	const setAnimState = useCallback((next: OverlayAnimState) => {
		dispatchUi({ type: "ANIM_SET", next });
	}, []);

	const clearError = useCallback(() => {
		dispatchUi({ type: "ERROR_CLEAR" });
	}, []);

	const setError = useCallback(
		(info: ErrorInfo, detail: string | null, requestId: string | null) => {
			dispatchUi({ type: "ERROR_SET", info, detail, requestId });
		},
		[],
	);

	return {
		pipelineState,
		animState,
		lastError,
		lastErrorDetail,
		lastFailedRequestId,
		setPipelineState,
		setAnimState,
		clearError,
		setError,
	};
}
