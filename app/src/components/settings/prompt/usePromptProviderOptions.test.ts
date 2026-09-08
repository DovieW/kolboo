import { expect, it, vi } from "vitest";
import type { AppSettings } from "../../../lib/tauri";
import { usePromptProviderOptions } from "./usePromptProviderOptions";

vi.mock("../../../lib/queries", () => ({
	useByokLlmModels: () => ({
		data: { custom_text: [{ value: "own-model", label: "own-model" }] },
	}),
	useManagedModels: () => ({
		data: [
			{
				provider: "openai",
				id: "managed-model",
				display_name: "Managed model",
				capabilities: ["chat_completions", "transcription"],
			},
		],
	}),
	useFireworksModels: () => ({ data: [] }),
	useOllamaModels: () => ({ data: [] }),
}));

const options = {
	activeProfileId: "default",
	isDefaultScope: true,
	managedAccessEnabled: false,
	showAllProvidersAndModels: false,
	availableProviders: {
		stt: [
			{
				value: "custom_audio",
				label: "Audio server",
				is_local: false,
				models: ["own-transcriber"],
			},
		],
		llm: [
			{
				value: "custom_text",
				label: "Text server",
				is_local: false,
				models: ["own-model"],
			},
		],
	},
	settings: {
		stt_provider: "custom_audio",
		stt_model: "own-transcriber",
		llm_provider: "custom_text",
		llm_model: "own-model",
	} as AppSettings,
	profiles: [],
	localProfileSttProvider: null,
	localProfileSttModel: null,
	localProfileLlmProvider: null,
	localProfileLlmModel: null,
	localProfileQuickAskProvider: null,
	localProfileQuickAskModel: null,
	localProfileQuickReplaceProvider: null,
	localProfileQuickReplaceModel: null,
	effectiveRouterLlmProvider: null,
};

it("offers configured custom capabilities and model IDs for BYOK", () => {
	const result = usePromptProviderOptions(options);
	expect(result.sttCloudProviders).toEqual([
		{ value: "custom_audio", label: "Audio server" },
	]);
	expect(result.llmCloudProviders).toEqual([
		{ value: "custom_text", label: "Text server" },
	]);
	expect(result.sttModelOptions).toEqual([
		{ value: "own-transcriber", label: "own-transcriber" },
	]);
	expect(result.llmModelOptions).toEqual([
		{ value: "own-model", label: "own-model" },
	]);
	expect(result.quickAskModelOptions).toEqual(result.llmModelOptions);
	expect(result.quickReplaceModelOptions).toEqual(result.llmModelOptions);
});

it("keeps custom providers outside managed-only choices and reveals them explicitly", () => {
	const managed = usePromptProviderOptions({
		...options,
		managedAccessEnabled: true,
	});
	expect(
		managed.sttCloudProviders.some((p) => p.value.startsWith("custom_")),
	).toBe(false);
	expect(
		managed.llmCloudProviders.some((p) => p.value.startsWith("custom_")),
	).toBe(false);
	const all = usePromptProviderOptions({
		...options,
		managedAccessEnabled: true,
		showAllProvidersAndModels: true,
	});
	expect(all.sttCloudProviders).toContainEqual({
		value: "custom_audio",
		label: "Audio server",
	});
	expect(all.llmCloudProviders).toContainEqual({
		value: "custom_text",
		label: "Text server",
	});
	expect(all.sttModelOptions).toEqual([
		{ value: "own-transcriber", label: "own-transcriber" },
	]);
	expect(all.llmModelOptions).toEqual([
		{ value: "own-model", label: "own-model" },
	]);
});
