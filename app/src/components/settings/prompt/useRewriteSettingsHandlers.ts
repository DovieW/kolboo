import { useCallback } from "react";
import type { ModelOption } from "../../../lib/modelOptions";
import type {
	AppSettings,
	OpenAiReasoningEffort,
	RewriteProgramPromptProfile,
} from "../../../lib/tauri";
import { tauriAPI } from "../../../lib/tauri";
import { SELECT_DEFAULT } from "./useThinkingOptions";

type GeminiThinkingLevel = "minimal" | "low" | "medium" | "high";

type UseRewriteSettingsHandlersOptions = {
	isDefaultScope: boolean;
	settings: AppSettings | undefined;
	defaultRewriteEnabled: boolean;
	defaultRewriteIncludeClipboardContext: boolean;
	// State setters from usePromptSettingsProfileState
	setRewriteEnabledInheriting: (v: boolean) => void;
	setLocalProfileRewriteEnabled: (v: boolean) => void;
	setRewriteIncludeClipboardContextInheriting: (v: boolean) => void;
	setLocalProfileRewriteIncludeClipboardContext: (v: boolean) => void;
	setLlmProviderInheriting: (v: boolean) => void;
	setLlmModelInheriting: (v: boolean) => void;
	setLocalProfileLlmProvider: (v: string | null) => void;
	setLocalProfileLlmModel: (v: string | null) => void;
	setOpenAiReasoningEffortInheriting: (v: boolean) => void;
	setLocalProfileOpenAiReasoningEffort: (v: string) => void;
	setGeminiThinkingLevelInheriting: (v: boolean) => void;
	setLocalProfileGeminiThinkingLevel: (v: string) => void;
	setGeminiThinkingBudgetInheriting: (v: boolean) => void;
	setLocalProfileGeminiThinkingBudget: (v: string) => void;
	setAnthropicThinkingBudgetInheriting: (v: boolean) => void;
	setLocalProfileAnthropicThinkingBudget: (v: string) => void;
	// Mutations for default scope
	updateRewriteLlmEnabled: {
		mutate: (v: boolean, opts?: { onSuccess?: () => void }) => void;
	};
	updateLLMProvider: {
		mutate: (v: string, opts?: { onSuccess?: () => void }) => void;
	};
	updateLLMModel: {
		mutate: (v: string | null, opts?: { onSuccess?: () => void }) => void;
	};
	updateOpenAiReasoningEffort: {
		mutate: (
			v: OpenAiReasoningEffort | null,
			opts?: { onSuccess?: () => void },
		) => void;
	};
	updateGeminiThinkingLevel: {
		mutate: (
			v: GeminiThinkingLevel | null,
			opts?: { onSuccess?: () => void },
		) => void;
	};
	updateGeminiThinkingBudget: {
		mutate: (v: number | null, opts?: { onSuccess?: () => void }) => void;
	};
	updateAnthropicThinkingBudget: {
		mutate: (v: number | null, opts?: { onSuccess?: () => void }) => void;
	};
	// Helper functions
	getLlmModelOptionsForProvider: (provider: string | null) => ModelOption[];
	saveProfileMetadata: (updates: Partial<RewriteProgramPromptProfile>) => void;
	openDisableOverrideDialog: (opts: {
		title: string;
		onConfirm: () => void;
	}) => void;
};

export type RewriteSettingsHandlers = {
	handleRewriteEnabledChange: (enabled: boolean) => void;
	handleDisableRewriteEnabledOverride: () => void;
	handleRewriteIncludeClipboardContextChange: (enabled: boolean) => void;
	handleDisableRewriteIncludeClipboardContextOverride: () => void;
	handleRewriteLlmProviderChange: (value: string | null) => void;
	handleDisableRewriteLlmProviderOverride: () => void;
	handleRewriteLlmModelChange: (value: string | null) => void;
	handleDisableRewriteLlmModelOverride: () => void;
	handleRewriteOpenAiThinkingChange: (value: string | null) => void;
	handleDisableRewriteOpenAiThinkingOverride: () => void;
	handleRewriteGeminiThinkingLevelChange: (value: string | null) => void;
	handleDisableRewriteGeminiThinkingLevelOverride: () => void;
	handleRewriteGeminiThinkingBudgetChange: (value: string | null) => void;
	handleDisableRewriteGeminiThinkingBudgetOverride: () => void;
	handleRewriteAnthropicThinkingBudgetChange: (value: string | null) => void;
	handleDisableRewriteAnthropicThinkingBudgetOverride: () => void;
	// Also expose default-scope handlers used by profile handlers
	handleDefaultLLMProviderChange: (value: string | null) => void;
	handleDefaultLLMModelChange: (value: string | null) => void;
	handleOpenAiThinkingChange: (value: string | null) => void;
	handleGeminiThinkingLevelChange: (value: string | null) => void;
	handleGeminiThinkingBudgetChange: (value: string | null) => void;
	handleAnthropicThinkingBudgetChange: (value: string | null) => void;
};

/**
 * Hook that provides handlers for rewrite settings changes.
 * Encapsulates logic for both default scope and profile-specific overrides.
 */
export function useRewriteSettingsHandlers({
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
}: UseRewriteSettingsHandlersOptions): RewriteSettingsHandlers {
	// ─────────────────────────────────────────────────────────────────────────
	// Default scope handlers (global settings)
	// ─────────────────────────────────────────────────────────────────────────

	const handleDefaultLLMProviderChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			updateLLMProvider.mutate(value, {
				onSuccess: () => {
					const models = getLlmModelOptionsForProvider(value);
					const firstModel = models?.[0];
					if (firstModel) {
						updateLLMModel.mutate(firstModel.value);
					}
					tauriAPI.emitSettingsChanged();
				},
			});
		},
		[getLlmModelOptionsForProvider, updateLLMModel, updateLLMProvider],
	);

	const handleDefaultLLMModelChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			updateLLMModel.mutate(value, {
				onSuccess: () => {
					tauriAPI.emitSettingsChanged();
				},
			});
		},
		[updateLLMModel],
	);

	const handleOpenAiThinkingChange = useCallback(
		(value: string | null) => {
			if (value == null || value === SELECT_DEFAULT) {
				updateOpenAiReasoningEffort.mutate(null, {
					onSuccess: () => {
						tauriAPI.emitSettingsChanged();
					},
				});
				return;
			}

			const v: OpenAiReasoningEffort | null =
				value === "none" ||
				value === "low" ||
				value === "medium" ||
				value === "high"
					? value
					: null;
			if (v == null) return;

			updateOpenAiReasoningEffort.mutate(v, {
				onSuccess: () => {
					tauriAPI.emitSettingsChanged();
				},
			});
		},
		[updateOpenAiReasoningEffort],
	);

	const handleGeminiThinkingLevelChange = useCallback(
		(value: string | null) => {
			const v =
				value === "minimal" ||
				value === "low" ||
				value === "medium" ||
				value === "high"
					? value
					: null;
			updateGeminiThinkingLevel.mutate(v, {
				onSuccess: () => {
					tauriAPI.emitSettingsChanged();
				},
			});
		},
		[updateGeminiThinkingLevel],
	);

	const handleGeminiThinkingBudgetChange = useCallback(
		(value: string | null) => {
			if (value == null || value === SELECT_DEFAULT) {
				updateGeminiThinkingBudget.mutate(null, {
					onSuccess: () => {
						tauriAPI.emitSettingsChanged();
					},
				});
				return;
			}

			const parsed = Number(value);
			if (!Number.isFinite(parsed)) return;
			updateGeminiThinkingBudget.mutate(parsed, {
				onSuccess: () => {
					tauriAPI.emitSettingsChanged();
				},
			});
		},
		[updateGeminiThinkingBudget],
	);

	const handleAnthropicThinkingBudgetChange = useCallback(
		(value: string | null) => {
			if (value == null || value === SELECT_DEFAULT) {
				updateAnthropicThinkingBudget.mutate(null, {
					onSuccess: () => {
						tauriAPI.emitSettingsChanged();
					},
				});
				return;
			}

			const parsed = Number(value);
			if (!Number.isFinite(parsed)) return;
			updateAnthropicThinkingBudget.mutate(parsed, {
				onSuccess: () => {
					tauriAPI.emitSettingsChanged();
				},
			});
		},
		[updateAnthropicThinkingBudget],
	);

	// ─────────────────────────────────────────────────────────────────────────
	// Rewrite-specific handlers (profile overrides)
	// ─────────────────────────────────────────────────────────────────────────

	const handleRewriteEnabledChange = useCallback(
		(enabled: boolean) => {
			if (isDefaultScope) {
				updateRewriteLlmEnabled.mutate(enabled, {
					onSuccess: () => {
						tauriAPI.emitSettingsChanged();
					},
				});
				return;
			}
			setRewriteEnabledInheriting(false);
			setLocalProfileRewriteEnabled(enabled);
			saveProfileMetadata({ rewrite_llm_enabled: enabled });
		},
		[
			isDefaultScope,
			saveProfileMetadata,
			setLocalProfileRewriteEnabled,
			setRewriteEnabledInheriting,
			updateRewriteLlmEnabled,
		],
	);

	const handleDisableRewriteEnabledOverride = useCallback(() => {
		openDisableOverrideDialog({
			title: "Disable Rewrite Transcription override?",
			onConfirm: () => {
				setRewriteEnabledInheriting(true);
				setLocalProfileRewriteEnabled(defaultRewriteEnabled);
				saveProfileMetadata({ rewrite_llm_enabled: null });
			},
		});
	}, [
		defaultRewriteEnabled,
		openDisableOverrideDialog,
		saveProfileMetadata,
		setLocalProfileRewriteEnabled,
		setRewriteEnabledInheriting,
	]);

	const handleRewriteIncludeClipboardContextChange = useCallback(
		(enabled: boolean) => {
			if (!isDefaultScope) {
				setRewriteIncludeClipboardContextInheriting(false);
			}
			setLocalProfileRewriteIncludeClipboardContext(enabled);
			saveProfileMetadata({ rewrite_include_clipboard_context: enabled });
		},
		[
			isDefaultScope,
			saveProfileMetadata,
			setLocalProfileRewriteIncludeClipboardContext,
			setRewriteIncludeClipboardContextInheriting,
		],
	);

	const handleDisableRewriteIncludeClipboardContextOverride =
		useCallback(() => {
			openDisableOverrideDialog({
				title: "Disable Rewrite Clipboard Context override?",
				onConfirm: () => {
					setRewriteIncludeClipboardContextInheriting(true);
					setLocalProfileRewriteIncludeClipboardContext(
						defaultRewriteIncludeClipboardContext,
					);
					saveProfileMetadata({ rewrite_include_clipboard_context: null });
				},
			});
		}, [
			defaultRewriteIncludeClipboardContext,
			openDisableOverrideDialog,
			saveProfileMetadata,
			setLocalProfileRewriteIncludeClipboardContext,
			setRewriteIncludeClipboardContextInheriting,
		]);

	const handleRewriteLlmProviderChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			if (isDefaultScope) {
				handleDefaultLLMProviderChange(value);
				return;
			}
			setLlmProviderInheriting(false);
			setLlmModelInheriting(false);
			setLocalProfileLlmProvider(value);
			const models = getLlmModelOptionsForProvider(value);
			const firstModel = models[0]?.value ?? null;
			setLocalProfileLlmModel(firstModel);
			saveProfileMetadata({
				llm_provider: value,
				llm_model: firstModel,
			});
		},
		[
			getLlmModelOptionsForProvider,
			handleDefaultLLMProviderChange,
			isDefaultScope,
			saveProfileMetadata,
			setLlmModelInheriting,
			setLlmProviderInheriting,
			setLocalProfileLlmModel,
			setLocalProfileLlmProvider,
		],
	);

	const handleDisableRewriteLlmProviderOverride = useCallback(() => {
		openDisableOverrideDialog({
			title: "Disable Language Model Provider override?",
			onConfirm: () => {
				setLlmProviderInheriting(true);
				setLlmModelInheriting(true);
				setLocalProfileLlmProvider(settings?.llm_provider ?? null);
				setLocalProfileLlmModel(settings?.llm_model ?? null);
				saveProfileMetadata({ llm_provider: null, llm_model: null });
			},
		});
	}, [
		openDisableOverrideDialog,
		saveProfileMetadata,
		setLlmModelInheriting,
		setLlmProviderInheriting,
		setLocalProfileLlmModel,
		setLocalProfileLlmProvider,
		settings?.llm_model,
		settings?.llm_provider,
	]);

	const handleRewriteLlmModelChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			if (isDefaultScope) {
				handleDefaultLLMModelChange(value);
				return;
			}
			setLlmModelInheriting(false);
			setLocalProfileLlmModel(value);
			saveProfileMetadata({ llm_model: value });
		},
		[
			handleDefaultLLMModelChange,
			isDefaultScope,
			saveProfileMetadata,
			setLlmModelInheriting,
			setLocalProfileLlmModel,
		],
	);

	const handleDisableRewriteLlmModelOverride = useCallback(() => {
		openDisableOverrideDialog({
			title: "Disable Rewrite LLM Model override?",
			onConfirm: () => {
				setLlmModelInheriting(true);
				setLocalProfileLlmModel(settings?.llm_model ?? null);
				saveProfileMetadata({ llm_model: null });
			},
		});
	}, [
		openDisableOverrideDialog,
		saveProfileMetadata,
		setLlmModelInheriting,
		setLocalProfileLlmModel,
		settings?.llm_model,
	]);

	const handleRewriteOpenAiThinkingChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			if (isDefaultScope) {
				handleOpenAiThinkingChange(value);
				return;
			}
			setOpenAiReasoningEffortInheriting(false);
			setLocalProfileOpenAiReasoningEffort(value);
			const effort =
				value === SELECT_DEFAULT
					? null
					: value === "none" ||
							value === "low" ||
							value === "medium" ||
							value === "high"
						? value
						: null;
			saveProfileMetadata({ openai_reasoning_effort: effort });
		},
		[
			handleOpenAiThinkingChange,
			isDefaultScope,
			saveProfileMetadata,
			setLocalProfileOpenAiReasoningEffort,
			setOpenAiReasoningEffortInheriting,
		],
	);

	const handleDisableRewriteOpenAiThinkingOverride = useCallback(() => {
		openDisableOverrideDialog({
			title: "Disable Thinking (Reasoning Effort) override?",
			onConfirm: () => {
				setOpenAiReasoningEffortInheriting(true);
				setLocalProfileOpenAiReasoningEffort(
					settings?.openai_reasoning_effort ?? SELECT_DEFAULT,
				);
				saveProfileMetadata({ openai_reasoning_effort: null });
			},
		});
	}, [
		openDisableOverrideDialog,
		saveProfileMetadata,
		setLocalProfileOpenAiReasoningEffort,
		setOpenAiReasoningEffortInheriting,
		settings?.openai_reasoning_effort,
	]);

	const handleRewriteGeminiThinkingLevelChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			if (isDefaultScope) {
				handleGeminiThinkingLevelChange(value);
				return;
			}
			setGeminiThinkingLevelInheriting(false);
			setLocalProfileGeminiThinkingLevel(value);
			const level =
				value === SELECT_DEFAULT
					? null
					: value === "minimal" ||
							value === "low" ||
							value === "medium" ||
							value === "high"
						? value
						: null;
			saveProfileMetadata({ gemini_thinking_level: level });
		},
		[
			handleGeminiThinkingLevelChange,
			isDefaultScope,
			saveProfileMetadata,
			setGeminiThinkingLevelInheriting,
			setLocalProfileGeminiThinkingLevel,
		],
	);

	const handleDisableRewriteGeminiThinkingLevelOverride = useCallback(() => {
		openDisableOverrideDialog({
			title: "Disable Thinking Level override?",
			onConfirm: () => {
				setGeminiThinkingLevelInheriting(true);
				setLocalProfileGeminiThinkingLevel(
					settings?.gemini_thinking_level ?? SELECT_DEFAULT,
				);
				saveProfileMetadata({ gemini_thinking_level: null });
			},
		});
	}, [
		openDisableOverrideDialog,
		saveProfileMetadata,
		setGeminiThinkingLevelInheriting,
		setLocalProfileGeminiThinkingLevel,
		settings?.gemini_thinking_level,
	]);

	const handleRewriteGeminiThinkingBudgetChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			if (isDefaultScope) {
				handleGeminiThinkingBudgetChange(value);
				return;
			}
			setGeminiThinkingBudgetInheriting(false);
			const parsed = value === SELECT_DEFAULT ? null : Number(value);
			setLocalProfileGeminiThinkingBudget(value);
			saveProfileMetadata({ gemini_thinking_budget: parsed });
		},
		[
			handleGeminiThinkingBudgetChange,
			isDefaultScope,
			saveProfileMetadata,
			setGeminiThinkingBudgetInheriting,
			setLocalProfileGeminiThinkingBudget,
		],
	);

	const handleDisableRewriteGeminiThinkingBudgetOverride = useCallback(() => {
		openDisableOverrideDialog({
			title: "Disable Thinking Budget override?",
			onConfirm: () => {
				setGeminiThinkingBudgetInheriting(true);
				const inherited = settings?.gemini_thinking_budget;
				setLocalProfileGeminiThinkingBudget(
					inherited == null ? SELECT_DEFAULT : String(inherited),
				);
				saveProfileMetadata({ gemini_thinking_budget: null });
			},
		});
	}, [
		openDisableOverrideDialog,
		saveProfileMetadata,
		setGeminiThinkingBudgetInheriting,
		setLocalProfileGeminiThinkingBudget,
		settings?.gemini_thinking_budget,
	]);

	const handleRewriteAnthropicThinkingBudgetChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			if (isDefaultScope) {
				handleAnthropicThinkingBudgetChange(value);
				return;
			}
			setAnthropicThinkingBudgetInheriting(false);
			const parsed = value === SELECT_DEFAULT ? null : Number(value);
			setLocalProfileAnthropicThinkingBudget(value);
			saveProfileMetadata({ anthropic_thinking_budget: parsed });
		},
		[
			handleAnthropicThinkingBudgetChange,
			isDefaultScope,
			saveProfileMetadata,
			setAnthropicThinkingBudgetInheriting,
			setLocalProfileAnthropicThinkingBudget,
		],
	);

	const handleDisableRewriteAnthropicThinkingBudgetOverride =
		useCallback(() => {
			openDisableOverrideDialog({
				title: "Disable Thinking Budget override?",
				onConfirm: () => {
					setAnthropicThinkingBudgetInheriting(true);
					const inherited = settings?.anthropic_thinking_budget;
					setLocalProfileAnthropicThinkingBudget(
						inherited == null ? SELECT_DEFAULT : String(inherited),
					);
					saveProfileMetadata({ anthropic_thinking_budget: null });
				},
			});
		}, [
			openDisableOverrideDialog,
			saveProfileMetadata,
			setAnthropicThinkingBudgetInheriting,
			setLocalProfileAnthropicThinkingBudget,
			settings?.anthropic_thinking_budget,
		]);

	return {
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
		// Default scope handlers
		handleDefaultLLMProviderChange,
		handleDefaultLLMModelChange,
		handleOpenAiThinkingChange,
		handleGeminiThinkingLevelChange,
		handleGeminiThinkingBudgetChange,
		handleAnthropicThinkingBudgetChange,
	};
}
