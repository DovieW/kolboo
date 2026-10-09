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

it("displays custom speech names while retaining the raw ID", () => {
	const result = usePromptProviderOptions({
		...options,
		availableProviders: {
			...options.availableProviders,
			stt: [
				{
					...options.availableProviders.stt[0],
					value: "custom_audio",
					label: "Audio server",
					is_local: false,
					models: ["own-transcriber"],
					model_names: { "own-transcriber": "Speech model" },
				},
			],
		},
	});
	expect(result.sttModelOptions).toEqual([
		{ value: "own-transcriber", label: "Speech model" },
	]);
	const missing = usePromptProviderOptions({
		...options,
		availableProviders: undefined,
	});
	expect(missing.sttModelOptions).toEqual([]);
	const discovery = usePromptProviderOptions({
		...options,
		managedAccessEnabled: true,
		showAllProvidersAndModels: true,
		availableProviders: undefined,
		settings: { ...options.settings, stt_provider: "groq", stt_model: null },
	});
	expect(discovery.effectiveSttProvider).toBe("groq");
	expect(discovery.sttModelOptions.map((model) => model.value)).toContain(
		"whisper-large-v3-turbo",
	);
	const unconfigured = usePromptProviderOptions({
		...options,
		managedAccessEnabled: true,
		showAllProvidersAndModels: true,
		settings: { ...options.settings, stt_provider: "groq", stt_model: null },
	});
	expect(unconfigured.sttModelOptions).toEqual(discovery.sttModelOptions);
	const noModels = usePromptProviderOptions({
		...options,
		availableProviders: {
			...options.availableProviders,
			stt: [
				{
					value: "custom_audio",
					label: "Audio server",
					is_local: false,
					models: null,
				},
			],
		},
	});
	expect(noModels.effectiveSttProvider).toBe("custom_audio");
	expect(noModels.sttModelOptions).toEqual([]);
	expect(noModels.selectedSttModelForUi).toBeNull();
});

it("does not present the first catalog entry as an implicitly selected default", () => {
	const result = usePromptProviderOptions({
		...options,
		settings: { ...options.settings, stt_model: null, llm_model: null },
	});
	expect(result.sttModelOptions).toHaveLength(1);
	expect(result.llmModelOptions).toHaveLength(1);
	expect(result.selectedSttModelForUi).toBeNull();
	expect(result.selectedLlmModelForUi).toBeNull();
	expect(result.selectedQuickAskModelForUi).toBeNull();
	expect(result.selectedQuickReplaceModelForUi).toBeNull();
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

it("preserves saved IDs across catalog refreshes in all BYOK prompt flows", () => {
	const settings = {
		...options.settings,
		stt_model: "old-stt",
		llm_model: "old-llm",
		quick_ask_model: "old-ask",
	};
	const result = usePromptProviderOptions({
		...options,
		settings,
		localProfileQuickReplaceModel: "old-replace",
	});
	expect(result.sttModelOptions).toContainEqual({
		value: "old-stt",
		label: "old-stt (saved selection)",
	});
	expect(result.llmModelOptions).toContainEqual({
		value: "old-llm",
		label: "old-llm (saved selection)",
	});
	expect(result.quickAskModelOptions).toContainEqual({
		value: "old-ask",
		label: "old-ask (saved selection)",
	});
	expect(result.quickReplaceModelOptions).toContainEqual({
		value: "old-replace",
		label: "old-replace (saved selection)",
	});
	expect(result.selectedLlmModelForUi).toBe("old-llm");
	expect(settings.llm_model).toBe("old-llm");
	const profile = usePromptProviderOptions({
		...options,
		activeProfileId: "profile",
		isDefaultScope: false,
		localProfileSttModel: "profile-stt",
		localProfileLlmModel: "profile-llm",
		localProfileQuickAskModel: "profile-ask",
		localProfileQuickReplaceModel: "profile-replace",
	});
	expect(profile.llmModelOptions.at(-1)?.value).toBe("profile-llm");
	expect(profile.sttModelOptions.at(-1)?.value).toBe("profile-stt");
	expect(profile.quickAskModelOptions.at(-1)?.value).toBe("profile-ask");
	expect(profile.quickReplaceModelOptions.at(-1)?.value).toBe(
		"profile-replace",
	);
	const inherited = usePromptProviderOptions({
		...options,
		activeProfileId: "profile",
		isDefaultScope: false,
	});
	expect(inherited.selectedLlmModelForUi).toBeNull();
	const empty = usePromptProviderOptions({
		...options,
		settings: undefined,
		availableProviders: undefined,
	});
	expect(empty.llmModelOptions).toEqual([]);
	expect(empty.sttModelOptions).toEqual([]);
});
