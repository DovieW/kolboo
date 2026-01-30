import { describe, expect, it } from "vitest";
import { getPipelinePollIntervalMs } from "./pipelinePolling";

describe("getPipelinePollIntervalMs", () => {
	it("polls fast when pipeline is active", () => {
		expect(
			getPipelinePollIntervalMs({
				pipelineState: "recording",
				animState: "visible",
				overlayMode: "never",
			}),
		).toBe(500);
	});

	it("polls slowly while idle when overlay is visible", () => {
		expect(
			getPipelinePollIntervalMs({
				pipelineState: "idle",
				animState: "visible",
				overlayMode: "recording_only",
			}),
		).toBe(5000);
	});

	it("does not poll while idle when overlay is fully hidden", () => {
		expect(
			getPipelinePollIntervalMs({
				pipelineState: "idle",
				animState: "exit",
				overlayMode: "recording_only",
			}),
		).toBe(0);
	});

	it("treats overlay_mode=always as visible even when anim is exit", () => {
		expect(
			getPipelinePollIntervalMs({
				pipelineState: "idle",
				animState: "exit",
				overlayMode: "always",
			}),
		).toBe(5000);
	});

	describe("OCR running behavior", () => {
    it("polls fast when OCR is running regardless of pipeline state", () => {
      expect(
        getPipelinePollIntervalMs({
          pipelineState: "idle",
          animState: "visible",
          overlayMode: "always",
          ocrStatus: "running",
        }),
      ).toBe(500);
    });

    it("polls fast when OCR is running even if overlay would otherwise be hidden", () => {
      expect(
        getPipelinePollIntervalMs({
          pipelineState: "idle",
          animState: "exit",
          overlayMode: "recording_only",
          ocrStatus: "running",
        }),
      ).toBe(500);
    });

    it("polls slowly when OCR is done and pipeline is idle", () => {
      expect(
        getPipelinePollIntervalMs({
          pipelineState: "idle",
          animState: "visible",
          overlayMode: "always",
          ocrStatus: "done",
        }),
      ).toBe(5000);
    });

    it("polls slowly when OCR is not_started and pipeline is idle", () => {
      expect(
        getPipelinePollIntervalMs({
          pipelineState: "idle",
          animState: "visible",
          overlayMode: "always",
          ocrStatus: "not_started",
        }),
      ).toBe(5000);
    });

    it("polls slowly when OCR is failed and pipeline is idle", () => {
      expect(
        getPipelinePollIntervalMs({
          pipelineState: "idle",
          animState: "visible",
          overlayMode: "always",
          ocrStatus: "failed",
        }),
      ).toBe(5000);
    });

    it("polls fast when both pipeline is active and OCR is running", () => {
      expect(
        getPipelinePollIntervalMs({
          pipelineState: "recording",
          animState: "visible",
          overlayMode: "always",
          ocrStatus: "running",
        }),
      ).toBe(500);
    });

    it("uses default (no OCR parameter) gracefully", () => {
      expect(
        getPipelinePollIntervalMs({
          pipelineState: "idle",
          animState: "visible",
          overlayMode: "always",
          ocrStatus: undefined,
        }),
      ).toBe(5000);
    });

    it("handles null OCR status like undefined", () => {
      expect(
        getPipelinePollIntervalMs({
          pipelineState: "idle",
          animState: "visible",
          overlayMode: "always",
          ocrStatus: null,
        }),
      ).toBe(5000);
    });
  });
});
