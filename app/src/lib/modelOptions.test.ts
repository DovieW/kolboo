import { describe, expect, it } from "vitest";
import {
	isManagedModelSelection,
	isRealtimeSttModel,
	LLM_MODELS,
	managedChatModelOptions,
	managedModelByokTarget,
	managedTranscriptionModelOptions,
	publishedManagedModels,
	retainSavedModelOption,
	STT_MODELS,
} from "./modelOptions";
import type { ManagedModel } from "./tauri";

const models: ManagedModel[] = [
	{
		id: "whisper-large-v3-turbo",
		display_name: "Whisper Large V3 Turbo",
		provider: "groq",
		capabilities: ["transcription"],
		default_for_provider: true,
	},
	{
		id: "gpt-5-mini",
		display_name: "GPT-5 mini",
		provider: "openai",
		capabilities: ["chat_completions", "responses"],
		default_for_provider: false,
	},
];

it("keeps saved model IDs visible without changing current options", () => {
	const options = [{ value: "current", label: "Current" }];
	expect(retainSavedModelOption(options, null)).toBe(options);
	expect(retainSavedModelOption(options, "current")).toBe(options);
	expect(retainSavedModelOption(options, "old-model")).toEqual([
		...options,
		{ value: "old-model", label: "old-model (saved selection)" },
	]);
	expect(retainSavedModelOption([], "custom-model")).toEqual([]);
	expect(options).toHaveLength(1);
});

it("offers refreshed provider families without deprecated audio or duplicate IDs", () => {
	expect(STT_MODELS.fireworks).toBeUndefined();
	for (const [provider, model] of [
		["openai", "gpt-6.1-sol"],
		["gemini", "gemini-3.8-flash"],
		["anthropic", "claude-haiku-5-5"],
		["cohere", "command-a-plus-05-2026"],
		["cerebras", "qwen-3.8-27b"],
		["groq", "qwen/qwen3.8-27b"],
		["fireworks", "accounts/fireworks/models/deepseek-v4p1-flash"],
	] as const)
		expect(LLM_MODELS[provider]?.some((option) => option.value === model)).toBe(
			true,
		);
	for (const options of [
		...Object.values(STT_MODELS),
		...Object.values(LLM_MODELS),
	]) {
		expect(new Set(options.map((option) => option.value)).size).toBe(
			options.length,
		);
	}
});

it("offers current AssemblyAI models with realtime distinct from file transcription", () => {
	expect(STT_MODELS.assemblyai?.map((model) => model.value)).toEqual([
		"universal-3-6-pro",
		"universal-3-5-pro",
		"universal-2",
		"universal-streaming-english",
		"universal-streaming-multilingual",
		"universal",
	]);
	expect(isRealtimeSttModel("assemblyai", "universal-3-6-pro")).toBe(true);
	expect(isRealtimeSttModel("assemblyai", "universal-3-5-pro")).toBe(false);
	expect(isRealtimeSttModel("assemblyai", "universal-2")).toBe(false);
});

describe("managed model options", () => {
	it("respects an empty published catalog", () => {
		expect(publishedManagedModels([])).toEqual([]);
	});
	it("does not offer managed models before successful live discovery", () => {
		expect(publishedManagedModels(undefined)).toEqual([]);
		expect(publishedManagedModels(null)).toEqual([]);
		expect(publishedManagedModels(models)).toBe(models);
	});

	it("keeps transcription and chat capabilities separate", () => {
		expect(managedTranscriptionModelOptions(models, "groq")).toEqual([
			{
				value: "whisper-large-v3-turbo",
				label: "Whisper Large V3 Turbo",
			},
		]);
		expect(managedChatModelOptions(models)).toEqual([
			{ value: "gpt-5-mini", label: "GPT-5 mini · openai" },
		]);
	});

	it("matches managed compatibility by provider, model, and capability", () => {
		expect(
			isManagedModelSelection(
				models,
				"transcription",
				"groq",
				"whisper-large-v3-turbo",
			),
		).toBe(true);
		expect(
			isManagedModelSelection(
				models,
				"transcription",
				"openai",
				"whisper-large-v3-turbo",
			),
		).toBe(false);
	});

	it("maps managed models to the provider-native BYOK selection", () => {
		const openAiModel = models.find((model) => model.provider === "openai");
		if (!openAiModel) throw new Error("OpenAI fixture is missing");

		expect(managedModelByokTarget(openAiModel)).toEqual({
			provider: "openai",
			model: "gpt-5-mini",
		});
		expect(
			managedModelByokTarget({
				id: "gemini-3-flash",
				display_name: "Gemini 3 Flash",
				provider: "google",
				capabilities: ["chat_completions"],
				default_for_provider: true,
			}),
		).toEqual({
			provider: "gemini",
			model: "models/gemini-3-flash-preview",
		});
		expect(
			managedModelByokTarget({
				id: "@cf/meta/llama-3.1-8b-instruct",
				display_name: "Llama 3.1 8B",
				provider: "cloudflare",
				capabilities: ["chat_completions"],
				default_for_provider: true,
			}),
		).toBeNull();
	});
});
