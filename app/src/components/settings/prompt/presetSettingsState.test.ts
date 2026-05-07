import { describe, expect, it } from "vitest";
import {
	createRewritePreset,
	mergeRewritePreset,
} from "../../../lib/tauri/presetDefaults";
import type {
	RewritePreset,
	RewriteProgramPromptProfile,
} from "../../../lib/tauri/types";
import {
	presetRoutingHintsFromText,
	resolvePresetEditorState,
	resolvePresetMetadataEditorState,
} from "./presetSettingsState";

function profile(
	overrides: Partial<RewriteProgramPromptProfile>,
): RewriteProgramPromptProfile {
	return {
		id: "default",
		name: "Default",
		program_paths: [],
		cleanup_prompt_sections: null,
		presets: [],
		default_preset_id: null,
		default_preset_description: null,
		default_target_rewrite_llm_enabled: true,
		router: null,
		active_preset_id: null,
		rewrite_llm_enabled: null,
		...overrides,
	};
}

function preset(overrides: Partial<RewritePreset>): RewritePreset {
	return mergeRewritePreset(
		createRewritePreset("preset-1", "Preset"),
		overrides,
	);
}

describe("preset defaults", () => {
	it("creates new presets with canonical inherit-by-default fields", () => {
		expect(createRewritePreset("preset-1")).toMatchObject({
			id: "preset-1",
			name: "New preset",
			routing_hints: null,
			cleanup_prompt_sections: null,
			rewrite_llm_enabled: true,
			stt_provider: null,
			llm_provider: null,
			output_mode: null,
		});
	});

	it("normalizes shared preset patches through one canonical merge path", () => {
		const merged = mergeRewritePreset(
			createRewritePreset("preset-1", "Preset"),
			{
				routing_hints: [" alpha ", "", "beta"],
				stt_provider: undefined,
				llm_model: undefined,
			},
		);

		expect(merged.routing_hints).toEqual(["alpha", "beta"]);
		expect(merged.stt_provider).toBeNull();
		expect(merged.llm_model).toBeNull();
	});
});

describe("preset editor state", () => {
	it("serializes routing hints text with trimmed non-empty lines only", () => {
		expect(presetRoutingHintsFromText("alpha\n\n beta \n")).toEqual([
			"alpha",
			"beta",
		]);
		expect(presetRoutingHintsFromText("   \n\t")).toBeNull();
	});

	it("hydrates local preset metadata from the selected preset", () => {
		expect(
			resolvePresetMetadataEditorState(
				preset({
					name: "Focus mode",
					routing_hints: ["email", "draft reply"],
				}),
			),
		).toEqual({
			name: "Focus mode",
			routingHintsText: "email\ndraft reply",
		});
	});

	it("bundles runtime fallback provenance with editor metadata", () => {
		const state = resolvePresetEditorState({
			profile: profile({
				stt_provider: "groq",
				llm_model: "claude-sonnet",
			}),
			preset: preset({
				name: "Voice email",
				routing_hints: ["email"],
				llm_provider: "openai",
			}),
			settings: {
				stt_provider: "openai",
				stt_model: "gpt-4o-transcribe",
				stt_language: "en",
				stt_timeout_seconds: 10,
				llm_provider: "gemini",
				llm_model: "gemini-2.5-pro",
			},
			defaultSttTimeout: 30,
			defaultSttLanguage: "auto",
		});

		expect(state.name).toBe("Voice email");
		expect(state.routingHintsText).toBe("email");
		expect(state.runtimeFallbackViews?.sttProvider).toEqual({
			value: "groq",
			source: "profile",
			explicitNull: true,
		});
		expect(state.runtimeFallbackViews?.llmProvider).toEqual({
			value: "openai",
			source: "preset",
			explicitNull: false,
		});
	});
});
