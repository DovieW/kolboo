import { API_KEYS } from "../../../lib/apiKeys";
import { resolveModelPreference } from "../../../lib/modelSelection";
import {
	configuredProviderModelOptions,
	managedChatModelOptions,
	managedTranscriptionModelOptions,
	retainSavedModelOption,
	STT_MODELS,
} from "../../../lib/modelOptions";
import {
	useByokLlmModels,
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

type ProviderOption = {
	value: string;
	label: string;
	is_local: boolean;
	models?: string[] | null;
	model_names?: Record<string, string> | null;
};

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
	const byokLlmModelsQuery = useByokLlmModels(
		!managedAccessEnabled || showAllProvidersAndModels,
	);
	const byokLlmModels = byokLlmModelsQuery.data;
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
		(provider) => byokLlmModels[provider.id] !== undefined,
	).map((provider) => ({ value: provider.id, label: provider.label }));
	for (const p of availableProviders?.stt ?? [])
		if (p.value.startsWith("custom_"))
			allSttCloudProviders.push({ value: p.value, label: p.label });
	for (const p of availableProviders?.llm ?? [])
		if (p.value.startsWith("custom_"))
			allLlmCloudProviders.push({ value: p.value, label: p.label });
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

	const rawSttProvider =
		activeProfileId === "default"
			? (settings?.stt_provider ?? null)
			: (localProfileSttProvider ?? settings?.stt_provider ?? null);
	const profile = profiles.find(profile => profile.id === activeProfileId && profile.id !== "default");
	const sttPreference = resolveModelPreference("transcription", settings, profile);
	const rewritePreference = resolveModelPreference("rewrite", settings, profile);
	const askPreference = resolveModelPreference("quick_ask", settings, profile);
	const replacePreference = resolveModelPreference("quick_replace", settings, profiles.find(profile => profile.id === activeProfileId));
	const effectiveSttProvider = sttPreference.selection?.provider_id ?? rawSttProvider;

	const effectiveSttModel = sttPreference.selection?.model_id ?? (
		effectiveSttProvider === null
			? null
			: activeProfileId === "default"
				? (settings?.stt_model ?? null)
				: (localProfileSttModel ?? settings?.stt_model ?? null));

	const rawLlmProvider =
		activeProfileId === "default"
			? (settings?.llm_provider ?? null)
			: (localProfileLlmProvider ?? settings?.llm_provider ?? null);
	const effectiveLlmProvider = rewritePreference.selection?.provider_id ?? rawLlmProvider;

	const rawQuickAskProvider =
		activeProfileId === "default"
			? (settings?.quick_ask_provider ?? settings?.llm_provider ?? null)
			: (localProfileQuickAskProvider ??
				settings?.quick_ask_provider ??
				settings?.llm_provider ??
				null);
	const effectiveQuickAskProvider = askPreference.selection?.provider_id ?? rawQuickAskProvider;

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
	const effectiveQuickReplaceProvider = replacePreference.selection?.provider_id ?? rawQuickReplaceProvider;

	const effectiveQuickAskModel = askPreference.selection?.model_id ?? (
		effectiveQuickAskProvider === null
			? null
			: activeProfileId === "default"
				? (settings?.quick_ask_model ?? settings?.llm_model ?? null)
				: (localProfileQuickAskModel ??
					settings?.quick_ask_model ??
					settings?.llm_model ??
					null));

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
		const configured = byokLlmModels[provider] ?? [];
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

	const currentSttModelOptions = effectiveSttProvider
		? managedAccessEnabled && managedCatalogReady && !showAllProvidersAndModels
			? managedTranscriptionModelOptions(managedModels, effectiveSttProvider)
			: (configuredProviderModelOptions(
					availableProviders?.stt.find((p) => p.value === effectiveSttProvider),
				) ??
				STT_MODELS[effectiveSttProvider] ??
				[])
		: [];

	// Catalog refreshes must not blank out or silently replace a saved model.
	// Managed-only availability remains authoritative; saved BYOK IDs are not
	// permission to enable a model through the managed service.
	const retainSaved =
		managedAccessEnabled && !showAllProvidersAndModels
			? (
					options: Array<{ value: string; label: string }>,
					_model: string | null,
				) => options
			: retainSavedModelOption;
	const sttModelOptions = retainSaved(
		currentSttModelOptions,
		effectiveSttModel,
	);
	const llmModelOptions = retainSaved(
		getLlmModelOptionsForProvider(effectiveLlmProvider),
		isDefaultScope ? (settings?.llm_model ?? null) : localProfileLlmModel,
	);
	const quickAskModelOptions = retainSaved(
		getLlmModelOptionsForProvider(effectiveQuickAskProvider),
		effectiveQuickAskModel,
	);
	const quickReplaceModelOptions = retainSaved(
		getLlmModelOptionsForProvider(effectiveQuickReplaceProvider),
		isDefaultScope
			? (localProfileQuickReplaceModel ?? settings?.llm_model ?? null)
			: localProfileQuickReplaceModel,
	);

	const selectedSttModelForUi = sttPreference.selection?.model_id ?? (
		sttModelOptions.length === 0
			? null
			: isDefaultScope
				? (settings?.stt_model ?? null)
				: localProfileSttModel);

	const selectedQuickAskModelForUi = askPreference.selection?.model_id ?? (
		quickAskModelOptions.length === 0
			? null
			: isDefaultScope
				? (settings?.quick_ask_model ??
					(effectiveQuickAskProvider === effectiveLlmProvider
						? settings?.llm_model
						: null) ??
					null)
				: localProfileQuickAskModel);

	const selectedQuickReplaceModelForUi = replacePreference.selection?.model_id ?? (
		quickReplaceModelOptions.length === 0
			? null
			: isDefaultScope
				? (localProfileQuickReplaceModel ??
					(effectiveQuickReplaceProvider === effectiveLlmProvider
						? settings?.llm_model
						: null) ??
					null)
				: localProfileQuickReplaceModel);

	const effectiveLlmModel = rewritePreference.selection?.model_id ?? (
		effectiveLlmProvider === null
			? null
			: activeProfileId === "default"
				? (settings?.llm_model ?? null)
				: (localProfileLlmModel ?? settings?.llm_model ?? null));

	const selectedLlmModelForUi = rewritePreference.selection?.model_id ?? (
		llmModelOptions.length === 0
			? null
			: isDefaultScope
				? (settings?.llm_model ?? null)
				: localProfileLlmModel);

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
