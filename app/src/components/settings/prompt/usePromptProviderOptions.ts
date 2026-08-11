import { API_KEYS } from "../../../lib/apiKeys";
import {
	LLM_MODELS,
	managedChatModelOptions,
	managedTranscriptionModelOptions,
	STT_MODELS,
} from "../../../lib/modelOptions";
import {
	useFireworksModels,
	useManagedModels,
	useOllamaModels,
} from "../../../lib/queries";
import type {
	AppSettings,
	RewriteProgramPromptProfile,
} from "../../../lib/tauri";
import { resolvePromptProfileFallbacks } from "./effectivePromptSettings";

// Keep this aligned with backend defaults (see Quick Replace config resolution in `src-tauri/src/lib.rs`).
const DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT =
	"You are an expert editor. Apply the user's instructions to the provided text.\n\nRules:\n- Return ONLY the updated text (no commentary, no code fences).\n- Preserve the original language and formatting unless instructed otherwise.";

type ProviderOption = { value: string; label: string; is_local: boolean };

type AvailableProviders = {
	stt: ProviderOption[];
	llm: ProviderOption[];
};

type UsePromptProviderOptionsOptions = {
	activeProfileId: string;
	isDefaultScope: boolean;
	managedAccessEnabled: boolean;
	showAllProvidersAndModels: boolean;
	availableProviders: AvailableProviders | undefined;
	settings: AppSettings | undefined;
	profiles: RewriteProgramPromptProfile[];
	localProfileSttProvider: string | null;
	localProfileSttModel: string | null;
	localProfileLlmProvider: string | null;
	localProfileLlmModel: string | null;
	localProfileQuickAskProvider: string | null;
	localProfileQuickAskModel: string | null;
	localProfileQuickReplaceProvider: string | null;
	localProfileQuickReplaceModel: string | null;
	effectiveRouterLlmProvider: string | null;
};

type PromptProviderOptions = {
	sttCloudProviders: Array<{ value: string; label: string }>;
	sttLocalProviders: Array<{ value: string; label: string }>;
	sttProviderOptions: Array<{
		group: string;
		items: Array<{ value: string; label: string }>;
	}>;
	llmCloudProviders: Array<{ value: string; label: string }>;
	llmLocalProviders: Array<{ value: string; label: string }>;
	llmProviderOptions: Array<{
		group: string;
		items: Array<{ value: string; label: string }>;
	}>;
	effectiveSttProvider: string | null;
	effectiveSttModel: string | null;
	effectiveLlmProvider: string | null;
	effectiveLlmModel: string | null;
	effectiveQuickAskProvider: string | null;
	effectiveQuickAskModel: string | null;
	effectiveQuickReplaceProvider: string | null;
	sttModelOptions: Array<{ value: string; label: string }>;
	llmModelOptions: Array<{ value: string; label: string }>;
	quickAskModelOptions: Array<{ value: string; label: string }>;
	quickReplaceModelOptions: Array<{ value: string; label: string }>;
	selectedSttModelForUi: string | null;
	selectedLlmModelForUi: string | null;
	selectedQuickAskModelForUi: string | null;
	selectedQuickReplaceModelForUi: string | null;
	defaultProfile: RewriteProgramPromptProfile | null;
	defaultQuickReplaceEnabled: boolean;
	defaultQuickReplaceProvider: string | null;
	defaultQuickReplaceModel: string | null;
	defaultQuickReplaceSystemPrompt: string;
	defaultRewriteIncludeClipboardContext: boolean;
	defaultQuickReplaceIncludeClipboardContext: boolean;
	defaultQuickAskIncludeClipboardContext: boolean;
	quickAskIncludeSelectedText: boolean;
	quickAskConversationHistoryEnabled: boolean;
	quickAskConversationHistoryCount: number;
	managedModels: ReturnType<typeof useManagedModels>["data"];
	fireworksModelsQuery: ReturnType<typeof useFireworksModels>;
	ollamaModelsQuery: ReturnType<typeof useOllamaModels>;
	getLlmModelOptionsForProvider: (
		provider: string | null,
	) => Array<{ value: string; label: string }>;
};

export function usePromptProviderOptions({
	activeProfileId,
	isDefaultScope,
	managedAccessEnabled,
	showAllProvidersAndModels,
	availableProviders,
	settings,
	profiles,
	localProfileSttProvider,
	localProfileSttModel,
	localProfileLlmProvider,
	localProfileLlmModel,
	localProfileQuickAskProvider,
	localProfileQuickAskModel,
	localProfileQuickReplaceProvider,
	localProfileQuickReplaceModel,
	effectiveRouterLlmProvider,
}: UsePromptProviderOptionsOptions): PromptProviderOptions {
	const managedModelsQuery = useManagedModels(managedAccessEnabled);
	const managedModels = managedModelsQuery.data ?? [];
	const managedCatalogReady = managedModels.length > 0;
	const managedProviderReady =
		managedAccessEnabled &&
		managedModels.some((model) =>
			model.capabilities.includes("chat_completions"),
		);
	const providerLabels = new Map(
		API_KEYS.map((provider) => [provider.id, provider.label]),
	);
	const allSttCloudProviders = API_KEYS.filter(
		(provider) => STT_MODELS[provider.id] !== undefined,
	).map((provider) => ({ value: provider.id, label: provider.label }));
	const allLlmCloudProviders = API_KEYS.filter(
		(provider) => LLM_MODELS[provider.id] !== undefined,
	).map((provider) => ({ value: provider.id, label: provider.label }));
	const managedSttProviders = Array.from(
		new Set(
			managedModels
				.filter((model) => model.capabilities.includes("transcription"))
				.map((model) => model.provider),
		),
	).map((provider) => ({
		value: provider,
		label: providerLabels.get(provider) ?? provider,
	}));

	// Provider dropdown options
	const configuredSttCloudProviders =
		availableProviders?.stt
			.filter((p) => !p.is_local)
			.map((p) => ({ value: p.value, label: p.label })) ?? [];
	const configuredSttLocalProviders =
		availableProviders?.stt
			.filter((p) => p.is_local)
			.map((p) => ({ value: p.value, label: p.label })) ?? [];
	const sttCloudProviders = managedAccessEnabled
		? showAllProvidersAndModels
			? allSttCloudProviders
			: managedCatalogReady
				? managedSttProviders
				: configuredSttCloudProviders
		: configuredSttCloudProviders;
	const sttLocalProviders =
		managedAccessEnabled && !showAllProvidersAndModels
			? []
			: configuredSttLocalProviders;
	const sttProviderOptions = [
		{ group: "Cloud", items: sttCloudProviders },
		{ group: "Local", items: sttLocalProviders },
	];

	const configuredLlmCloudProviders =
		availableProviders?.llm
			.filter((p) => !p.is_local)
			.map((p) => ({ value: p.value, label: p.label })) ?? [];
	const managedLlmProvider = managedProviderReady
		? [{ value: "managed", label: "Kolboo Managed" }]
		: [];
	const llmCloudProviders = managedAccessEnabled
		? showAllProvidersAndModels
			? [
					...managedLlmProvider,
					...allLlmCloudProviders.filter(
						(provider) => provider.value !== "managed",
					),
				]
			: managedCatalogReady
				? managedLlmProvider
				: configuredLlmCloudProviders
		: configuredLlmCloudProviders;
	const configuredLlmLocalProviders =
		availableProviders?.llm
			.filter((p) => p.is_local)
			.map((p) => ({ value: p.value, label: p.label })) ?? [];
	const llmLocalProviders =
		managedAccessEnabled && !showAllProvidersAndModels
			? []
			: configuredLlmLocalProviders;
	const llmProviderOptions = [
		{ group: "Cloud", items: llmCloudProviders },
		{ group: "Local", items: llmLocalProviders },
	];

	// Treat providers as "unselected" if they're not currently available in the
	// dropdown (e.g. on a fresh install before API keys are configured).
	const sttProviderValueSet = new Set(
		[...sttCloudProviders, ...sttLocalProviders].map((p) => p.value),
	);
	const llmProviderValueSet = new Set(
		[...llmCloudProviders, ...llmLocalProviders].map((p) => p.value),
	);

	const rawSttProvider =
		activeProfileId === "default"
			? (settings?.stt_provider ?? null)
			: (localProfileSttProvider ?? settings?.stt_provider ?? null);
	const effectiveSttProvider =
		rawSttProvider && sttProviderValueSet.has(rawSttProvider)
			? rawSttProvider
			: null;

	const effectiveSttModel =
		effectiveSttProvider === null
			? null
			: activeProfileId === "default"
				? (settings?.stt_model ?? null)
				: (localProfileSttModel ?? settings?.stt_model ?? null);

	const rawLlmProvider =
		activeProfileId === "default"
			? (settings?.llm_provider ?? null)
			: (localProfileLlmProvider ?? settings?.llm_provider ?? null);
	const effectiveLlmProvider =
		rawLlmProvider && llmProviderValueSet.has(rawLlmProvider)
			? rawLlmProvider
			: null;

	const rawQuickAskProvider =
		activeProfileId === "default"
			? (settings?.quick_ask_provider ?? settings?.llm_provider ?? null)
			: (localProfileQuickAskProvider ??
				settings?.quick_ask_provider ??
				settings?.llm_provider ??
				null);
	const effectiveQuickAskProvider =
		rawQuickAskProvider && llmProviderValueSet.has(rawQuickAskProvider)
			? rawQuickAskProvider
			: null;

	const defaultProfile = profiles.find((p) => p.id === "default") ?? null;
	const {
		baseQuickReplaceEnabled: defaultQuickReplaceEnabled,
		baseQuickReplaceProvider: defaultQuickReplaceProvider,
		baseQuickReplaceModel: defaultQuickReplaceModel,
		baseQuickReplaceSystemPrompt: defaultQuickReplaceSystemPromptValue,
		baseRewriteIncludeClipboardContext: defaultRewriteIncludeClipboardContext,
		baseQuickReplaceIncludeClipboardContext:
			defaultQuickReplaceIncludeClipboardContext,
		baseQuickAskIncludeClipboardContext: defaultQuickAskIncludeClipboardContext,
	} = resolvePromptProfileFallbacks({
		defaultProfile,
		settings: {
			quick_replace_enabled: settings?.quick_replace_enabled,
			llm_provider: settings?.llm_provider,
			llm_model: settings?.llm_model,
		},
		defaultQuickReplaceSystemPrompt: DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT,
	});

	const quickAskIncludeSelectedText =
		settings?.quick_ask_include_selected_text ?? false;

	const quickAskConversationHistoryEnabled =
		settings?.quick_ask_conversation_history_enabled ?? false;
	const quickAskConversationHistoryCount =
		settings?.quick_ask_conversation_history_count ?? 3;

	const rawQuickReplaceProvider =
		activeProfileId === "default"
			? (localProfileQuickReplaceProvider ?? settings?.llm_provider ?? null)
			: (localProfileQuickReplaceProvider ?? defaultQuickReplaceProvider);
	const effectiveQuickReplaceProvider =
		rawQuickReplaceProvider && llmProviderValueSet.has(rawQuickReplaceProvider)
			? rawQuickReplaceProvider
			: null;

	const effectiveQuickAskModel =
		effectiveQuickAskProvider === null
			? null
			: activeProfileId === "default"
				? (settings?.quick_ask_model ?? settings?.llm_model ?? null)
				: (localProfileQuickAskModel ??
					settings?.quick_ask_model ??
					settings?.llm_model ??
					null);

	const fireworksModelsQuery = useFireworksModels(
		effectiveLlmProvider === "fireworks" ||
			effectiveQuickAskProvider === "fireworks" ||
			effectiveQuickReplaceProvider === "fireworks" ||
			effectiveRouterLlmProvider === "fireworks",
	);

	const ollamaModelsQuery = useOllamaModels(
		effectiveLlmProvider === "ollama" ||
			effectiveQuickAskProvider === "ollama" ||
			effectiveQuickReplaceProvider === "ollama" ||
			effectiveRouterLlmProvider === "ollama",
	);

	const getLlmModelOptionsForProvider = (provider: string | null) => {
		if (!provider) return [];
		if (provider === "managed") {
			return managedChatModelOptions(managedModels);
		}
		if (provider === "fireworks") {
			const dynamic = fireworksModelsQuery.data;
			if (Array.isArray(dynamic) && dynamic.length > 0) return dynamic;
		}
		if (provider === "ollama") {
			const dynamic = ollamaModelsQuery.data;
			if (Array.isArray(dynamic) && dynamic.length > 0) return dynamic;
		}
		const configured = LLM_MODELS[provider] ?? [];
		const managedAliases = managedModels
			.filter((model) => {
				const byokProvider =
					model.provider === "google" ? "gemini" : model.provider;
				return (
					byokProvider === provider &&
					model.capabilities.includes("chat_completions")
				);
			})
			.map((model) => ({
				value:
					model.provider === "google" && model.id === "gemini-3-flash"
						? "models/gemini-3-flash-preview"
						: model.id,
				label: model.display_name,
			}));
		return [
			...configured,
			...managedAliases.filter(
				(alias) => !configured.some((option) => option.value === alias.value),
			),
		];
	};

	const sttModelOptions = effectiveSttProvider
		? managedAccessEnabled && managedCatalogReady && !showAllProvidersAndModels
			? managedTranscriptionModelOptions(managedModels, effectiveSttProvider)
			: (STT_MODELS[effectiveSttProvider] ?? [])
		: [];

	const llmModelOptions = getLlmModelOptionsForProvider(effectiveLlmProvider);
	const quickAskModelOptions = getLlmModelOptionsForProvider(
		effectiveQuickAskProvider,
	);
	const quickReplaceModelOptions = getLlmModelOptionsForProvider(
		effectiveQuickReplaceProvider,
	);

	const selectedSttModelForUi =
		sttModelOptions.length === 0
			? null
			: isDefaultScope
				? (settings?.stt_model ?? sttModelOptions[0]?.value ?? null)
				: localProfileSttModel;

	const selectedQuickAskModelForUi =
		quickAskModelOptions.length === 0
			? null
			: isDefaultScope
				? (settings?.quick_ask_model ??
					(effectiveQuickAskProvider === effectiveLlmProvider
						? settings?.llm_model
						: null) ??
					quickAskModelOptions[0]?.value ??
					null)
				: localProfileQuickAskModel;

	const selectedQuickReplaceModelForUi =
		quickReplaceModelOptions.length === 0
			? null
			: isDefaultScope
				? (localProfileQuickReplaceModel ??
					(effectiveQuickReplaceProvider === effectiveLlmProvider
						? settings?.llm_model
						: null) ??
					quickReplaceModelOptions[0]?.value ??
					null)
				: localProfileQuickReplaceModel;

	const effectiveLlmModel =
		effectiveLlmProvider === null
			? null
			: activeProfileId === "default"
				? (settings?.llm_model ?? null)
				: (localProfileLlmModel ?? settings?.llm_model ?? null);

	const selectedLlmModelForUi =
		llmModelOptions.length === 0
			? null
			: isDefaultScope
				? (settings?.llm_model ?? llmModelOptions[0]?.value ?? null)
				: localProfileLlmModel;

	return {
		sttCloudProviders,
		sttLocalProviders,
		sttProviderOptions,
		llmCloudProviders,
		llmLocalProviders,
		llmProviderOptions,
		effectiveSttProvider,
		effectiveSttModel,
		effectiveLlmProvider,
		effectiveLlmModel,
		effectiveQuickAskProvider,
		effectiveQuickAskModel,
		effectiveQuickReplaceProvider,
		sttModelOptions,
		llmModelOptions,
		quickAskModelOptions,
		quickReplaceModelOptions,
		selectedSttModelForUi,
		selectedLlmModelForUi,
		selectedQuickAskModelForUi,
		selectedQuickReplaceModelForUi,
		defaultProfile,
		defaultQuickReplaceEnabled,
		defaultQuickReplaceProvider,
		defaultQuickReplaceModel,
		defaultQuickReplaceSystemPrompt: defaultQuickReplaceSystemPromptValue,
		defaultRewriteIncludeClipboardContext,
		defaultQuickReplaceIncludeClipboardContext,
		defaultQuickAskIncludeClipboardContext,
		quickAskIncludeSelectedText,
		quickAskConversationHistoryEnabled,
		quickAskConversationHistoryCount,
		managedModels: managedModelsQuery.data,
		fireworksModelsQuery,
		ollamaModelsQuery,
		getLlmModelOptionsForProvider,
	};
}
