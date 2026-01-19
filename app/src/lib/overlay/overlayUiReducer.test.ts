import { describe, expect, it } from "vitest";
import {
	type OverlayUiState,
	overlayUiReducer,
	PIPELINE_POLL_SUPPRESS_MS,
} from "../useOverlayUiReducer";

const baseState: OverlayUiState = {
	pipelineState: "idle",
	animState: "visible",
	lastError: null,
	lastErrorDetail: null,
	lastFailedRequestId: null,
	ignorePollUntilTs: 0,
};

describe("overlayUiReducer", () => {
	it("sets pipeline state and suppresses poll after non-poll update", () => {
		const at = 1000;
		const next = overlayUiReducer(baseState, {
			type: "PIPELINE_SET",
			source: "event",
			next: "recording",
			at,
		});

		expect(next.pipelineState).toBe("recording");
		expect(next.ignorePollUntilTs).toBe(at + PIPELINE_POLL_SUPPRESS_MS);
	});

	it("ignores poll updates during suppression window", () => {
		const state: OverlayUiState = {
			...baseState,
			pipelineState: "recording",
			ignorePollUntilTs: 2000,
		};

		const next = overlayUiReducer(state, {
			type: "PIPELINE_SET",
			source: "poll",
			next: "idle",
			at: 1500,
		});

		expect(next).toBe(state);
	});

	it("allows poll updates after suppression window", () => {
		const state: OverlayUiState = {
			...baseState,
			pipelineState: "recording",
			ignorePollUntilTs: 1000,
		};

		const next = overlayUiReducer(state, {
			type: "PIPELINE_SET",
			source: "poll",
			next: "idle",
			at: 2000,
		});

		expect(next.pipelineState).toBe("idle");
		expect(next.ignorePollUntilTs).toBe(1000);
	});

	it("sets and clears errors", () => {
		const withError = overlayUiReducer(baseState, {
			type: "ERROR_SET",
			info: { message: "Boom", recoverable: true },
			detail: "detail",
			requestId: "req-1",
		});

		expect(withError.lastError?.message).toBe("Boom");
		expect(withError.lastErrorDetail).toBe("detail");
		expect(withError.lastFailedRequestId).toBe("req-1");

		const cleared = overlayUiReducer(withError, { type: "ERROR_CLEAR" });
		expect(cleared.lastError).toBeNull();
		expect(cleared.lastErrorDetail).toBeNull();
		expect(cleared.lastFailedRequestId).toBeNull();
	});

	it("sets animation state", () => {
		const next = overlayUiReducer(baseState, {
			type: "ANIM_SET",
			next: "exit",
		});
		expect(next.animState).toBe("exit");
	});

	it("ignores stale poll right after event update", () => {
		const eventAt = 1000;
		const afterEvent = overlayUiReducer(baseState, {
			type: "PIPELINE_SET",
			source: "event",
			next: "recording",
			at: eventAt,
		});

		const pollAt = eventAt + 200;
		const afterPoll = overlayUiReducer(afterEvent, {
			type: "PIPELINE_SET",
			source: "poll",
			next: "idle",
			at: pollAt,
		});

		expect(afterPoll).toBe(afterEvent);
	});

	it("accepts poll once suppression window passes", () => {
		const eventAt = 1000;
		const afterEvent = overlayUiReducer(baseState, {
			type: "PIPELINE_SET",
			source: "event",
			next: "recording",
			at: eventAt,
		});

		const pollAt = eventAt + PIPELINE_POLL_SUPPRESS_MS + 1;
		const afterPoll = overlayUiReducer(afterEvent, {
			type: "PIPELINE_SET",
			source: "poll",
			next: "idle",
			at: pollAt,
		});

		expect(afterPoll.pipelineState).toBe("idle");
		expect(afterPoll.ignorePollUntilTs).toBe(afterEvent.ignorePollUntilTs);
	});

	it("hotkey update beats poll until suppression ends", () => {
		const hotkeyAt = 500;
		const afterHotkey = overlayUiReducer(baseState, {
			type: "PIPELINE_SET",
			source: "hotkey",
			next: "recording",
			at: hotkeyAt,
		});

		const pollAt = hotkeyAt + 100;
		const afterPoll = overlayUiReducer(afterHotkey, {
			type: "PIPELINE_SET",
			source: "poll",
			next: "idle",
			at: pollAt,
		});

		expect(afterPoll).toBe(afterHotkey);
	});
});
