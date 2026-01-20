import { LLM_MODELS, STT_MODELS } from "../../../lib/modelOptions";
import { useFireworksModels, useOllamaModels } from "../../../lib/queries";
import type {
	AppSettings,
	RewriteProgramPromptProfile,
} from "../../../lib/tauri";

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
	fireworksModelsQuery: ReturnType<typeof useFireworksModels>;
	ollamaModelsQuery: ReturnType<typeof useOllamaModels>;
	getLlmModelOptionsForProvider: (
		provider: string | null,
	) => Array<{ value: string; label: string }>;
};

export function usePromptProviderOptions({
	activeProfileId,
	isDefaultScope,
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
	// Provider dropdown options
	const sttCloudProviders =
		availableProviders?.stt
			.filter((p) => !p.is_local)
			.map((p) => ({ value: p.value, label: p.label })) ?? [];
	const sttLocalProviders =
		availableProviders?.stt
			.filter((p) => p.is_local)
			.map((p) => ({ value: p.value, label: p.label })) ?? [];
	const sttProviderOptions = [
		{ group: "Cloud", items: sttCloudProviders },
		{ group: "Local", items: sttLocalProviders },
	];

	const llmCloudProviders =
		availableProviders?.llm
			.filter((p) => !p.is_local)
			.map((p) => ({ value: p.value, label: p.label })) ?? [];
	const llmLocalProviders =
		availableProviders?.llm
			.filter((p) => p.is_local)
			.map((p) => ({ value: p.value, label: p.label })) ?? [];
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

	const defaultQuickReplaceEnabled =
		typeof defaultProfile?.quick_replace_enabled === "boolean"
			? defaultProfile.quick_replace_enabled
			: (settings?.quick_replace_enabled ?? false);
	const defaultQuickReplaceProvider =
		defaultProfile?.quick_replace_provider ?? settings?.llm_provider ?? null;
	const defaultQuickReplaceModel =
		defaultProfile?.quick_replace_model ?? settings?.llm_model ?? null;
	const defaultQuickReplaceSystemPromptValue =
		defaultProfile?.quick_replace_system_prompt ??
		DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT;

	const defaultRewriteIncludeClipboardContext =
		typeof defaultProfile?.rewrite_include_clipboard_context === "boolean"
			? defaultProfile.rewrite_include_clipboard_context
			: false;
	const defaultQuickReplaceIncludeClipboardContext =
		typeof defaultProfile?.quick_replace_include_clipboard_context === "boolean"
			? defaultProfile.quick_replace_include_clipboard_context
			: false;
	const defaultQuickAskIncludeClipboardContext =
		typeof defaultProfile?.quick_ask_include_clipboard_context === "boolean"
			? defaultProfile.quick_ask_include_clipboard_context
			: false;

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
		if (provider === "fireworks") {
			const dynamic = fireworksModelsQuery.data;
			if (Array.isArray(dynamic) && dynamic.length > 0) return dynamic;
		}
		if (provider === "ollama") {
			const dynamic = ollamaModelsQuery.data;
			if (Array.isArray(dynamic) && dynamic.length > 0) return dynamic;
		}
		return LLM_MODELS[provider] ?? [];
	};

	const sttModelOptions = effectiveSttProvider
		? (STT_MODELS[effectiveSttProvider] ?? [])
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
		fireworksModelsQuery,
		ollamaModelsQuery,
		getLlmModelOptionsForProvider,
	};
}
