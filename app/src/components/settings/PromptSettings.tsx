import {
	Accordion,
	Button,
	Group,
	Loader,
	Select,
	Switch,
	Text,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useEffect, useMemo, useRef, useState } from "react";
import { formatErrorMessage } from "../../lib/formatError";
import {
	EMBEDDING_MODELS,
	isRealtimeSttModel,
	type ModelOption,
	managedModelByokTarget,
} from "../../lib/modelOptions";
import {
	useAvailableProviders,
	useDefaultSections,
	useHasLastAudioForSttTest,
	useIterateRewritePrompt,
	useLicenseAuthContext,
	useModelPricing,
	useSettings,
	useTestLlmRewrite,
	useTestRewriteWithPrompt,
	useTestSttTranscribeLastAudio,
	useUpdateAnthropicThinkingBudget,
	useUpdateCleanupPromptSections,
	useUpdateGeminiThinkingBudget,
	useUpdateGeminiThinkingLevel,
	useUpdateLLMModel,
	useUpdateLLMProvider,
	useUpdateOpenAiReasoningEffort,
	useUpdateQuickAskAnthropicThinkingBudget,
	useUpdateQuickAskConversationHistoryCount,
	useUpdateQuickAskConversationHistoryEnabled,
	useUpdateQuickAskDismissMode,
	useUpdateQuickAskGeminiThinkingBudget,
	useUpdateQuickAskGeminiThinkingLevel,
	useUpdateQuickAskIncludeSelectedText,
	useUpdateQuickAskModel,
	useUpdateQuickAskOpenAiReasoningEffort,
	useUpdateQuickAskProvider,
	useUpdateQuickAskSystemPrompt,
	useUpdateRewriteLlmEnabled,
	useUpdateRewriteProgramPromptProfiles,
	useUpdateSTTLanguage,
	useUpdateSTTModel,
	useUpdateSTTProvider,
	useUpdateSTTTimeout,
	useUpdateSTTTranscriptionPrompt,
	useUpdateSTTUseManagedInference,
} from "../../lib/queries";
import {
	DEFAULT_STT_LANGUAGE,
	STT_LANGUAGE_OPTIONS,
} from "../../lib/sttLanguages";
import {
	type ActiveWindowOcrMode,
	type AppSettings,
	type CleanupPromptSections,
	type CleanupPromptSectionsOverride,
	hasManagedInferenceAccess,
	type IntentRouterSettings,
	type QuickAskDismissMode,
	type RewritePreset,
	type RewriteProgramPromptProfile,
	tauriAPI,
} from "../../lib/tauri";
import { PresetEditorModal } from "./prompt/PresetEditorModal";
import { PromptIntentRouterSection } from "./prompt/PromptIntentRouterSection";
import { PromptSettingsModals } from "./prompt/PromptSettingsModals";
import { QuickAskPanel } from "./prompt/QuickAskPanel";
import { RewriteSettingsSection } from "./prompt/RewriteSettingsSection";
import {
	formatUsdRateFromMicros,
	isGeminiThinkingLevel,
	isOpenAiReasoningEffort,
	normalizeRouter,
} from "./prompt/settingsUtils";
import { TranscribeSettingsSection } from "./prompt/TranscribeSettingsSection";
import {
	EDIT_DEFAULT_PRESET,
	usePresetManagement,
} from "./prompt/usePresetManagement";
import { usePromptLabState } from "./prompt/usePromptLabState";
import { usePromptProviderOptions } from "./prompt/usePromptProviderOptions";
import { usePromptSettingsProfileState } from "./prompt/usePromptSettingsProfileState";
import { usePromptSettingsTests } from "./prompt/usePromptSettingsTests";
import { useRewriteSettingsHandlers } from "./prompt/useRewriteSettingsHandlers";
import {
	type SectionKey,
	useSectionManagement,
} from "./prompt/useSectionManagement";
import { useSttSettingsHandlers } from "./prompt/useSttSettingsHandlers";
import {
	ANTHROPIC_THINKING_LEVEL_BUDGETS,
	formatThinkingBudgetShort,
	openAiDefaultReasoningEffortForModel,
	SELECT_DEFAULT,
	useThinkingOptions,
} from "./prompt/useThinkingOptions";
import { QuickReplaceSettings } from "./QuickReplaceSettings";
import { RewritePromptLabModal } from "./RewritePromptLabModal";
import { SettingsRow } from "./SettingsRow";

const INHERIT_TOOLTIP = "Inheriting from Default profile";

// (debug logging removed)

const DEFAULT_SECTIONS: CleanupPromptSections = {
	system: { content: null },
};

// NOTE: This timeout is used by the Rust pipeline as a transcription request timeout.
// Keep this default aligned with backend fallbacks so "unset" settings don't lie.
const DEFAULT_STT_TIMEOUT = 10;

// Keep this aligned with the backend default seeding in `ensure_default_settings(...)`.
const DEFAULT_QUICK_ASK_SYSTEM_PROMPT =
	"Try to answer the question in a single word, sentence or paragraph when possible. Use markdown for formatting when necessary.";

// Keep this aligned with backend defaults (see Quick Replace config resolution in `src-tauri/src/lib.rs`).
const DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT =
	"You are an expert editor. Apply the user's instructions to the provided text.\n\nRules:\n- Return ONLY the updated text (no commentary, no code fences).\n- Preserve the original language and formatting unless instructed otherwise.";

function _createId(): string {
	// `crypto.randomUUID()` is available in modern browsers; keep a fallback for safety.
	// This only needs to be unique enough for local settings.
	return (
		globalThis.crypto?.randomUUID?.() ??
		`id_${Date.now()}_${Math.random().toString(16).slice(2)}`
	);
}

export function PromptSettings({
	editingProfileId,
}: {
	editingProfileId?: string;
}) {
	const activeProfileId = editingProfileId ?? "default";
	const isDefaultScope = activeProfileId === "default";

	const { data: settings, isLoading: isLoadingSettings } = useSettings();
	const { data: defaultSections, isLoading: isLoadingDefaultSections } =
		useDefaultSections();
	const { data: availableProviders, isLoading: isLoadingProviders } =
		useAvailableProviders();
	const { data: licenseAuthContext } = useLicenseAuthContext();
	const managedAccessEnabled = hasManagedInferenceAccess(licenseAuthContext);
	const [showAllProvidersAndModels, setShowAllProvidersAndModels] =
		useState(false);
	const updateCleanupPromptSections = useUpdateCleanupPromptSections();
	const updateRewriteLlmEnabled = useUpdateRewriteLlmEnabled();
	const updateRewriteProgramPromptProfiles =
		useUpdateRewriteProgramPromptProfiles();
	const testLlmRewrite = useTestLlmRewrite();
	const iterateRewritePrompt = useIterateRewritePrompt();
	const testRewriteWithPrompt = useTestRewriteWithPrompt();
	const testSttLastAudio = useTestSttTranscribeLastAudio();
	const { data: hasLastAudioForSttTest } = useHasLastAudioForSttTest();

	// Default profile (global) provider settings
	const updateSTTProvider = useUpdateSTTProvider();
	const updateSTTModel = useUpdateSTTModel();
	const updateSTTUseManagedInference = useUpdateSTTUseManagedInference();
	const updateSTTLanguage = useUpdateSTTLanguage();
	const updateSTTTranscriptionPrompt = useUpdateSTTTranscriptionPrompt();
	const updateLLMProvider = useUpdateLLMProvider();
	const updateLLMModel = useUpdateLLMModel();
	const updateOpenAiReasoningEffort = useUpdateOpenAiReasoningEffort();
	const updateAnthropicThinkingBudget = useUpdateAnthropicThinkingBudget();
	const updateGeminiThinkingBudget = useUpdateGeminiThinkingBudget();
	const updateGeminiThinkingLevel = useUpdateGeminiThinkingLevel();
	const _updateSTTTimeout = useUpdateSTTTimeout();

	const updateQuickAskProvider = useUpdateQuickAskProvider();
	const updateQuickAskModel = useUpdateQuickAskModel();
	const updateQuickAskSystemPrompt = useUpdateQuickAskSystemPrompt();
	const updateQuickAskDismissMode = useUpdateQuickAskDismissMode();
	const updateQuickAskIncludeSelectedText =
		useUpdateQuickAskIncludeSelectedText();
	const updateQuickAskConversationHistoryEnabled =
		useUpdateQuickAskConversationHistoryEnabled();
	const updateQuickAskConversationHistoryCount =
		useUpdateQuickAskConversationHistoryCount();
	const updateQuickAskOpenAiReasoningEffort =
		useUpdateQuickAskOpenAiReasoningEffort();
	const updateQuickAskAnthropicThinkingBudget =
		useUpdateQuickAskAnthropicThinkingBudget();
	const updateQuickAskGeminiThinkingBudget =
		useUpdateQuickAskGeminiThinkingBudget();
	const updateQuickAskGeminiThinkingLevel =
		useUpdateQuickAskGeminiThinkingLevel();

	const profiles: RewriteProgramPromptProfile[] =
		settings?.rewrite_program_prompt_profiles ?? [];

	const activeProfile: RewriteProgramPromptProfile | null = useMemo(() => {
		const found = profiles.find((p) => p.id === activeProfileId) ?? null;
		if (found) return found;

		// Backward compatible: if Default hasn't been migrated into the profile list yet,
		// provide an in-memory fallback so the UI can still render.
		if (activeProfileId === "default") {
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

				context_grab_method: null,

				rewrite_include_clipboard_context: null,
				quick_replace_include_clipboard_context: null,
				quick_ask_include_clipboard_context: null,
				quick_ask_dismiss_mode: null,

				rewrite_active_window_ocr_mode: null,
				quick_replace_active_window_ocr_mode: null,
				quick_ask_active_window_ocr_mode: null,

				quick_replace_enabled: null,
				quick_replace_provider: null,
				quick_replace_model: null,
				quick_replace_system_prompt: null,
			};
		}

		return null;
	}, [profiles, activeProfileId]);

	const activeProfileLabel = useMemo(() => {
		if (activeProfileId === "default") return "Default";
		const name = activeProfile?.name?.trim();
		return name ? name : activeProfileId;
	}, [activeProfileId, activeProfile?.name]);

	// OCR provider status (availability is gated in the backend based on base URL + auth).
	// While loading, default to "available" to avoid flicker/false disabling.
	const ocrProviderAvailable = availableProviders?.ocr?.available ?? true;
	const ocrProviderUnavailableReason = availableProviders?.ocr?.reason ?? null;

	const defaultRewriteEnabled = settings?.rewrite_llm_enabled ?? false;

	const {
		localProfileSttProvider,
		setLocalProfileSttProvider,
		localProfileSttModel,
		setLocalProfileSttModel,
		localProfileSttLanguage,
		setLocalProfileSttLanguage,
		localProfileLlmProvider,
		setLocalProfileLlmProvider,
		localProfileLlmModel,
		setLocalProfileLlmModel,
		localProfileQuickAskProvider,
		setLocalProfileQuickAskProvider,
		localProfileQuickAskModel,
		setLocalProfileQuickAskModel,
		localProfileQuickAskDismissMode,
		setLocalProfileQuickAskDismissMode,
		localQuickAskSystemPrompt,
		setLocalQuickAskSystemPrompt,
		localProfileQuickReplaceEnabled,
		setLocalProfileQuickReplaceEnabled,
		localProfileQuickReplaceProvider,
		setLocalProfileQuickReplaceProvider,
		localProfileQuickReplaceModel,
		setLocalProfileQuickReplaceModel,
		localQuickReplaceSystemPrompt,
		setLocalQuickReplaceSystemPrompt,
		localProfileRewriteIncludeClipboardContext,
		setLocalProfileRewriteIncludeClipboardContext,
		localProfileQuickReplaceIncludeClipboardContext,
		setLocalProfileQuickReplaceIncludeClipboardContext,
		localProfileQuickAskIncludeClipboardContext,
		setLocalProfileQuickAskIncludeClipboardContext,
		localProfileRewriteActiveWindowOcrMode,
		setLocalProfileRewriteActiveWindowOcrMode,
		localProfileQuickReplaceActiveWindowOcrMode,
		setLocalProfileQuickReplaceActiveWindowOcrMode,
		localProfileQuickAskActiveWindowOcrMode,
		setLocalProfileQuickAskActiveWindowOcrMode,
		localProfileOpenAiReasoningEffort,
		setLocalProfileOpenAiReasoningEffort,
		localProfileGeminiThinkingLevel,
		setLocalProfileGeminiThinkingLevel,
		localProfileGeminiThinkingBudget,
		setLocalProfileGeminiThinkingBudget,
		localProfileAnthropicThinkingBudget,
		setLocalProfileAnthropicThinkingBudget,
		localProfileRewriteEnabled,
		setLocalProfileRewriteEnabled,
		localProfileSttTimeout,
		setLocalProfileSttTimeout,
		localProfileQuickAskOpenAiReasoningEffort,
		setLocalProfileQuickAskOpenAiReasoningEffort,
		localProfileQuickAskGeminiThinkingLevel,
		setLocalProfileQuickAskGeminiThinkingLevel,
		localProfileQuickAskGeminiThinkingBudget,
		setLocalProfileQuickAskGeminiThinkingBudget,
		localProfileQuickAskAnthropicThinkingBudget,
		setLocalProfileQuickAskAnthropicThinkingBudget,
		sttProviderInheriting,
		setSttProviderInheriting,
		sttModelInheriting,
		setSttModelInheriting,
		sttLanguageInheriting,
		setSttLanguageInheriting,
		sttTimeoutInheriting,
		setSttTimeoutInheriting,
		llmProviderInheriting,
		setLlmProviderInheriting,
		llmModelInheriting,
		setLlmModelInheriting,
		rewriteEnabledInheriting,
		setRewriteEnabledInheriting,
		rewriteIncludeClipboardContextInheriting,
		setRewriteIncludeClipboardContextInheriting,
		quickReplaceIncludeClipboardContextInheriting,
		setQuickReplaceIncludeClipboardContextInheriting,
		quickAskIncludeClipboardContextInheriting,
		setQuickAskIncludeClipboardContextInheriting,
		rewriteActiveWindowOcrModeInheriting,
		setRewriteActiveWindowOcrModeInheriting,
		quickReplaceActiveWindowOcrModeInheriting,
		setQuickReplaceActiveWindowOcrModeInheriting,
		quickAskActiveWindowOcrModeInheriting,
		setQuickAskActiveWindowOcrModeInheriting,
		openAiReasoningEffortInheriting,
		setOpenAiReasoningEffortInheriting,
		geminiThinkingLevelInheriting,
		setGeminiThinkingLevelInheriting,
		geminiThinkingBudgetInheriting,
		setGeminiThinkingBudgetInheriting,
		anthropicThinkingBudgetInheriting,
		setAnthropicThinkingBudgetInheriting,
		quickAskProviderInheriting,
		setQuickAskProviderInheriting,
		quickAskModelInheriting,
		setQuickAskModelInheriting,
		quickAskSystemPromptInheriting,
		setQuickAskSystemPromptInheriting,
		quickAskDismissModeInheriting,
		setQuickAskDismissModeInheriting,
		quickReplaceEnabledInheriting,
		setQuickReplaceEnabledInheriting,
		quickReplaceProviderInheriting,
		setQuickReplaceProviderInheriting,
		quickReplaceModelInheriting,
		setQuickReplaceModelInheriting,
		quickReplaceSystemPromptInheriting,
		setQuickReplaceSystemPromptInheriting,
		quickAskOpenAiReasoningEffortInheriting,
		setQuickAskOpenAiReasoningEffortInheriting,
		quickAskGeminiThinkingLevelInheriting,
		setQuickAskGeminiThinkingLevelInheriting,
		quickAskGeminiThinkingBudgetInheriting,
		setQuickAskGeminiThinkingBudgetInheriting,
		quickAskAnthropicThinkingBudgetInheriting,
		setQuickAskAnthropicThinkingBudgetInheriting,
	} = usePromptSettingsProfileState({
		activeProfileId,
		activeProfile,
		profiles,
		settings,
		defaultRewriteEnabled,
		defaultQuickReplaceSystemPrompt: DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT,
		defaultSttTimeout: DEFAULT_STT_TIMEOUT,
	});

	const {
		rewriteTestInput,
		setRewriteTestInput,
		rewriteTestOutput,
		rewriteTestError,
		rewriteTestDurationMs,
		runRewriteTest,
		sttTestOutput,
		sttTestError,
		sttTestDurationMs,
		handleRunSttTest,
	} = usePromptSettingsTests({
		activeProfileId,
		errorToMessage: formatErrorMessage,
		testLlmRewrite,
		testRewriteWithPrompt,
		testSttLastAudio,
	});

	const [isCachingRouterEmbeddings, setIsCachingRouterEmbeddings] =
		useState(false);

	const handleConfirmResetDialog = () => {
		const confirm = resetDialog?.onConfirm;
		setResetDialog(null);
		confirm?.();
	};

	const effectiveRouter: IntentRouterSettings | null = useMemo(() => {
		if (!activeProfile) return null;
		return normalizeRouter(activeProfile.router);
	}, [activeProfile]);

	const {
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
		defaultQuickReplaceEnabled,
		defaultQuickReplaceProvider,
		defaultQuickReplaceModel,
		defaultQuickReplaceSystemPrompt,
		defaultRewriteIncludeClipboardContext,
		defaultQuickReplaceIncludeClipboardContext,
		defaultQuickAskIncludeClipboardContext,
		quickAskIncludeSelectedText,
		quickAskConversationHistoryEnabled,
		quickAskConversationHistoryCount,
		managedModels,
		ollamaModelsQuery,
		getLlmModelOptionsForProvider,
	} = usePromptProviderOptions({
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
		effectiveRouterLlmProvider: effectiveRouter?.llm_provider ?? null,
	});

	const defaultQuickAskDismissMode: QuickAskDismissMode = useMemo(() => {
		const defaultProfile = profiles.find((p) => p.id === "default");
		if (defaultProfile?.quick_ask_dismiss_mode === "auto") return "auto";
		if (defaultProfile?.quick_ask_dismiss_mode === "manual") return "manual";
		return settings?.quick_ask_dismiss_mode ?? "manual";
	}, [profiles, settings?.quick_ask_dismiss_mode]);

	const presetRuntimeSettings = useMemo(
		() =>
			settings
				? ({
						stt_provider: settings.stt_provider,
						stt_model: settings.stt_model,
						stt_language: settings.stt_language,
						stt_timeout_seconds: settings.stt_timeout_seconds,
						llm_provider: settings.llm_provider,
						llm_model: settings.llm_model,
					} satisfies Partial<
						Pick<
							AppSettings,
							| "stt_provider"
							| "stt_model"
							| "stt_language"
							| "stt_timeout_seconds"
							| "llm_provider"
							| "llm_model"
						>
					>)
				: undefined,
		[settings],
	);

	const saveRouter = (router: IntentRouterSettings | null) => {
		if (!activeProfile) return;
		saveProfileMetadata({ router });
	};

	const [localSttTranscriptionPrompt, setLocalSttTranscriptionPrompt] =
		useState<string>("");

	const [quickAskTestInput, setQuickAskTestInput] = useState<string>("");
	const [quickAskTestOutput, setQuickAskTestOutput] = useState<string>("");
	const [quickAskTestError, setQuickAskTestError] = useState<string>("");
	const [quickAskTestDurationMs, setQuickAskTestDurationMs] = useState<
		number | null
	>(null);
	const [quickAskTestPending, setQuickAskTestPending] = useState(false);
	const quickAskTestStartRef = useRef<number | null>(null);

	const [resetDialog, setResetDialog] = useState<null | {
		title: string;
		onConfirm: () => void;
	}>(null);

	const openDisableOverrideDialog = (args: {
		title: string;
		onConfirm: () => void;
	}) => {
		setResetDialog(args);
	};

	const handleRewriteActiveWindowOcrModeChange = (
		mode: ActiveWindowOcrMode,
	) => {
		if (!isDefaultScope) setRewriteActiveWindowOcrModeInheriting(false);
		setLocalProfileRewriteActiveWindowOcrMode(mode);
		saveProfileMetadata({ rewrite_active_window_ocr_mode: mode });
	};

	const handleDisableRewriteActiveWindowOcrModeOverride = () => {
		openDisableOverrideDialog({
			title: "Disable Rewrite Active Window OCR override?",
			onConfirm: () => {
				setRewriteActiveWindowOcrModeInheriting(true);
				saveProfileMetadata({ rewrite_active_window_ocr_mode: null });
			},
		});
	};

	const isOpenAiStt = effectiveSttProvider === "openai";
	const isAquavoiceStt = effectiveSttProvider === "aquavoice";
	const isGroqStt = effectiveSttProvider === "groq";
	const isWhisperServerStt = effectiveSttProvider === "whisper-server";
	const isWhisper1Selected = isOpenAiStt && effectiveSttModel === "whisper-1";
	const isGroqWhisperModel =
		isGroqStt &&
		(effectiveSttModel === null ||
			Boolean(effectiveSttModel?.includes("whisper")));

	const promptMaxChars = 224;
	const isPrompt224CharLimited =
		isWhisper1Selected ||
		isGroqWhisperModel ||
		isAquavoiceStt ||
		isWhisperServerStt;

	const sttPromptSupported =
		(isOpenAiStt &&
			(effectiveSttModel === "whisper-1" ||
				(Boolean(effectiveSttModel?.includes("transcribe")) &&
					!effectiveSttModel?.includes("diarize")))) ||
		isGroqWhisperModel ||
		isAquavoiceStt ||
		isWhisperServerStt;

	const sttPromptDisabledReason = useMemo(() => {
		if (!effectiveSttProvider) {
			return "Select an STT provider to enable transcription prompting.";
		}

		if (effectiveSttProvider === "openai") {
			const modelLabel = effectiveSttModel ?? "default";
			return `The selected OpenAI model (${modelLabel}) does not support transcription prompting.`;
		}

		if (effectiveSttProvider === "groq") {
			const modelLabel = effectiveSttModel ?? "default";
			return `The selected Groq model (${modelLabel}) does not support transcription prompting.`;
		}

		if (effectiveSttProvider === "aquavoice") {
			const modelLabel = effectiveSttModel ?? "default";
			return `The selected Aquovoice model (${modelLabel}) does not support transcription prompting.`;
		}

		return "Transcription prompt is only supported for certain models.";
	}, [effectiveSttProvider, effectiveSttModel]);

	const hasStoredTranscriptionPrompt =
		Boolean(settings?.stt_transcription_prompt?.trim()) && sttPromptSupported;

	// Keep the local UI state in sync with persisted settings.
	useEffect(() => {
		setLocalSttTranscriptionPrompt(settings?.stt_transcription_prompt ?? "");
	}, [settings?.stt_transcription_prompt]);

	// Debounced save (global setting). We only allow editing/saving when supported.
	useEffect(() => {
		if (!sttPromptSupported) return;

		const normalized = localSttTranscriptionPrompt.trim();
		const toStore: string | null = normalized.length > 0 ? normalized : null;
		const storedNormalized: string | null =
			settings?.stt_transcription_prompt?.trim() || null;

		if (toStore === storedNormalized) return;

		const handle = window.setTimeout(() => {
			updateSTTTranscriptionPrompt.mutate(toStore);
		}, 500);

		return () => {
			window.clearTimeout(handle);
		};
	}, [
		localSttTranscriptionPrompt,
		settings?.stt_transcription_prompt,
		sttPromptSupported,
		updateSTTTranscriptionPrompt,
	]);

	// NOTE: Quick Ask System Prompt uses an explicit Save button (like Rewrite prompts),
	// so we intentionally do NOT auto-save/debounce here.

	const sttProviderIsWhisperServer = effectiveSttProvider === "whisper-server";

	// If Ollama is selected and no explicit model is set yet, automatically
	// persist the first discovered model so backend and UI stay in sync.
	useEffect(() => {
		if (!isDefaultScope) return;
		if (effectiveLlmProvider !== "ollama") return;
		if (updateLLMModel.isPending) return;
		if (settings?.llm_model) return;

		const models = ollamaModelsQuery.data;
		if (!Array.isArray(models) || models.length === 0) return;

		const first = models[0]?.value ?? null;
		if (!first) return;

		updateLLMModel.mutate(first);
	}, [
		isDefaultScope,
		effectiveLlmProvider,
		settings?.llm_model,
		ollamaModelsQuery.data,
		updateLLMModel,
	]);

	const [whisperServerModelDraft, setWhisperServerModelDraft] = useState("");

	useEffect(() => {
		if (!sttProviderIsWhisperServer) return;
		setWhisperServerModelDraft(selectedSttModelForUi ?? "");
	}, [sttProviderIsWhisperServer, selectedSttModelForUi]);

	const sttPricing = useModelPricing(
		effectiveSttProvider,
		"stt",
		selectedSttModelForUi,
	);
	const llmPricing = useModelPricing(
		effectiveLlmProvider,
		"llm",
		selectedLlmModelForUi,
	);

	const sttPricingLabel = useMemo(() => {
		const stt = sttPricing.data?.stt;
		const isRealtime = isRealtimeSttModel(
			effectiveSttProvider,
			selectedSttModelForUi,
		);

		if (!stt) {
			// No pricing data but still realtime — show just the tag.
			return isRealtime ? "Realtime" : null;
		}

		const minSecs =
			typeof stt.min_billed_secs === "number" ? stt.min_billed_secs : null;

		const withMinBill = (base: string) =>
			minSecs ? `${base} · min ${minSecs}s` : base;

		const withRealtime = (label: string) =>
			isRealtime ? `Realtime · ${label}` : label;

		if (typeof stt.usd_micros_per_hour === "number") {
			const base = `${formatUsdRateFromMicros(stt.usd_micros_per_hour)}/hr`;
			return withRealtime(withMinBill(base));
		}

		// Some providers report pricing as USD/minute. For consistency in the UI,
		// normalize everything to USD/hour.
		if (typeof stt.usd_micros_per_minute === "number") {
			const perHourMicros = Math.round(stt.usd_micros_per_minute * 60);
			const base = `${formatUsdRateFromMicros(perHourMicros)}/hr`;
			return withRealtime(withMinBill(base));
		}

		// No pricing but still realtime
		if (isRealtime) return "Realtime";

		return null;
	}, [sttPricing.data, effectiveSttProvider, selectedSttModelForUi]);

	const llmPricingLabel = useMemo(() => {
		const llm = llmPricing.data?.llm;
		if (!llm) return null;

		const input = formatUsdRateFromMicros(llm.input_usd_micros_per_1m);
		const output = formatUsdRateFromMicros(llm.output_usd_micros_per_1m);
		return `in ${input} · out ${output} /1M tok`;
	}, [llmPricing.data]);

	// Thinking options (supports flags, dropdown options, utility functions)
	const {
		supportsOpenAiThinking,
		supportsGeminiThinkingLevel,
		supportsGeminiThinkingBudget,
		supportsAnthropicThinkingBudget,
		supportsQuickAskOpenAiThinking,
		supportsQuickAskGeminiThinkingLevel,
		supportsQuickAskGeminiThinkingBudget,
		supportsQuickAskAnthropicThinkingBudget,
		quickAskModelForThinking,
		openAiThinkingOptions,
		quickAskOpenAiThinkingOptions,
		geminiThinkingLevelOptions,
		geminiThinkingBudgetOptions,
		isGemini3Pro,
		quickAskGeminiThinkingLevelOptions,
		quickAskGeminiThinkingBudgetOptions,
		anthropicThinkingLevelOptionsWithCustom,
		quickAskAnthropicThinkingLevelOptionsWithCustom,
	} = useThinkingOptions({
		effectiveLlmProvider,
		effectiveLlmModel,
		effectiveQuickAskProvider,
		effectiveQuickAskModel,
		selectedQuickAskModelForUi,
		isDefaultScope,
		defaultAnthropicThinkingBudget: settings?.anthropic_thinking_budget,
		localProfileAnthropicThinkingBudget,
		defaultQuickAskAnthropicThinkingBudget:
			settings?.quick_ask_anthropic_thinking_budget,
		localProfileQuickAskAnthropicThinkingBudget,
	});

	const baseStoredSections: CleanupPromptSections = settings
		?.cleanup_prompt_sections?.system
		? settings.cleanup_prompt_sections
		: DEFAULT_SECTIONS;

	const storedSectionsResolved: CleanupPromptSections =
		activeProfileId === "default" || !activeProfile
			? baseStoredSections
			: {
					system:
						activeProfile.cleanup_prompt_sections?.system ??
						baseStoredSections.system,
				};

	const hasCustomContent = {
		system: Boolean(storedSectionsResolved.system.content),
	};

	const defaultSystemPromptInheritMode = isDefaultScope
		? null
		: activeProfile?.cleanup_prompt_sections?.system == null
			? "inheriting"
			: "overriding";

	const saveProfileMetadata = (next: Partial<RewriteProgramPromptProfile>) => {
		const exists = profiles.some((p) => p.id === activeProfileId);

		// Backward compatible: if Default hasn't been migrated into the profile list yet,
		// treat it like a normal profile by upserting a persisted entry.
		if (!exists) {
			if (activeProfileId !== "default") return;

			const defaultProfile: RewriteProgramPromptProfile = {
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

				context_grab_method: null,
				quick_ask_dismiss_mode: null,
			};

			const updated = [...profiles, { ...defaultProfile, ...next }];
			updateRewriteProgramPromptProfiles.mutate(updated);
			return;
		}

		const updated = profiles.map((p) =>
			p.id === activeProfileId ? { ...p, ...next } : p,
		);

		updateRewriteProgramPromptProfiles.mutate(updated);
	};

	// Section management (local section state, save/reset handlers)
	const {
		localSections,
		setLocalSections,
		effectiveCurrentPrompt,
		profilePromptOverridesRef,
		handleSave,
		handleReset,
		normalizePromptOverrides,
	} = useSectionManagement({
		settings,
		defaultSections,
		activeProfileId,
		profiles,
		activeProfile,
		updateCleanupPromptSections,
		saveProfileMetadata,
	});

	const {
		promptLabOpen,
		promptLabContextPrompt,
		promptLabContextLabel,
		promptLabApplyTarget,
		handleOpenPresetPromptLab,
		handleOpenDefaultPromptLab,
		closePromptLab,
	} = usePromptLabState({
		effectiveCurrentPrompt,
		activeProfileLabel,
	});

	const isLoading =
		isLoadingSettings ||
		isLoadingDefaultSections ||
		isLoadingProviders ||
		settings === undefined ||
		defaultSections === undefined ||
		localSections === null;

	// Preset management (CRUD, linking, local form state)
	const {
		presets,
		editingPresetId,
		setEditingPresetId,
		selectedPreset,
		selectedPresetRuntimeFallbackViews,
		isEditingDefaultPreset,
		localPresetName,
		setLocalPresetName,
		localPresetHintsText,
		setLocalPresetHintsText,
		localDefaultPresetDescription,
		setLocalDefaultPresetDescription,
		presetEditorOpen,
		setPresetEditorOpen,
		deletePresetDialog,
		setDeletePresetDialog,
		handleConfirmDeletePreset,
		linkPresetModalOpen,
		setLinkPresetModalOpen,
		linkableProfiles,
		linkSourceProfileId,
		linkSourcePresetId,
		linkSourceProfile,
		linkSourcePreset,
		openLinkPresetModal,
		confirmLinkPreset,
		handleLinkSourceProfileChange,
		handleLinkSourcePresetChange,
		newPreset,
		updatePreset,
		isSharedPresetId,
		isSavingProfiles,
	} = usePresetManagement({
		activeProfile,
		activeProfileId,
		profiles,
		settings: presetRuntimeSettings,
		defaultSttTimeout: DEFAULT_STT_TIMEOUT,
		defaultSttLanguage: DEFAULT_STT_LANGUAGE,
		saveProfileMetadata,
	});

	const {
		handleRewriteEnabledChange,
		handleDisableRewriteEnabledOverride,
		handleRewriteIncludeClipboardContextChange,
		handleDisableRewriteIncludeClipboardContextOverride,
		handleRewriteLlmProviderChange,
		handleDisableRewriteLlmProviderOverride,
		handleRewriteLlmModelChange,
		handleDisableRewriteLlmModelOverride,
		handleRewriteOpenAiThinkingChange,
		handleDisableRewriteOpenAiThinkingOverride,
		handleRewriteGeminiThinkingLevelChange,
		handleDisableRewriteGeminiThinkingLevelOverride,
		handleRewriteGeminiThinkingBudgetChange,
		handleDisableRewriteGeminiThinkingBudgetOverride,
		handleRewriteAnthropicThinkingBudgetChange,
		handleDisableRewriteAnthropicThinkingBudgetOverride,
	} = useRewriteSettingsHandlers({
		isDefaultScope,
		settings,
		defaultRewriteEnabled,
		defaultRewriteIncludeClipboardContext,
		setRewriteEnabledInheriting,
		setLocalProfileRewriteEnabled,
		setRewriteIncludeClipboardContextInheriting,
		setLocalProfileRewriteIncludeClipboardContext,
		setLlmProviderInheriting,
		setLlmModelInheriting,
		setLocalProfileLlmProvider,
		setLocalProfileLlmModel,
		setOpenAiReasoningEffortInheriting,
		setLocalProfileOpenAiReasoningEffort,
		setGeminiThinkingLevelInheriting,
		setLocalProfileGeminiThinkingLevel,
		setGeminiThinkingBudgetInheriting,
		setLocalProfileGeminiThinkingBudget,
		setAnthropicThinkingBudgetInheriting,
		setLocalProfileAnthropicThinkingBudget,
		updateRewriteLlmEnabled,
		updateLLMProvider,
		updateLLMModel,
		updateOpenAiReasoningEffort,
		updateGeminiThinkingLevel,
		updateGeminiThinkingBudget,
		updateAnthropicThinkingBudget,
		getLlmModelOptionsForProvider,
		saveProfileMetadata,
		openDisableOverrideDialog,
	});

	const managedSttCompatible = Boolean(
		managedModels?.some(
			(model) =>
				model.provider === effectiveSttProvider &&
				model.id === selectedSttModelForUi &&
				model.capabilities.includes("transcription"),
		),
	);
	const sttOwnKeyConfigured = Boolean(
		availableProviders?.stt.some(
			(provider) =>
				!provider.is_local && provider.value === effectiveSttProvider,
		),
	);

	const managedLlmSelection = managedModels?.find((model) => {
		if (!model.capabilities.includes("chat_completions")) return false;
		if (effectiveLlmProvider === "managed") {
			return model.id === selectedLlmModelForUi;
		}
		const target = managedModelByokTarget(model);
		return (
			target?.provider === effectiveLlmProvider &&
			target.model === selectedLlmModelForUi
		);
	});
	const managedLlmCompatible = Boolean(managedLlmSelection);
	const managedLlmByokTarget = managedLlmSelection
		? managedModelByokTarget(managedLlmSelection)
		: null;
	const llmUsingOwnKey =
		managedLlmCompatible && effectiveLlmProvider !== "managed";
	const llmOwnKeyConfigured = Boolean(
		managedLlmByokTarget &&
			availableProviders?.llm.some(
				(provider) =>
					!provider.is_local &&
					provider.value === managedLlmByokTarget.provider,
			),
	);

	const setRewriteProviderAndModel = (provider: string, model: string) => {
		if (isDefaultScope) {
			updateLLMProvider.mutate(provider, {
				onSuccess: () => updateLLMModel.mutate(model),
			});
			return;
		}
		setLlmProviderInheriting(false);
		setLlmModelInheriting(false);
		setLocalProfileLlmProvider(provider);
		setLocalProfileLlmModel(model);
		saveProfileMetadata({ llm_provider: provider, llm_model: model });
	};

	const handleLlmUseOwnKeyChange = (useOwnKey: boolean) => {
		if (!managedLlmSelection) return;
		if (useOwnKey) {
			const target = managedModelByokTarget(managedLlmSelection);
			if (!target) return;
			setShowAllProvidersAndModels(true);
			setRewriteProviderAndModel(target.provider, target.model);
			return;
		}
		setRewriteProviderAndModel("managed", managedLlmSelection.id);
	};

	const {
		handleWhisperServerModelDraftBlur,
		handleSttProviderChange,
		handleSttModelChange,
		handleSttLanguageChange,
		handleSttTimeoutChange,
		handleSttTimeoutBlur,
		handleDisableSttProviderOverride,
		handleDisableSttModelOverride,
		handleDisableSttLanguageOverride,
		handleDisableSttTimeoutOverride,
	} = useSttSettingsHandlers({
		isDefaultScope,
		settings,
		activeProfile,
		whisperServerModelDraft,
		localProfileSttTimeout,
		localProfileSttLanguage,
		setSttProviderInheriting,
		setSttModelInheriting,
		setSttLanguageInheriting,
		setSttTimeoutInheriting,
		setLocalProfileSttProvider,
		setLocalProfileSttModel,
		setLocalProfileSttLanguage,
		setLocalProfileSttTimeout,
		updateSTTProvider,
		updateSTTModel,
		updateSTTLanguage,
		updateSTTTimeout: _updateSTTTimeout,
		saveProfileMetadata,
		openDisableOverrideDialog,
	});

	const handleDefaultQuickAskProviderChange = (value: string | null) => {
		if (!value) return;
		updateQuickAskProvider.mutate(value, {
			onSuccess: () => {
				const models = getLlmModelOptionsForProvider(value);
				const firstModel = models?.[0];
				if (firstModel) {
					updateQuickAskModel.mutate(firstModel.value);
				}
			},
		});
	};

	const handleDefaultQuickAskModelChange = (value: string | null) => {
		if (!value) return;
		updateQuickAskModel.mutate(value);
	};

	if (isLoading) {
		return (
			<div
				style={{
					display: "flex",
					justifyContent: "center",
					padding: "20px",
				}}
			>
				<Loader size="sm" color="orange" />
			</div>
		);
	}

	// If user selected a profile that no longer exists, fall back to Default.
	if (activeProfileId !== "default" && !activeProfile) {
		return (
			<div style={{ fontSize: 12, opacity: 0.75 }}>
				That profile no longer exists. Select another profile in the Editing
				dropdown.
			</div>
		);
	}

	const presetSelectOptions = presets.map((p) => {
		const base = p.name || p.id;
		const suffix = isSharedPresetId(p.id) ? " (shared)" : "";
		return {
			value: p.id,
			label: `${base}${suffix}`,
		};
	});

	const defaultPresetRewriteStepValue =
		(activeProfile?.default_target_rewrite_llm_enabled ?? true) ? "on" : "off";

	const defaultPresetValue =
		!activeProfile || !activeProfile.default_preset_id
			? "__none__"
			: activeProfile.default_preset_id;

	const activePresetValue =
		!activeProfile || !activeProfile.active_preset_id
			? "__none__"
			: activeProfile.active_preset_id;

	const routerStrategyValue =
		!effectiveRouter || !effectiveRouter.enabled
			? "off"
			: effectiveRouter.strategy;

	const embeddingProviderValue =
		effectiveRouter?.embedding_provider ?? "openai";
	const embeddingModels =
		EMBEDDING_MODELS[embeddingProviderValue] ?? EMBEDDING_MODELS.openai ?? [];
	const embeddingModelValue = (() => {
		const raw = effectiveRouter?.embedding_model ?? null;
		if (raw && embeddingModels.some((m) => m.value === raw)) return raw;
		return embeddingModels[0]?.value ?? null;
	})();

	const getEmbeddingModelsForProvider = (provider: string): ModelOption[] => {
		return EMBEDDING_MODELS[provider] ?? [];
	};

	const handleCacheRouterEmbeddings = async () => {
		if (activeProfileId === "default") return;
		setIsCachingRouterEmbeddings(true);
		try {
			const res = await tauriAPI.cacheRouterEmbeddings({
				profileId: activeProfileId,
			});
			notifications.show({
				title: "Stored router embeddings",
				message: `Cached ${res.cached_now} / ${res.total_hints} hints (${res.skipped_existing} already cached) · ${res.provider} / ${res.model}`,
				color: "gray",
			});
		} catch (e) {
			notifications.show({
				title: "Failed to store embeddings",
				message: formatErrorMessage(e),
				color: "red",
			});
		} finally {
			setIsCachingRouterEmbeddings(false);
		}
	};

	const profilePromptDefaultContent = localSections.system.content ?? "";

	const getPresetPromptOverride = (
		preset: RewritePreset,
		key: SectionKey,
	): CleanupPromptSections[SectionKey] | null => {
		const o = preset.cleanup_prompt_sections ?? null;
		if (!o) return null;
		const v = o[key];
		return v ?? null;
	};

	const savePresetSectionOverride = (
		preset: RewritePreset,
		key: SectionKey,
		section: CleanupPromptSections[SectionKey] | null,
	) => {
		const current: CleanupPromptSectionsOverride =
			preset.cleanup_prompt_sections ?? {};
		const nextOverrides: CleanupPromptSectionsOverride = {
			...current,
			[key]: section,
		};
		const hasAny = nextOverrides.system != null;
		updatePreset(preset.id, {
			cleanup_prompt_sections: hasAny ? nextOverrides : null,
		});
	};

	const handleDisableDefaultSystemPromptOverride = () => {
		openDisableOverrideDialog({
			title: "Disable System Prompt override?",
			onConfirm: () => {
				const base = settings?.cleanup_prompt_sections ?? DEFAULT_SECTIONS;

				const current: CleanupPromptSectionsOverride =
					activeProfile?.cleanup_prompt_sections ?? {};
				const next = normalizePromptOverrides({
					...current,
					system: null,
				});
				profilePromptOverridesRef.current = next;

				const resolved: CleanupPromptSections = {
					system: next?.system ?? base.system,
				};

				setLocalSections({
					system: {
						content: resolved.system.content ?? defaultSections?.system ?? "",
					},
				});

				saveProfileMetadata({
					cleanup_prompt_sections: next,
				});
			},
		});
	};

	const handleDefaultPresetRewriteStepChange = (value: string) => {
		if (!value) return;
		saveProfileMetadata({
			default_target_rewrite_llm_enabled: value === "on",
		});
	};

	const handleSaveDefaultPresetDescription = (value: string | null) => {
		saveProfileMetadata({ default_preset_description: value });
	};

	return (
		<>
			<PromptSettingsModals
				linkPresetModalOpen={linkPresetModalOpen}
				onCloseLinkPresetModal={() => setLinkPresetModalOpen(false)}
				linkableProfiles={linkableProfiles}
				linkSourceProfileId={linkSourceProfileId}
				onLinkSourceProfileChange={handleLinkSourceProfileChange}
				linkSourcePresetId={linkSourcePresetId}
				onLinkSourcePresetChange={handleLinkSourcePresetChange}
				linkSourceProfile={linkSourceProfile}
				canConfirmLinkPreset={Boolean(linkSourcePreset)}
				onConfirmLinkPreset={confirmLinkPreset}
				deletePresetDialog={deletePresetDialog}
				onCloseDeletePresetDialog={() => setDeletePresetDialog(null)}
				onConfirmDeletePreset={handleConfirmDeletePreset}
				resetDialog={resetDialog}
				onCloseResetDialog={() => setResetDialog(null)}
				onConfirmResetDialog={handleConfirmResetDialog}
			/>

			<RewritePromptLabModal
				opened={promptLabOpen}
				onClose={closePromptLab}
				profileId={activeProfileId}
				profileLabel={promptLabContextLabel || activeProfileLabel}
				initialLlmProvider={effectiveLlmProvider}
				initialLlmModel={effectiveLlmModel}
				initialTranscript={rewriteTestInput}
				initialProblemOutput={rewriteTestOutput}
				currentPrompt={promptLabContextPrompt || effectiveCurrentPrompt}
				onSetPrompt={(nextPrompt) => {
					const trimmed = nextPrompt.trim();
					if (!trimmed) return;

					const target = promptLabApplyTarget;
					if (!target || target.type === "profile") {
						handleSave("system", trimmed);
						return;
					}

					const preset = presets.find((p) => p.id === target.presetId);
					if (!preset) {
						// Preset was deleted/changed while modal was open.
						handleSave("system", trimmed);
						return;
					}

					const baseContent = profilePromptDefaultContent;
					const contentToStore =
						trimmed === baseContent ? null : trimmed || null;
					const section =
						contentToStore == null ? null : { content: contentToStore };
					savePresetSectionOverride(preset, target.key, section);
				}}
				onIteratePrompt={async (params) => {
					const res = await iterateRewritePrompt.mutateAsync({
						transcript: params.transcript,
						problemOutput: params.problemOutput,
						desiredOutput: params.desiredOutput,
						currentPrompt: params.currentPrompt,
						profileId: params.profileId,
						mode: params.mode,
						llmProvider: params.llmProvider,
						llmModel: params.llmModel,
						openAiReasoningEffort: params.openAiReasoningEffort,
						geminiThinkingLevel: params.geminiThinkingLevel,
						geminiThinkingBudget: params.geminiThinkingBudget,
						anthropicThinkingBudget: params.anthropicThinkingBudget,
					});

					return {
						improvedPrompt: res.improved_prompt,
						providerUsed: res.provider_used,
						modelUsed: res.model_used,
					};
				}}
				onTestPrompt={async (params) => {
					const res = await testRewriteWithPrompt.mutateAsync({
						transcript: params.transcript,
						prompt: params.prompt,
						profileId: params.profileId,
					});

					return {
						output: res.output,
						providerUsed: res.provider_used,
						modelUsed: res.model_used,
					};
				}}
			/>

			{managedAccessEnabled ? (
				<SettingsRow
					label="Provider visibility"
					description="Managed providers and models are shown by default."
					right={
						<Switch
							label="Show all providers and models"
							checked={showAllProvidersAndModels}
							onChange={(event) =>
								setShowAllProvidersAndModels(event.currentTarget.checked)
							}
							color="gray"
							size="md"
						/>
					}
				/>
			) : null}

			<TranscribeSettingsSection
				activeProfileId={activeProfileId}
				isDefaultScope={isDefaultScope}
				inheritTooltip={INHERIT_TOOLTIP}
				sttProviderInheriting={sttProviderInheriting}
				sttModelInheriting={sttModelInheriting}
				sttTimeoutInheriting={sttTimeoutInheriting}
				sttLanguageInheriting={sttLanguageInheriting}
				effectiveSttProvider={effectiveSttProvider}
				sttProviderOptions={sttProviderOptions}
				isSttProviderOptionsDisabled={
					sttCloudProviders.length === 0 && sttLocalProviders.length === 0
				}
				sttProviderIsWhisperServer={sttProviderIsWhisperServer}
				sttModelOptions={sttModelOptions}
				selectedSttModelForUi={selectedSttModelForUi}
				sttPricingLabel={sttPricingLabel}
				sttLanguageOptions={STT_LANGUAGE_OPTIONS}
				localProfileSttLanguage={localProfileSttLanguage}
				whisperServerModelDraft={whisperServerModelDraft}
				onWhisperServerModelDraftChange={setWhisperServerModelDraft}
				onWhisperServerModelBlur={handleWhisperServerModelDraftBlur}
				onSttProviderChange={handleSttProviderChange}
				onSttModelChange={handleSttModelChange}
				onSttLanguageChange={handleSttLanguageChange}
				onDisableSttProviderOverride={handleDisableSttProviderOverride}
				onDisableSttModelOverride={handleDisableSttModelOverride}
				onDisableSttLanguageOverride={handleDisableSttLanguageOverride}
				onDisableSttTimeoutOverride={handleDisableSttTimeoutOverride}
				localProfileSttTimeout={localProfileSttTimeout}
				onSttTimeoutChange={handleSttTimeoutChange}
				onSttTimeoutBlur={handleSttTimeoutBlur}
				sttPromptSupported={sttPromptSupported}
				sttPromptDisabledReason={sttPromptDisabledReason}
				sttPromptMaxChars={promptMaxChars}
				isPrompt224CharLimited={isPrompt224CharLimited}
				localSttTranscriptionPrompt={localSttTranscriptionPrompt}
				onSttPromptChange={setLocalSttTranscriptionPrompt}
				sttTestDurationMs={sttTestDurationMs}
				sttTestError={sttTestError}
				sttTestOutput={sttTestOutput}
				hasLastAudioForSttTest={Boolean(hasLastAudioForSttTest)}
				isSttTestRunning={testSttLastAudio.isPending}
				onRunSttTest={handleRunSttTest}
				hasStoredTranscriptionPrompt={hasStoredTranscriptionPrompt}
				managedAccessEnabled={managedAccessEnabled}
				managedModelCompatible={managedSttCompatible}
				useManagedInference={settings?.stt_use_managed_inference ?? true}
				ownKeyConfigured={sttOwnKeyConfigured}
				onUseOwnKeyChange={(useOwnKey) =>
					updateSTTUseManagedInference.mutate(!useOwnKey)
				}
			/>

			<RewriteSettingsSection
				isDefaultScope={isDefaultScope}
				inheritTooltip={INHERIT_TOOLTIP}
				ocrProviderAvailable={ocrProviderAvailable}
				ocrProviderUnavailableReason={ocrProviderUnavailableReason}
				defaultRewriteEnabled={defaultRewriteEnabled}
				localProfileRewriteEnabled={localProfileRewriteEnabled}
				rewriteEnabledInheriting={rewriteEnabledInheriting}
				onRewriteEnabledChange={handleRewriteEnabledChange}
				onDisableRewriteEnabledOverride={handleDisableRewriteEnabledOverride}
				isUpdatingRewriteEnabled={updateRewriteLlmEnabled.isPending}
				localProfileRewriteIncludeClipboardContext={
					localProfileRewriteIncludeClipboardContext
				}
				rewriteIncludeClipboardContextInheriting={
					rewriteIncludeClipboardContextInheriting
				}
				onRewriteIncludeClipboardContextChange={
					handleRewriteIncludeClipboardContextChange
				}
				onDisableRewriteIncludeClipboardContextOverride={
					handleDisableRewriteIncludeClipboardContextOverride
				}
				localProfileRewriteActiveWindowOcrMode={
					localProfileRewriteActiveWindowOcrMode
				}
				rewriteActiveWindowOcrModeInheriting={
					rewriteActiveWindowOcrModeInheriting
				}
				onRewriteActiveWindowOcrModeChange={
					handleRewriteActiveWindowOcrModeChange
				}
				onDisableRewriteActiveWindowOcrModeOverride={
					handleDisableRewriteActiveWindowOcrModeOverride
				}
				effectiveLlmProvider={effectiveLlmProvider}
				llmProviderOptions={llmProviderOptions}
				isLlmProviderDisabled={
					llmCloudProviders.length === 0 && llmLocalProviders.length === 0
				}
				llmProviderInheriting={llmProviderInheriting}
				onLlmProviderChange={handleRewriteLlmProviderChange}
				onDisableLlmProviderOverride={handleDisableRewriteLlmProviderOverride}
				llmModelOptions={llmModelOptions}
				llmModelInheriting={llmModelInheriting}
				localProfileLlmModel={localProfileLlmModel}
				llmPricingLabel={llmPricingLabel}
				settings={settings}
				onLlmModelChange={handleRewriteLlmModelChange}
				onDisableLlmModelOverride={handleDisableRewriteLlmModelOverride}
				supportsOpenAiThinking={supportsOpenAiThinking}
				openAiReasoningEffortInheriting={openAiReasoningEffortInheriting}
				localProfileOpenAiReasoningEffort={localProfileOpenAiReasoningEffort}
				openAiThinkingOptions={openAiThinkingOptions}
				effectiveLlmModel={effectiveLlmModel}
				onOpenAiThinkingChange={handleRewriteOpenAiThinkingChange}
				onDisableOpenAiThinkingOverride={
					handleDisableRewriteOpenAiThinkingOverride
				}
				openAiDefaultReasoningEffortForModel={
					openAiDefaultReasoningEffortForModel
				}
				supportsGeminiThinkingLevel={supportsGeminiThinkingLevel}
				isGemini3Pro={isGemini3Pro}
				geminiThinkingLevelInheriting={geminiThinkingLevelInheriting}
				localProfileGeminiThinkingLevel={localProfileGeminiThinkingLevel}
				geminiThinkingLevelOptions={geminiThinkingLevelOptions}
				onGeminiThinkingLevelChange={handleRewriteGeminiThinkingLevelChange}
				onDisableGeminiThinkingLevelOverride={
					handleDisableRewriteGeminiThinkingLevelOverride
				}
				supportsGeminiThinkingBudget={supportsGeminiThinkingBudget}
				geminiThinkingBudgetInheriting={geminiThinkingBudgetInheriting}
				localProfileGeminiThinkingBudget={localProfileGeminiThinkingBudget}
				geminiThinkingBudgetOptions={geminiThinkingBudgetOptions}
				onGeminiThinkingBudgetChange={handleRewriteGeminiThinkingBudgetChange}
				onDisableGeminiThinkingBudgetOverride={
					handleDisableRewriteGeminiThinkingBudgetOverride
				}
				supportsAnthropicThinkingBudget={supportsAnthropicThinkingBudget}
				anthropicThinkingBudgetInheriting={anthropicThinkingBudgetInheriting}
				localProfileAnthropicThinkingBudget={
					localProfileAnthropicThinkingBudget
				}
				anthropicThinkingLevelOptionsWithCustom={
					anthropicThinkingLevelOptionsWithCustom
				}
				onAnthropicThinkingBudgetChange={
					handleRewriteAnthropicThinkingBudgetChange
				}
				onDisableAnthropicThinkingBudgetOverride={
					handleDisableRewriteAnthropicThinkingBudgetOverride
				}
				formatThinkingBudgetShort={formatThinkingBudgetShort}
				managedAccessEnabled={managedAccessEnabled}
				managedModelCompatible={managedLlmCompatible}
				usingOwnKey={llmUsingOwnKey}
				ownKeyAvailable={Boolean(managedLlmByokTarget)}
				ownKeyConfigured={llmOwnKeyConfigured}
				onUseOwnKeyChange={handleLlmUseOwnKeyChange}
			/>
			{/* System prompt + test rewrite live inside the preset editor (Default or a specific preset). */}

			{activeProfile ? (
				<div
					className="settings-accordion-block"
					style={{ marginTop: 0, marginBottom: 16 }}
				>
					<Accordion variant="separated" radius="md">
						<Accordion.Item value={`${activeProfileId}-presets`}>
							<Accordion.Control>
								<div>
									<p className="settings-label">Presets</p>
									<p className="settings-description">
										Create multiple dictation modes for this program, then
										choose one manually or let the intent router auto-select.
									</p>
								</div>
							</Accordion.Control>
							<Accordion.Panel>
								<div
									style={{
										display: "flex",
										flexDirection: "column",
										gap: 12,
									}}
								>
									<Group
										justify="space-between"
										align="center"
										wrap="wrap"
										gap={12}
									>
										<div
											style={{
												display: "flex",
												alignItems: "center",
												gap: 12,
												flexWrap: "wrap",
											}}
										>
											<div>
												<Text size="xs" c="dimmed" mb={4}>
													Default preset
												</Text>
												<Select
													data={[
														{ value: "__none__", label: "Default" },
														...presetSelectOptions,
													]}
													value={defaultPresetValue}
													onChange={(value) => {
														if (!value) return;
														saveProfileMetadata({
															default_preset_id:
																value === "__none__" ? null : value,
														});
													}}
													placeholder="Default"
													withCheckIcon={false}
													styles={{
														input: {
															backgroundColor: "var(--bg-elevated)",
															borderColor: "var(--border-default)",
															color: "var(--text-primary)",
															minWidth: 220,
														},
													}}
												/>
											</div>

											<div>
												<Text size="xs" c="dimmed" mb={4}>
													Manual preset override (persisted)
												</Text>
												<Select
													data={[
														{
															value: "__none__",
															label: "No override (use router/default)",
														},
														...presetSelectOptions,
													]}
													value={activePresetValue}
													onChange={(value) => {
														if (!value) return;
														saveProfileMetadata({
															active_preset_id:
																value === "__none__" ? null : value,
														});
													}}
													placeholder="Default"
													withCheckIcon={false}
													styles={{
														input: {
															backgroundColor: "var(--bg-elevated)",
															borderColor: "var(--border-default)",
															color: "var(--text-primary)",
															minWidth: 260,
														},
													}}
												/>
											</div>
										</div>

										<Button
											color="gray"
											variant="light"
											onClick={() => setPresetEditorOpen(true)}
										>
											Edit Presets
										</Button>
									</Group>

									<PresetEditorModal
										opened={presetEditorOpen}
										onClose={() => setPresetEditorOpen(false)}
										editDefaultPresetId={EDIT_DEFAULT_PRESET}
										presetSelectOptions={presetSelectOptions}
										editingPresetId={editingPresetId}
										onEditingPresetChange={setEditingPresetId}
										onNewPreset={newPreset}
										linkableProfiles={linkableProfiles}
										onOpenLinkPresetModal={openLinkPresetModal}
										isEditingDefaultPreset={isEditingDefaultPreset}
										selectedPreset={selectedPreset}
										selectedPresetRuntimeFallbackViews={
											selectedPresetRuntimeFallbackViews
										}
										onRequestDeletePreset={(preset) =>
											setDeletePresetDialog({
												presetId: preset.id,
												presetName: preset.name?.trim() || preset.id,
												isShared: isSharedPresetId(preset.id),
											})
										}
										localPresetName={localPresetName}
										onLocalPresetNameChange={setLocalPresetName}
										localPresetHintsText={localPresetHintsText}
										onLocalPresetHintsChange={setLocalPresetHintsText}
										onUpdatePreset={updatePreset}
										getPresetPromptOverride={getPresetPromptOverride}
										profilePromptDefaultContent={profilePromptDefaultContent}
										activeProfileId={activeProfileId}
										activeProfileLabel={activeProfileLabel}
										onOpenPresetPromptLab={handleOpenPresetPromptLab}
										onSavePresetSectionOverride={savePresetSectionOverride}
										isSavingProfiles={isSavingProfiles}
										rewriteTestInput={rewriteTestInput}
										onRewriteTestInputChange={setRewriteTestInput}
										onRunRewriteTest={runRewriteTest}
										isTestingRewrite={testRewriteWithPrompt.isPending}
										rewriteTestDurationMs={rewriteTestDurationMs}
										rewriteTestError={rewriteTestError}
										rewriteTestOutput={rewriteTestOutput}
										defaultPresetRewriteStepValue={
											defaultPresetRewriteStepValue
										}
										onDefaultPresetRewriteStepChange={
											handleDefaultPresetRewriteStepChange
										}
										localDefaultPresetDescription={
											localDefaultPresetDescription
										}
										onLocalDefaultPresetDescriptionChange={
											setLocalDefaultPresetDescription
										}
										currentDefaultPresetDescription={
											activeProfile?.default_preset_description ?? null
										}
										onSaveDefaultPresetDescription={
											handleSaveDefaultPresetDescription
										}
										defaultSystemPromptContent={
											localSections?.system.content ?? ""
										}
										defaultSystemPromptDefaultContent={
											defaultSections?.system ?? ""
										}
										defaultSystemPromptHasCustom={hasCustomContent.system}
										defaultSystemPromptInheritMode={
											defaultSystemPromptInheritMode
										}
										onDisableDefaultSystemPromptOverride={
											isDefaultScope
												? undefined
												: handleDisableDefaultSystemPromptOverride
										}
										onOpenDefaultPromptLab={handleOpenDefaultPromptLab}
										onSaveDefaultSystemPrompt={(content) =>
											handleSave("system", content)
										}
										onResetDefaultSystemPrompt={() => handleReset("system")}
										isDefaultPromptSaving={
											updateCleanupPromptSections.isPending ||
											updateRewriteProgramPromptProfiles.isPending
										}
										isDefaultPromptLabDisabled={
											updateCleanupPromptSections.isPending ||
											updateRewriteProgramPromptProfiles.isPending ||
											updateRewriteLlmEnabled.isPending
										}
										isSavingCleanupSections={
											updateCleanupPromptSections.isPending
										}
										isSavingRewriteEnabled={updateRewriteLlmEnabled.isPending}
									/>
								</div>
							</Accordion.Panel>
						</Accordion.Item>

						<PromptIntentRouterSection
							activeProfileId={activeProfileId}
							presets={presets}
							settings={settings}
							profileRouter={activeProfile?.router}
							effectiveRouter={effectiveRouter}
							routerStrategyValue={routerStrategyValue}
							embeddingProviderValue={embeddingProviderValue}
							embeddingModels={embeddingModels}
							embeddingModelValue={embeddingModelValue}
							isCachingRouterEmbeddings={isCachingRouterEmbeddings}
							selectDefaultValue={SELECT_DEFAULT}
							anthropicThinkingBudgets={ANTHROPIC_THINKING_LEVEL_BUDGETS}
							getEmbeddingModelsForProvider={getEmbeddingModelsForProvider}
							getLlmModelOptionsForProvider={getLlmModelOptionsForProvider}
							saveRouter={saveRouter}
							onCacheRouterEmbeddings={handleCacheRouterEmbeddings}
						/>
					</Accordion>
				</div>
			) : null}

			<QuickReplaceSettings
				activeProfileId={activeProfileId}
				activeProfile={activeProfile}
				isDefaultScope={isDefaultScope}
				inheritTooltip={INHERIT_TOOLTIP}
				defaultSystemPrompt={DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT}
				ocrProviderAvailable={ocrProviderAvailable}
				ocrProviderUnavailableReason={ocrProviderUnavailableReason}
				defaultQuickReplaceEnabled={defaultQuickReplaceEnabled}
				defaultQuickReplaceIncludeClipboardContext={
					defaultQuickReplaceIncludeClipboardContext
				}
				localProfileQuickReplaceActiveWindowOcrMode={
					localProfileQuickReplaceActiveWindowOcrMode
				}
				quickReplaceActiveWindowOcrModeInheriting={
					quickReplaceActiveWindowOcrModeInheriting
				}
				setQuickReplaceActiveWindowOcrModeInheriting={
					setQuickReplaceActiveWindowOcrModeInheriting
				}
				setLocalProfileQuickReplaceActiveWindowOcrMode={
					setLocalProfileQuickReplaceActiveWindowOcrMode
				}
				defaultQuickReplaceProvider={defaultQuickReplaceProvider}
				defaultQuickReplaceModel={defaultQuickReplaceModel}
				defaultQuickReplaceSystemPrompt={defaultQuickReplaceSystemPrompt}
				effectiveQuickReplaceProvider={effectiveQuickReplaceProvider}
				llmProviderOptions={llmProviderOptions}
				llmProviderDisabled={
					llmCloudProviders.length === 0 && llmLocalProviders.length === 0
				}
				quickReplaceModelOptions={quickReplaceModelOptions}
				selectedQuickReplaceModelForUi={selectedQuickReplaceModelForUi}
				localProfileQuickReplaceEnabled={localProfileQuickReplaceEnabled}
				localProfileQuickReplaceIncludeClipboardContext={
					localProfileQuickReplaceIncludeClipboardContext
				}
				localQuickReplaceSystemPrompt={localQuickReplaceSystemPrompt}
				quickReplaceEnabledInheriting={quickReplaceEnabledInheriting}
				quickReplaceIncludeClipboardContextInheriting={
					quickReplaceIncludeClipboardContextInheriting
				}
				quickReplaceProviderInheriting={quickReplaceProviderInheriting}
				quickReplaceModelInheriting={quickReplaceModelInheriting}
				quickReplaceSystemPromptInheriting={quickReplaceSystemPromptInheriting}
				setQuickReplaceEnabledInheriting={setQuickReplaceEnabledInheriting}
				setQuickReplaceIncludeClipboardContextInheriting={
					setQuickReplaceIncludeClipboardContextInheriting
				}
				setQuickReplaceProviderInheriting={setQuickReplaceProviderInheriting}
				setQuickReplaceModelInheriting={setQuickReplaceModelInheriting}
				setQuickReplaceSystemPromptInheriting={
					setQuickReplaceSystemPromptInheriting
				}
				setLocalProfileQuickReplaceEnabled={setLocalProfileQuickReplaceEnabled}
				setLocalProfileQuickReplaceIncludeClipboardContext={
					setLocalProfileQuickReplaceIncludeClipboardContext
				}
				setLocalProfileQuickReplaceProvider={
					setLocalProfileQuickReplaceProvider
				}
				setLocalProfileQuickReplaceModel={setLocalProfileQuickReplaceModel}
				setLocalQuickReplaceSystemPrompt={setLocalQuickReplaceSystemPrompt}
				saveProfileMetadata={saveProfileMetadata}
				openDisableOverrideDialog={openDisableOverrideDialog}
				getLlmModelOptionsForProvider={getLlmModelOptionsForProvider}
				rewriteProvider={settings?.llm_provider ?? null}
				rewriteModel={settings?.llm_model ?? null}
				isSaving={updateRewriteProgramPromptProfiles.isPending}
			/>
			<QuickAskPanel
				activeProfileId={activeProfileId}
				activeProfile={activeProfile}
				isDefaultScope={isDefaultScope}
				inheritTooltip={INHERIT_TOOLTIP}
				defaultSystemPrompt={DEFAULT_QUICK_ASK_SYSTEM_PROMPT}
				selectDefault={SELECT_DEFAULT}
				settings={settings}
				ocrProviderAvailable={ocrProviderAvailable}
				ocrProviderUnavailableReason={ocrProviderUnavailableReason}
				effectiveQuickAskProvider={effectiveQuickAskProvider}
				effectiveQuickAskModel={effectiveQuickAskModel}
				quickAskIncludeSelectedText={quickAskIncludeSelectedText}
				quickAskConversationHistoryEnabled={quickAskConversationHistoryEnabled}
				quickAskConversationHistoryCount={quickAskConversationHistoryCount}
				localProfileQuickAskActiveWindowOcrMode={
					localProfileQuickAskActiveWindowOcrMode
				}
				quickAskActiveWindowOcrModeInheriting={
					quickAskActiveWindowOcrModeInheriting
				}
				setQuickAskActiveWindowOcrModeInheriting={
					setQuickAskActiveWindowOcrModeInheriting
				}
				setLocalProfileQuickAskActiveWindowOcrMode={
					setLocalProfileQuickAskActiveWindowOcrMode
				}
				quickAskIncludeClipboardContextInheriting={
					quickAskIncludeClipboardContextInheriting
				}
				quickAskProviderInheriting={quickAskProviderInheriting}
				quickAskModelInheriting={quickAskModelInheriting}
				quickAskOpenAiReasoningEffortInheriting={
					quickAskOpenAiReasoningEffortInheriting
				}
				quickAskGeminiThinkingLevelInheriting={
					quickAskGeminiThinkingLevelInheriting
				}
				quickAskGeminiThinkingBudgetInheriting={
					quickAskGeminiThinkingBudgetInheriting
				}
				quickAskAnthropicThinkingBudgetInheriting={
					quickAskAnthropicThinkingBudgetInheriting
				}
				quickAskSystemPromptInheriting={quickAskSystemPromptInheriting}
				quickAskDismissModeInheriting={quickAskDismissModeInheriting}
				defaultQuickAskIncludeClipboardContext={
					defaultQuickAskIncludeClipboardContext
				}
				defaultQuickAskDismissMode={defaultQuickAskDismissMode}
				localProfileQuickAskIncludeClipboardContext={
					localProfileQuickAskIncludeClipboardContext
				}
				localProfileQuickAskDismissMode={localProfileQuickAskDismissMode}
				localProfileQuickAskOpenAiReasoningEffort={
					localProfileQuickAskOpenAiReasoningEffort
				}
				localProfileQuickAskGeminiThinkingLevel={
					localProfileQuickAskGeminiThinkingLevel
				}
				localProfileQuickAskGeminiThinkingBudget={
					localProfileQuickAskGeminiThinkingBudget
				}
				localProfileQuickAskAnthropicThinkingBudget={
					localProfileQuickAskAnthropicThinkingBudget
				}
				localQuickAskSystemPrompt={localQuickAskSystemPrompt}
				quickAskModelOptions={quickAskModelOptions}
				selectedQuickAskModelForUi={selectedQuickAskModelForUi}
				quickAskOpenAiThinkingOptions={quickAskOpenAiThinkingOptions}
				quickAskGeminiThinkingLevelOptions={quickAskGeminiThinkingLevelOptions}
				quickAskGeminiThinkingBudgetOptions={
					quickAskGeminiThinkingBudgetOptions
				}
				quickAskAnthropicThinkingLevelOptionsWithCustom={
					quickAskAnthropicThinkingLevelOptionsWithCustom
				}
				supportsQuickAskOpenAiThinking={supportsQuickAskOpenAiThinking}
				supportsQuickAskGeminiThinkingLevel={
					supportsQuickAskGeminiThinkingLevel
				}
				supportsQuickAskGeminiThinkingBudget={
					supportsQuickAskGeminiThinkingBudget
				}
				supportsQuickAskAnthropicThinkingBudget={
					supportsQuickAskAnthropicThinkingBudget
				}
				quickAskModelForThinking={quickAskModelForThinking}
				llmProviderOptions={llmProviderOptions}
				llmProviderDisabled={
					llmCloudProviders.length === 0 && llmLocalProviders.length === 0
				}
				updateQuickAskIncludeSelectedText={updateQuickAskIncludeSelectedText}
				updateQuickAskConversationHistoryEnabled={
					updateQuickAskConversationHistoryEnabled
				}
				updateQuickAskConversationHistoryCount={
					updateQuickAskConversationHistoryCount
				}
				updateQuickAskOpenAiReasoningEffort={
					updateQuickAskOpenAiReasoningEffort
				}
				updateQuickAskGeminiThinkingLevel={updateQuickAskGeminiThinkingLevel}
				updateQuickAskGeminiThinkingBudget={updateQuickAskGeminiThinkingBudget}
				updateQuickAskAnthropicThinkingBudget={
					updateQuickAskAnthropicThinkingBudget
				}
				updateQuickAskSystemPrompt={updateQuickAskSystemPrompt}
				updateQuickAskDismissMode={updateQuickAskDismissMode}
				setQuickAskIncludeClipboardContextInheriting={
					setQuickAskIncludeClipboardContextInheriting
				}
				setQuickAskProviderInheriting={setQuickAskProviderInheriting}
				setQuickAskModelInheriting={setQuickAskModelInheriting}
				setQuickAskOpenAiReasoningEffortInheriting={
					setQuickAskOpenAiReasoningEffortInheriting
				}
				setQuickAskGeminiThinkingLevelInheriting={
					setQuickAskGeminiThinkingLevelInheriting
				}
				setQuickAskGeminiThinkingBudgetInheriting={
					setQuickAskGeminiThinkingBudgetInheriting
				}
				setQuickAskAnthropicThinkingBudgetInheriting={
					setQuickAskAnthropicThinkingBudgetInheriting
				}
				setQuickAskSystemPromptInheriting={setQuickAskSystemPromptInheriting}
				setQuickAskDismissModeInheriting={setQuickAskDismissModeInheriting}
				setLocalProfileQuickAskIncludeClipboardContext={
					setLocalProfileQuickAskIncludeClipboardContext
				}
				setLocalProfileQuickAskProvider={setLocalProfileQuickAskProvider}
				setLocalProfileQuickAskModel={setLocalProfileQuickAskModel}
				setLocalProfileQuickAskDismissMode={setLocalProfileQuickAskDismissMode}
				setLocalProfileQuickAskOpenAiReasoningEffort={
					setLocalProfileQuickAskOpenAiReasoningEffort
				}
				setLocalProfileQuickAskGeminiThinkingLevel={
					setLocalProfileQuickAskGeminiThinkingLevel
				}
				setLocalProfileQuickAskGeminiThinkingBudget={
					setLocalProfileQuickAskGeminiThinkingBudget
				}
				setLocalProfileQuickAskAnthropicThinkingBudget={
					setLocalProfileQuickAskAnthropicThinkingBudget
				}
				setLocalQuickAskSystemPrompt={setLocalQuickAskSystemPrompt}
				handleDefaultQuickAskProviderChange={
					handleDefaultQuickAskProviderChange
				}
				handleDefaultQuickAskModelChange={handleDefaultQuickAskModelChange}
				openDisableOverrideDialog={openDisableOverrideDialog}
				saveProfileMetadata={saveProfileMetadata}
				getLlmModelOptionsForProvider={getLlmModelOptionsForProvider}
				isOpenAiReasoningEffort={isOpenAiReasoningEffort}
				isGeminiThinkingLevel={isGeminiThinkingLevel}
				openAiDefaultReasoningEffortForModel={
					openAiDefaultReasoningEffortForModel
				}
				formatThinkingBudgetShort={formatThinkingBudgetShort}
				isSavingProfile={updateRewriteProgramPromptProfiles.isPending}
				errorToMessage={formatErrorMessage}
				quickAskTestInput={quickAskTestInput}
				quickAskTestOutput={quickAskTestOutput}
				quickAskTestError={quickAskTestError}
				quickAskTestDurationMs={quickAskTestDurationMs}
				quickAskTestPending={quickAskTestPending}
				quickAskTestStartRef={quickAskTestStartRef}
				setQuickAskTestInput={setQuickAskTestInput}
				setQuickAskTestOutput={setQuickAskTestOutput}
				setQuickAskTestError={setQuickAskTestError}
				setQuickAskTestDurationMs={setQuickAskTestDurationMs}
				setQuickAskTestPending={setQuickAskTestPending}
			/>
		</>
	);
}
