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
});
