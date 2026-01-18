import { describe, expect, it } from "vitest";
import {
  PIPELINE_POLL_SUPPRESS_MS,
  overlayUiReducer,
  type OverlayUiState,
} from "./overlayUiReducer";

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
});
