import { type Dispatch, type SetStateAction, useEffect, useState } from "react";
import { DEFAULT_STT_LANGUAGE } from "../../../lib/sttLanguages";
import type {
	ActiveWindowOcrMode,
	AppSettings,
	QuickAskDismissMode,
	RewriteProgramPromptProfile,
} from "../../../lib/tauri";
import {
	profileSettingIsInherited,
	resolvePromptProfileFallbacks,
} from "./effectivePromptSettings";

type UsePromptSettingsProfileStateOptions = {
	activeProfileId: string;
	activeProfile: RewriteProgramPromptProfile | null;
	profiles: RewriteProgramPromptProfile[];
	settings: AppSettings | undefined;
	defaultRewriteEnabled: boolean;
	defaultQuickReplaceSystemPrompt: string;
	defaultSttTimeout: number;
};

type Setter<T> = Dispatch<SetStateAction<T>>;

type PromptSettingsProfileState = {
	localProfileSttProvider: string | null;
	setLocalProfileSttProvider: Setter<string | null>;
	localProfileSttModel: string | null;
	setLocalProfileSttModel: Setter<string | null>;
	localProfileSttLanguage: string;
	setLocalProfileSttLanguage: Setter<string>;
	localProfileLlmProvider: string | null;
	setLocalProfileLlmProvider: Setter<string | null>;
	localProfileLlmModel: string | null;
	setLocalProfileLlmModel: Setter<string | null>;
	localProfileQuickAskProvider: string | null;
	setLocalProfileQuickAskProvider: Setter<string | null>;
	localProfileQuickAskModel: string | null;
	setLocalProfileQuickAskModel: Setter<string | null>;
	localProfileQuickAskDismissMode: QuickAskDismissMode;
	setLocalProfileQuickAskDismissMode: Setter<QuickAskDismissMode>;
	localQuickAskSystemPrompt: string;
	setLocalQuickAskSystemPrompt: Setter<string>;
	localProfileQuickReplaceEnabled: boolean;
	setLocalProfileQuickReplaceEnabled: Setter<boolean>;
	localProfileQuickReplaceProvider: string | null;
	setLocalProfileQuickReplaceProvider: Setter<string | null>;
	localProfileQuickReplaceModel: string | null;
	setLocalProfileQuickReplaceModel: Setter<string | null>;
	localQuickReplaceSystemPrompt: string;
	setLocalQuickReplaceSystemPrompt: Setter<string>;
	localProfileRewriteIncludeClipboardContext: boolean;
	setLocalProfileRewriteIncludeClipboardContext: Setter<boolean>;
	localProfileQuickReplaceIncludeClipboardContext: boolean;
	setLocalProfileQuickReplaceIncludeClipboardContext: Setter<boolean>;
	localProfileQuickAskIncludeClipboardContext: boolean;
	setLocalProfileQuickAskIncludeClipboardContext: Setter<boolean>;
	localProfileRewriteActiveWindowOcrMode: ActiveWindowOcrMode;
	setLocalProfileRewriteActiveWindowOcrMode: Setter<ActiveWindowOcrMode>;
	localProfileQuickReplaceActiveWindowOcrMode: ActiveWindowOcrMode;
	setLocalProfileQuickReplaceActiveWindowOcrMode: Setter<ActiveWindowOcrMode>;
	localProfileQuickAskActiveWindowOcrMode: ActiveWindowOcrMode;
	setLocalProfileQuickAskActiveWindowOcrMode: Setter<ActiveWindowOcrMode>;
	localProfileOpenAiReasoningEffort: string;
	setLocalProfileOpenAiReasoningEffort: Setter<string>;
	localProfileGeminiThinkingLevel: string;
	setLocalProfileGeminiThinkingLevel: Setter<string>;
	localProfileGeminiThinkingBudget: string;
	setLocalProfileGeminiThinkingBudget: Setter<string>;
	localProfileAnthropicThinkingBudget: string;
	setLocalProfileAnthropicThinkingBudget: Setter<string>;
	localProfileRewriteEnabled: boolean;
	setLocalProfileRewriteEnabled: Setter<boolean>;
	localProfileSttTimeout: string | number;
	setLocalProfileSttTimeout: Setter<string | number>;
	localProfileQuickAskOpenAiReasoningEffort: string;
	setLocalProfileQuickAskOpenAiReasoningEffort: Setter<string>;
	localProfileQuickAskGeminiThinkingLevel: string;
	setLocalProfileQuickAskGeminiThinkingLevel: Setter<string>;
	localProfileQuickAskGeminiThinkingBudget: string;
	setLocalProfileQuickAskGeminiThinkingBudget: Setter<string>;
	localProfileQuickAskAnthropicThinkingBudget: string;
	setLocalProfileQuickAskAnthropicThinkingBudget: Setter<string>;
	sttProviderInheriting: boolean;
	setSttProviderInheriting: Setter<boolean>;
	sttModelInheriting: boolean;
	setSttModelInheriting: Setter<boolean>;
	sttLanguageInheriting: boolean;
	setSttLanguageInheriting: Setter<boolean>;
	sttTimeoutInheriting: boolean;
	setSttTimeoutInheriting: Setter<boolean>;
	llmProviderInheriting: boolean;
	setLlmProviderInheriting: Setter<boolean>;
	llmModelInheriting: boolean;
	setLlmModelInheriting: Setter<boolean>;
	rewriteEnabledInheriting: boolean;
	setRewriteEnabledInheriting: Setter<boolean>;
	rewriteIncludeClipboardContextInheriting: boolean;
	setRewriteIncludeClipboardContextInheriting: Setter<boolean>;
	quickReplaceIncludeClipboardContextInheriting: boolean;
	setQuickReplaceIncludeClipboardContextInheriting: Setter<boolean>;
	quickAskIncludeClipboardContextInheriting: boolean;
	setQuickAskIncludeClipboardContextInheriting: Setter<boolean>;
	rewriteActiveWindowOcrModeInheriting: boolean;
	setRewriteActiveWindowOcrModeInheriting: Setter<boolean>;
	quickReplaceActiveWindowOcrModeInheriting: boolean;
	setQuickReplaceActiveWindowOcrModeInheriting: Setter<boolean>;
	quickAskActiveWindowOcrModeInheriting: boolean;
	setQuickAskActiveWindowOcrModeInheriting: Setter<boolean>;
	openAiReasoningEffortInheriting: boolean;
	setOpenAiReasoningEffortInheriting: Setter<boolean>;
	geminiThinkingLevelInheriting: boolean;
	setGeminiThinkingLevelInheriting: Setter<boolean>;
	geminiThinkingBudgetInheriting: boolean;
	setGeminiThinkingBudgetInheriting: Setter<boolean>;
	anthropicThinkingBudgetInheriting: boolean;
	setAnthropicThinkingBudgetInheriting: Setter<boolean>;
	quickAskProviderInheriting: boolean;
	setQuickAskProviderInheriting: Setter<boolean>;
	quickAskModelInheriting: boolean;
	setQuickAskModelInheriting: Setter<boolean>;
	quickAskSystemPromptInheriting: boolean;
	setQuickAskSystemPromptInheriting: Setter<boolean>;
	quickAskDismissModeInheriting: boolean;
	setQuickAskDismissModeInheriting: Setter<boolean>;
	quickReplaceEnabledInheriting: boolean;
	setQuickReplaceEnabledInheriting: Setter<boolean>;
	quickReplaceProviderInheriting: boolean;
	setQuickReplaceProviderInheriting: Setter<boolean>;
	quickReplaceModelInheriting: boolean;
	setQuickReplaceModelInheriting: Setter<boolean>;
	quickReplaceSystemPromptInheriting: boolean;
	setQuickReplaceSystemPromptInheriting: Setter<boolean>;
	quickAskOpenAiReasoningEffortInheriting: boolean;
	setQuickAskOpenAiReasoningEffortInheriting: Setter<boolean>;
	quickAskGeminiThinkingLevelInheriting: boolean;
	setQuickAskGeminiThinkingLevelInheriting: Setter<boolean>;
	quickAskGeminiThinkingBudgetInheriting: boolean;
	setQuickAskGeminiThinkingBudgetInheriting: Setter<boolean>;
	quickAskAnthropicThinkingBudgetInheriting: boolean;
	setQuickAskAnthropicThinkingBudgetInheriting: Setter<boolean>;
};

export function usePromptSettingsProfileState({
	activeProfileId,
	activeProfile,
	profiles,
	settings,
	defaultRewriteEnabled,
	defaultQuickReplaceSystemPrompt,
	defaultSttTimeout,
}: UsePromptSettingsProfileStateOptions): PromptSettingsProfileState {
	const [localProfileSttProvider, setLocalProfileSttProvider] = useState<
		string | null
	>(null);
	const [localProfileSttModel, setLocalProfileSttModel] = useState<
		string | null
	>(null);
	const [localProfileSttLanguage, setLocalProfileSttLanguage] =
		useState<string>(DEFAULT_STT_LANGUAGE);
	const [localProfileLlmProvider, setLocalProfileLlmProvider] = useState<
		string | null
	>(null);
	const [localProfileLlmModel, setLocalProfileLlmModel] = useState<
		string | null
	>(null);

	const [localProfileQuickAskProvider, setLocalProfileQuickAskProvider] =
		useState<string | null>(null);
	const [localProfileQuickAskModel, setLocalProfileQuickAskModel] = useState<
		string | null
	>(null);
	const [localProfileQuickAskDismissMode, setLocalProfileQuickAskDismissMode] =
		useState<QuickAskDismissMode>("manual");
	const [localQuickAskSystemPrompt, setLocalQuickAskSystemPrompt] =
		useState<string>("");

	const [localProfileQuickReplaceEnabled, setLocalProfileQuickReplaceEnabled] =
		useState<boolean>(false);
	const [
		localProfileQuickReplaceProvider,
		setLocalProfileQuickReplaceProvider,
	] = useState<string | null>(null);
	const [localProfileQuickReplaceModel, setLocalProfileQuickReplaceModel] =
		useState<string | null>(null);
	const [localQuickReplaceSystemPrompt, setLocalQuickReplaceSystemPrompt] =
		useState<string>("");

	const [
		localProfileRewriteIncludeClipboardContext,
		setLocalProfileRewriteIncludeClipboardContext,
	] = useState<boolean>(false);
	const [
		localProfileQuickReplaceIncludeClipboardContext,
		setLocalProfileQuickReplaceIncludeClipboardContext,
	] = useState<boolean>(false);
	const [
		localProfileQuickAskIncludeClipboardContext,
		setLocalProfileQuickAskIncludeClipboardContext,
	] = useState<boolean>(false);

	const [
		localProfileRewriteActiveWindowOcrMode,
		setLocalProfileRewriteActiveWindowOcrMode,
	] = useState<ActiveWindowOcrMode>("off");
	const [
		localProfileQuickReplaceActiveWindowOcrMode,
		setLocalProfileQuickReplaceActiveWindowOcrMode,
	] = useState<ActiveWindowOcrMode>("off");
	const [
		localProfileQuickAskActiveWindowOcrMode,
		setLocalProfileQuickAskActiveWindowOcrMode,
	] = useState<ActiveWindowOcrMode>("off");

	// Per-profile thinking/reasoning knobs (stored on the profile object).
	// In UI, SELECT_DEFAULT means "inherit from Default/global settings".
	const [
		localProfileOpenAiReasoningEffort,
		setLocalProfileOpenAiReasoningEffort,
	] = useState<string>("default");
	const [localProfileGeminiThinkingLevel, setLocalProfileGeminiThinkingLevel] =
		useState<string>("default");
	const [
		localProfileGeminiThinkingBudget,
		setLocalProfileGeminiThinkingBudget,
	] = useState<string>("default");
	const [
		localProfileAnthropicThinkingBudget,
		setLocalProfileAnthropicThinkingBudget,
	] = useState<string>("default");
	const [localProfileRewriteEnabled, setLocalProfileRewriteEnabled] =
		useState<boolean>(false);
	const [localProfileSttTimeout, setLocalProfileSttTimeout] = useState<
		string | number
	>(defaultSttTimeout);

	const [
		localProfileQuickAskOpenAiReasoningEffort,
		setLocalProfileQuickAskOpenAiReasoningEffort,
	] = useState<string>("default");
	const [
		localProfileQuickAskGeminiThinkingLevel,
		setLocalProfileQuickAskGeminiThinkingLevel,
	] = useState<string>("default");
	const [
		localProfileQuickAskGeminiThinkingBudget,
		setLocalProfileQuickAskGeminiThinkingBudget,
	] = useState<string>("default");
	const [
		localProfileQuickAskAnthropicThinkingBudget,
		setLocalProfileQuickAskAnthropicThinkingBudget,
	] = useState<string>("default");

	// Track whether profile settings are inheriting (original value was null)
	const [sttProviderInheriting, setSttProviderInheriting] = useState(false);
	const [sttModelInheriting, setSttModelInheriting] = useState(false);
	const [sttLanguageInheriting, setSttLanguageInheriting] = useState(false);
	const [sttTimeoutInheriting, setSttTimeoutInheriting] = useState(false);
	const [llmProviderInheriting, setLlmProviderInheriting] = useState(false);
	const [llmModelInheriting, setLlmModelInheriting] = useState(false);
	const [rewriteEnabledInheriting, setRewriteEnabledInheriting] =
		useState(false);

	const [
		rewriteIncludeClipboardContextInheriting,
		setRewriteIncludeClipboardContextInheriting,
	] = useState(false);
	const [
		quickReplaceIncludeClipboardContextInheriting,
		setQuickReplaceIncludeClipboardContextInheriting,
	] = useState(false);
	const [
		quickAskIncludeClipboardContextInheriting,
		setQuickAskIncludeClipboardContextInheriting,
	] = useState(false);

	const [
		rewriteActiveWindowOcrModeInheriting,
		setRewriteActiveWindowOcrModeInheriting,
	] = useState(false);
	const [
		quickReplaceActiveWindowOcrModeInheriting,
		setQuickReplaceActiveWindowOcrModeInheriting,
	] = useState(false);
	const [
		quickAskActiveWindowOcrModeInheriting,
		setQuickAskActiveWindowOcrModeInheriting,
	] = useState(false);

	const [openAiReasoningEffortInheriting, setOpenAiReasoningEffortInheriting] =
		useState(false);
	const [geminiThinkingLevelInheriting, setGeminiThinkingLevelInheriting] =
		useState(false);
	const [geminiThinkingBudgetInheriting, setGeminiThinkingBudgetInheriting] =
		useState(false);
	const [
		anthropicThinkingBudgetInheriting,
		setAnthropicThinkingBudgetInheriting,
	] = useState(false);

	const [quickAskProviderInheriting, setQuickAskProviderInheriting] =
		useState(false);
	const [quickAskModelInheriting, setQuickAskModelInheriting] = useState(false);
	const [quickAskSystemPromptInheriting, setQuickAskSystemPromptInheriting] =
		useState(false);
	const [quickAskDismissModeInheriting, setQuickAskDismissModeInheriting] =
		useState(false);

	const [quickReplaceEnabledInheriting, setQuickReplaceEnabledInheriting] =
		useState(false);
	const [quickReplaceProviderInheriting, setQuickReplaceProviderInheriting] =
		useState(false);
	const [quickReplaceModelInheriting, setQuickReplaceModelInheriting] =
		useState(false);
	const [
		quickReplaceSystemPromptInheriting,
		setQuickReplaceSystemPromptInheriting,
	] = useState(false);

	const [
		quickAskOpenAiReasoningEffortInheriting,
		setQuickAskOpenAiReasoningEffortInheriting,
	] = useState(false);
	const [
		quickAskGeminiThinkingLevelInheriting,
		setQuickAskGeminiThinkingLevelInheriting,
	] = useState(false);
	const [
		quickAskGeminiThinkingBudgetInheriting,
		setQuickAskGeminiThinkingBudgetInheriting,
	] = useState(false);
	const [
		quickAskAnthropicThinkingBudgetInheriting,
		setQuickAskAnthropicThinkingBudgetInheriting,
	] = useState(false);

	useEffect(() => {
		if (activeProfile) {
			// Track whether each setting is inheriting (null in the profile)
			const sttProviderIsNull = profileSettingIsInherited(
				activeProfile,
				"stt_provider",
			);
			const sttModelIsNull = profileSettingIsInherited(
				activeProfile,
				"stt_model",
			);
			const sttLanguageIsNull = profileSettingIsInherited(
				activeProfile,
				"stt_language",
			);
			const sttTimeoutIsNull = profileSettingIsInherited(
				activeProfile,
				"stt_timeout_seconds",
			);
			const llmProviderIsNull = profileSettingIsInherited(
				activeProfile,
				"llm_provider",
			);
			const llmModelIsNull = profileSettingIsInherited(
				activeProfile,
				"llm_model",
			);
			const rewriteEnabledIsNull = profileSettingIsInherited(
				activeProfile,
				"rewrite_llm_enabled",
			);

			const openAiReasoningEffortIsNull = profileSettingIsInherited(
				activeProfile,
				"openai_reasoning_effort",
			);
			const geminiThinkingLevelIsNull = profileSettingIsInherited(
				activeProfile,
				"gemini_thinking_level",
			);
			const geminiThinkingBudgetIsNull = profileSettingIsInherited(
				activeProfile,
				"gemini_thinking_budget",
			);
			const anthropicThinkingBudgetIsNull = profileSettingIsInherited(
				activeProfile,
				"anthropic_thinking_budget",
			);

			const quickAskProviderIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_ask_provider",
			);
			const quickAskModelIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_ask_model",
			);
			const quickAskSystemPromptIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_ask_system_prompt",
			);
			const quickAskDismissModeIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_ask_dismiss_mode",
			);

			const defaultProfile = profiles.find((p) => p.id === "default") ?? null;
			const fallbackSettings = {
				quick_replace_enabled: settings?.quick_replace_enabled,
				llm_provider: settings?.llm_provider,
				llm_model: settings?.llm_model,
				rewrite_active_window_ocr_mode:
					settings?.rewrite_active_window_ocr_mode,
				quick_replace_active_window_ocr_mode:
					settings?.quick_replace_active_window_ocr_mode,
				quick_ask_active_window_ocr_mode:
					settings?.quick_ask_active_window_ocr_mode,
				quick_ask_dismiss_mode: settings?.quick_ask_dismiss_mode,
			};

			const {
				baseQuickReplaceEnabled,
				baseQuickReplaceProvider,
				baseQuickReplaceModel,
				baseQuickReplaceSystemPrompt,
				baseRewriteIncludeClipboardContext,
				baseQuickReplaceIncludeClipboardContext,
				baseQuickAskIncludeClipboardContext,
				baseRewriteActiveWindowOcrMode,
				baseQuickReplaceActiveWindowOcrMode,
				baseQuickAskActiveWindowOcrMode,
				baseQuickAskDismissMode,
			} = resolvePromptProfileFallbacks({
				defaultProfile,
				settings: fallbackSettings,
				defaultQuickReplaceSystemPrompt,
			});

			const quickReplaceEnabledIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_replace_enabled",
			);
			const quickReplaceProviderIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_replace_provider",
			);
			const quickReplaceModelIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_replace_model",
			);
			const quickReplaceSystemPromptIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_replace_system_prompt",
			);

			const rewriteIncludeClipboardContextIsNull = profileSettingIsInherited(
				activeProfile,
				"rewrite_include_clipboard_context",
			);
			const quickReplaceIncludeClipboardContextIsNull =
				profileSettingIsInherited(
					activeProfile,
					"quick_replace_include_clipboard_context",
				);
			const quickAskIncludeClipboardContextIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_ask_include_clipboard_context",
			);

			const rewriteActiveWindowOcrModeIsNull = profileSettingIsInherited(
				activeProfile,
				"rewrite_active_window_ocr_mode",
			);
			const quickReplaceActiveWindowOcrModeIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_replace_active_window_ocr_mode",
			);
			const quickAskActiveWindowOcrModeIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_ask_active_window_ocr_mode",
			);

			const quickAskOpenAiReasoningEffortIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_ask_openai_reasoning_effort",
			);
			const quickAskGeminiThinkingLevelIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_ask_gemini_thinking_level",
			);
			const quickAskGeminiThinkingBudgetIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_ask_gemini_thinking_budget",
			);
			const quickAskAnthropicThinkingBudgetIsNull = profileSettingIsInherited(
				activeProfile,
				"quick_ask_anthropic_thinking_budget",
			);

			setSttProviderInheriting(sttProviderIsNull);
			setSttModelInheriting(sttModelIsNull);
			setSttLanguageInheriting(sttLanguageIsNull);
			setSttTimeoutInheriting(sttTimeoutIsNull);
			setLlmProviderInheriting(llmProviderIsNull);
			setLlmModelInheriting(llmModelIsNull);
			setRewriteEnabledInheriting(rewriteEnabledIsNull);

			setOpenAiReasoningEffortInheriting(openAiReasoningEffortIsNull);
			setGeminiThinkingLevelInheriting(geminiThinkingLevelIsNull);
			setGeminiThinkingBudgetInheriting(geminiThinkingBudgetIsNull);
			setAnthropicThinkingBudgetInheriting(anthropicThinkingBudgetIsNull);

			setQuickAskProviderInheriting(quickAskProviderIsNull);
			setQuickAskModelInheriting(quickAskModelIsNull);
			setQuickAskSystemPromptInheriting(quickAskSystemPromptIsNull);
			setQuickAskDismissModeInheriting(quickAskDismissModeIsNull);

			setQuickReplaceEnabledInheriting(quickReplaceEnabledIsNull);
			setQuickReplaceProviderInheriting(quickReplaceProviderIsNull);
			setQuickReplaceModelInheriting(quickReplaceModelIsNull);
			setQuickReplaceSystemPromptInheriting(quickReplaceSystemPromptIsNull);

			setRewriteIncludeClipboardContextInheriting(
				rewriteIncludeClipboardContextIsNull,
			);
			setQuickReplaceIncludeClipboardContextInheriting(
				quickReplaceIncludeClipboardContextIsNull,
			);
			setQuickAskIncludeClipboardContextInheriting(
				quickAskIncludeClipboardContextIsNull,
			);

			setRewriteActiveWindowOcrModeInheriting(rewriteActiveWindowOcrModeIsNull);
			setQuickReplaceActiveWindowOcrModeInheriting(
				quickReplaceActiveWindowOcrModeIsNull,
			);
			setQuickAskActiveWindowOcrModeInheriting(
				quickAskActiveWindowOcrModeIsNull,
			);

			setQuickAskOpenAiReasoningEffortInheriting(
				quickAskOpenAiReasoningEffortIsNull,
			);
			setQuickAskGeminiThinkingLevelInheriting(
				quickAskGeminiThinkingLevelIsNull,
			);
			setQuickAskGeminiThinkingBudgetInheriting(
				quickAskGeminiThinkingBudgetIsNull,
			);
			setQuickAskAnthropicThinkingBudgetInheriting(
				quickAskAnthropicThinkingBudgetIsNull,
			);

			// Set local state (falling back to global defaults for display)
			setLocalProfileSttProvider(
				activeProfile.stt_provider ?? settings?.stt_provider ?? null,
			);
			setLocalProfileSttModel(
				activeProfile.stt_model ?? settings?.stt_model ?? null,
			);
			setLocalProfileSttLanguage(
				activeProfile.stt_language ??
					settings?.stt_language ??
					DEFAULT_STT_LANGUAGE,
			);
			setLocalProfileLlmProvider(
				activeProfile.llm_provider ?? settings?.llm_provider ?? null,
			);
			setLocalProfileLlmModel(
				activeProfile.llm_model ?? settings?.llm_model ?? null,
			);

			setLocalProfileQuickAskProvider(
				activeProfile.quick_ask_provider ??
					settings?.quick_ask_provider ??
					settings?.llm_provider ??
					null,
			);
			setLocalProfileQuickAskModel(
				activeProfile.quick_ask_model ??
					settings?.quick_ask_model ??
					settings?.llm_model ??
					null,
			);
			setLocalProfileQuickAskDismissMode(
				activeProfileId === "default"
					? (activeProfile.quick_ask_dismiss_mode ??
							settings?.quick_ask_dismiss_mode ??
							"manual")
					: (activeProfile.quick_ask_dismiss_mode ?? baseQuickAskDismissMode),
			);
			setLocalQuickAskSystemPrompt(
				activeProfile.quick_ask_system_prompt ??
					settings?.quick_ask_system_prompt ??
					"",
			);

			setLocalProfileQuickReplaceEnabled(
				activeProfileId === "default"
					? typeof activeProfile.quick_replace_enabled === "boolean"
						? activeProfile.quick_replace_enabled
						: (settings?.quick_replace_enabled ?? false)
					: (activeProfile.quick_replace_enabled ?? baseQuickReplaceEnabled),
			);
			setLocalProfileQuickReplaceProvider(
				activeProfileId === "default"
					? (activeProfile.quick_replace_provider ??
							settings?.llm_provider ??
							null)
					: (activeProfile.quick_replace_provider ?? baseQuickReplaceProvider),
			);
			setLocalProfileQuickReplaceModel(
				activeProfileId === "default"
					? (activeProfile.quick_replace_model ?? settings?.llm_model ?? null)
					: (activeProfile.quick_replace_model ?? baseQuickReplaceModel),
			);
			setLocalQuickReplaceSystemPrompt(
				activeProfileId === "default"
					? (activeProfile.quick_replace_system_prompt ??
							defaultQuickReplaceSystemPrompt)
					: (activeProfile.quick_replace_system_prompt ??
							baseQuickReplaceSystemPrompt),
			);

			setLocalProfileRewriteIncludeClipboardContext(
				activeProfileId === "default"
					? typeof activeProfile.rewrite_include_clipboard_context === "boolean"
						? activeProfile.rewrite_include_clipboard_context
						: false
					: (activeProfile.rewrite_include_clipboard_context ??
							baseRewriteIncludeClipboardContext),
			);

			setLocalProfileQuickReplaceIncludeClipboardContext(
				activeProfileId === "default"
					? typeof activeProfile.quick_replace_include_clipboard_context ===
						"boolean"
						? activeProfile.quick_replace_include_clipboard_context
						: false
					: (activeProfile.quick_replace_include_clipboard_context ??
							baseQuickReplaceIncludeClipboardContext),
			);

			setLocalProfileQuickAskIncludeClipboardContext(
				activeProfileId === "default"
					? typeof activeProfile.quick_ask_include_clipboard_context ===
						"boolean"
						? activeProfile.quick_ask_include_clipboard_context
						: false
					: (activeProfile.quick_ask_include_clipboard_context ??
							baseQuickAskIncludeClipboardContext),
			);

			setLocalProfileRewriteActiveWindowOcrMode(
				activeProfileId === "default"
					? (activeProfile.rewrite_active_window_ocr_mode ??
							settings?.rewrite_active_window_ocr_mode ??
							"off")
					: (activeProfile.rewrite_active_window_ocr_mode ??
							baseRewriteActiveWindowOcrMode),
			);
			setLocalProfileQuickReplaceActiveWindowOcrMode(
				activeProfileId === "default"
					? (activeProfile.quick_replace_active_window_ocr_mode ??
							settings?.quick_replace_active_window_ocr_mode ??
							"off")
					: (activeProfile.quick_replace_active_window_ocr_mode ??
							baseQuickReplaceActiveWindowOcrMode),
			);
			setLocalProfileQuickAskActiveWindowOcrMode(
				activeProfileId === "default"
					? (activeProfile.quick_ask_active_window_ocr_mode ??
							settings?.quick_ask_active_window_ocr_mode ??
							"off")
					: (activeProfile.quick_ask_active_window_ocr_mode ??
							baseQuickAskActiveWindowOcrMode),
			);

			setLocalProfileOpenAiReasoningEffort(
				activeProfile.openai_reasoning_effort ?? "default",
			);
			setLocalProfileGeminiThinkingLevel(
				activeProfile.gemini_thinking_level ?? "default",
			);
			setLocalProfileGeminiThinkingBudget(
				activeProfile.gemini_thinking_budget == null
					? "default"
					: String(activeProfile.gemini_thinking_budget),
			);
			setLocalProfileAnthropicThinkingBudget(
				activeProfile.anthropic_thinking_budget == null
					? "default"
					: String(activeProfile.anthropic_thinking_budget),
			);
			setLocalProfileRewriteEnabled(
				activeProfile.rewrite_llm_enabled ?? defaultRewriteEnabled,
			);
			setLocalProfileSttTimeout(
				activeProfile.stt_timeout_seconds ??
					settings?.stt_timeout_seconds ??
					defaultSttTimeout,
			);

			setLocalProfileQuickAskOpenAiReasoningEffort(
				activeProfile.quick_ask_openai_reasoning_effort ?? "default",
			);
			setLocalProfileQuickAskGeminiThinkingLevel(
				activeProfile.quick_ask_gemini_thinking_level ?? "default",
			);
			setLocalProfileQuickAskGeminiThinkingBudget(
				activeProfile.quick_ask_gemini_thinking_budget == null
					? "default"
					: String(activeProfile.quick_ask_gemini_thinking_budget),
			);
			setLocalProfileQuickAskAnthropicThinkingBudget(
				activeProfile.quick_ask_anthropic_thinking_budget == null
					? "default"
					: String(activeProfile.quick_ask_anthropic_thinking_budget),
			);
		} else {
			// Default scope - not inheriting
			setSttProviderInheriting(false);
			setSttModelInheriting(false);
			setSttLanguageInheriting(false);
			setSttTimeoutInheriting(false);
			setLlmProviderInheriting(false);
			setLlmModelInheriting(false);
			setRewriteEnabledInheriting(false);

			setOpenAiReasoningEffortInheriting(false);
			setGeminiThinkingLevelInheriting(false);
			setGeminiThinkingBudgetInheriting(false);
			setAnthropicThinkingBudgetInheriting(false);

			setQuickAskProviderInheriting(false);
			setQuickAskModelInheriting(false);
			setQuickAskSystemPromptInheriting(false);
			setQuickAskDismissModeInheriting(false);

			setQuickReplaceEnabledInheriting(false);
			setQuickReplaceProviderInheriting(false);
			setQuickReplaceModelInheriting(false);
			setQuickReplaceSystemPromptInheriting(false);

			setRewriteIncludeClipboardContextInheriting(false);
			setQuickReplaceIncludeClipboardContextInheriting(false);
			setQuickAskIncludeClipboardContextInheriting(false);
			setRewriteActiveWindowOcrModeInheriting(false);
			setQuickReplaceActiveWindowOcrModeInheriting(false);
			setQuickAskActiveWindowOcrModeInheriting(false);

			setQuickAskOpenAiReasoningEffortInheriting(false);
			setQuickAskGeminiThinkingLevelInheriting(false);
			setQuickAskGeminiThinkingBudgetInheriting(false);
			setQuickAskAnthropicThinkingBudgetInheriting(false);

			setLocalProfileSttProvider(null);
			setLocalProfileSttModel(null);
			setLocalProfileSttLanguage(
				settings?.stt_language ?? DEFAULT_STT_LANGUAGE,
			);
			setLocalProfileLlmProvider(null);
			setLocalProfileLlmModel(null);
			setLocalProfileQuickAskProvider(null);
			setLocalProfileQuickAskModel(null);
			setLocalProfileQuickAskDismissMode(
				settings?.quick_ask_dismiss_mode ?? "manual",
			);
			setLocalQuickAskSystemPrompt(settings?.quick_ask_system_prompt ?? "");

			setLocalProfileQuickReplaceEnabled(
				settings?.quick_replace_enabled ?? false,
			);
			setLocalProfileQuickReplaceProvider(settings?.llm_provider ?? null);
			setLocalProfileQuickReplaceModel(settings?.llm_model ?? null);
			setLocalQuickReplaceSystemPrompt(defaultQuickReplaceSystemPrompt);

			setLocalProfileRewriteIncludeClipboardContext(false);
			setLocalProfileQuickReplaceIncludeClipboardContext(false);
			setLocalProfileQuickAskIncludeClipboardContext(false);
			setLocalProfileRewriteActiveWindowOcrMode(
				settings?.rewrite_active_window_ocr_mode ?? "off",
			);
			setLocalProfileQuickReplaceActiveWindowOcrMode(
				settings?.quick_replace_active_window_ocr_mode ?? "off",
			);
			setLocalProfileQuickAskActiveWindowOcrMode(
				settings?.quick_ask_active_window_ocr_mode ?? "off",
			);
			setLocalProfileRewriteEnabled(defaultRewriteEnabled);
			setLocalProfileSttTimeout(
				settings?.stt_timeout_seconds ?? defaultSttTimeout,
			);

			setLocalProfileOpenAiReasoningEffort("default");
			setLocalProfileGeminiThinkingLevel("default");
			setLocalProfileGeminiThinkingBudget("default");
			setLocalProfileAnthropicThinkingBudget("default");

			setLocalProfileQuickAskOpenAiReasoningEffort("default");
			setLocalProfileQuickAskGeminiThinkingLevel("default");
			setLocalProfileQuickAskGeminiThinkingBudget("default");
			setLocalProfileQuickAskAnthropicThinkingBudget("default");
		}
	}, [
		activeProfileId,
		activeProfile,
		settings?.rewrite_active_window_ocr_mode,
		settings?.quick_replace_active_window_ocr_mode,
		settings?.quick_ask_active_window_ocr_mode,
		settings?.stt_timeout_seconds,
		settings?.stt_provider,
		settings?.stt_model,
		settings?.stt_language,
		settings?.llm_provider,
		settings?.llm_model,
		settings?.quick_ask_provider,
		settings?.quick_ask_model,
		settings?.quick_ask_system_prompt,
		settings?.quick_ask_dismiss_mode,
		defaultRewriteEnabled,
		profiles,
		settings?.quick_replace_enabled,
		defaultQuickReplaceSystemPrompt,
		defaultSttTimeout,
	]);

	return {
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
	};
}
