import { describe, expect, it } from "vitest";
import { normalizeRawRewritePreset } from "./presets";

describe("raw preset settings normalizer", () => {
	it("rejects malformed preset records without stable ids", () => {
		expect(normalizeRawRewritePreset(null)).toBeNull();
		expect(normalizeRawRewritePreset({ name: "No id" })).toBeNull();
	});

	it("normalizes legacy preset blobs through canonical inherit semantics", () => {
		expect(
			normalizeRawRewritePreset({
				id: "preset-1",
				name: "Voice email",
				description: "   ",
				routing_hints: [" email ", "", 5, "reply"],
				rewrite_llm_enabled: null,
				stt_language: "  EN ",
				stt_timeout_seconds: Number.NaN,
				playing_audio_handling: "mute_and_pause",
				output_mode: "keystrokes",
			}),
		).toMatchObject({
			id: "preset-1",
			name: "Voice email",
			description: null,
			routing_hints: ["email", "reply"],
			rewrite_llm_enabled: true,
			stt_language: "en",
			stt_timeout_seconds: null,
			playing_audio_handling: "mute_and_pause",
			output_mode: "paste",
		});
	});
});
