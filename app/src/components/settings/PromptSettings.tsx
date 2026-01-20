import {
	Accordion,
	ActionIcon,
	Button,
	Group,
	Loader,
	Select,
	Switch,
	Text,
	Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { Info, RotateCcw } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
	EMBEDDING_MODELS,
	type ModelOption,
	STT_MODELS,
} from "../../lib/modelOptions";
import {
	useAvailableProviders,
	useDefaultSections,
	useHasLastAudioForSttTest,
	useIterateRewritePrompt,
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
	useUpdateQuickAskGeminiThinkingBudget,
	useUpdateQuickAskGeminiThinkingLevel,
	useUpdateQuickAskIncludeSelectedText,
	useUpdateQuickAskModel,
	useUpdateQuickAskOpenAiReasoningEffort,
	useUpdateQuickAskProvider,
	useUpdateQuickAskSystemPrompt,
	useUpdateRewriteLlmEnabled,
	useUpdateRewriteProgramPromptProfiles,
	useUpdateSTTModel,
	useUpdateSTTProvider,
	useUpdateSTTTimeout,
	useUpdateSTTTranscriptionPrompt,
} from "../../lib/queries";
import {
	type CleanupPromptSections,
	type CleanupPromptSectionsOverride,
	type IntentRouterSettings,
	type OpenAiReasoningEffort,
	type RewritePreset,
	type RewriteProgramPromptProfile,
	tauriAPI,
} from "../../lib/tauri";
import { HintSelect } from "../HintSelect";
import { PresetEditorModal } from "./prompt/PresetEditorModal";
import { PromptIntentRouterSection } from "./prompt/PromptIntentRouterSection";
import { usePromptProviderOptions } from "./prompt/usePromptProviderOptions";
import { usePromptSettingsProfileState } from "./prompt/usePromptSettingsProfileState";
import { usePromptSettingsTests } from "./prompt/usePromptSettingsTests";
import {
	type LinkableProfileOption,
	PromptSettingsModals,
} from "./prompt/PromptSettingsModals";
import { QuickAskPanel } from "./prompt/QuickAskPanel";
import { TranscribeSettingsSection } from "./prompt/TranscribeSettingsSection";
import { QuickReplaceSettings } from "./QuickReplaceSettings";
import { RewritePromptLabModal } from "./RewritePromptLabModal";

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

const isRecord = (value: unknown): value is Record<string, unknown> => {
	return value != null && typeof value === "object";
};

const isOpenAiReasoningEffort = (
	value: unknown,
): value is OpenAiReasoningEffort => {
	return (
		value === "none" ||
		value === "minimal" ||
		value === "low" ||
		value === "medium" ||
		value === "high" ||
		value === "xhigh"
	);
};

const isGeminiThinkingLevel = (
	value: unknown,
): value is "minimal" | "low" | "medium" | "high" => {
	return (
		value === "minimal" ||
		value === "low" ||
		value === "medium" ||
		value === "high"
	);
};

function formatUsdRateFromMicros(micros: number): string {
	const safeMicros =
		typeof micros === "number" && Number.isFinite(micros) ? micros : 0;
	const dollars = safeMicros / 1_000_000;

	if (dollars > 0 && dollars < 0.01) {
		return `$${dollars.toFixed(3).replace(/0+$/, "").replace(/\.$/, "")}`;
	}

	return `$${dollars.toFixed(2).replace(/\.00$/, "")}`;
}

type SectionKey = "system";

function errorToMessage(err: unknown): string {
	if (err instanceof Error) return err.message;
	if (typeof err === "string") return err;
	if (isRecord(err)) {
		if (typeof err.message === "string") return err.message;
		if (typeof err.error === "string") return err.error;
		try {
			return JSON.stringify(err);
		} catch {
			return String(err);
		}
	}
	return String(err);
}

interface LocalSectionState {
	content: string;
}

interface LocalSections {
	system: LocalSectionState;
}

function createId(): string {
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
	const EDIT_DEFAULT_PRESET = "__default__";

	const activeProfileId = editingProfileId ?? "default";
	const isDefaultScope = activeProfileId === "default";

	const { data: settings, isLoading: isLoadingSettings } = useSettings();
	const { data: defaultSections, isLoading: isLoadingDefaultSections } =
		useDefaultSections();
	const { data: availableProviders, isLoading: isLoadingProviders } =
		useAvailableProviders();
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
	const updateSTTTranscriptionPrompt = useUpdateSTTTranscriptionPrompt();
	const updateLLMProvider = useUpdateLLMProvider();
	const updateLLMModel = useUpdateLLMModel();
	const updateOpenAiReasoningEffort = useUpdateOpenAiReasoningEffort();
	const updateAnthropicThinkingBudget = useUpdateAnthropicThinkingBudget();
	const updateGeminiThinkingBudget = useUpdateGeminiThinkingBudget();
	const updateGeminiThinkingLevel = useUpdateGeminiThinkingLevel();
	const updateSTTTimeout = useUpdateSTTTimeout();

	const updateQuickAskProvider = useUpdateQuickAskProvider();
	const updateQuickAskModel = useUpdateQuickAskModel();
	const updateQuickAskSystemPrompt = useUpdateQuickAskSystemPrompt();
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

	const defaultRewriteEnabled = settings?.rewrite_llm_enabled ?? false;

	const {
		localProfileSttProvider,
		setLocalProfileSttProvider,
		localProfileSttModel,
		setLocalProfileSttModel,
		localProfileLlmProvider,
		setLocalProfileLlmProvider,
		localProfileLlmModel,
		setLocalProfileLlmModel,
		localProfileQuickAskProvider,
		setLocalProfileQuickAskProvider,
		localProfileQuickAskModel,
		setLocalProfileQuickAskModel,
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
		errorToMessage,
		testLlmRewrite,
		testRewriteWithPrompt,
		testSttLastAudio,
	});

	const [promptLabOpen, setPromptLabOpen] = useState(false);
	const [promptLabContextPrompt, setPromptLabContextPrompt] =
		useState<string>("");
	const [promptLabContextLabel, setPromptLabContextLabel] =
		useState<string>("");
	const [promptLabApplyTarget, setPromptLabApplyTarget] = useState<
		| { type: "profile"; key: SectionKey }
		| { type: "preset"; presetId: string; key: SectionKey }
		| null
	>(null);

	const [isCachingRouterEmbeddings, setIsCachingRouterEmbeddings] =
		useState(false);

	// Presets + intent router (profile-only features).
	const presets: RewritePreset[] = useMemo(() => {
		if (!activeProfile) return [];
		return Array.isArray(activeProfile.presets) ? activeProfile.presets : [];
	}, [activeProfile]);

	const getPresetsForProfile = useCallback(
		(p: RewriteProgramPromptProfile): RewritePreset[] => {
			const raw = p.presets;
			return Array.isArray(raw) ? raw : [];
		},
		[],
	);

	const presetRefCounts = useMemo(() => {
		const counts = new Map<string, number>();
		for (const profile of profiles) {
			const seen = new Set<string>();
			for (const preset of getPresetsForProfile(profile)) {
				if (seen.has(preset.id)) continue;
				seen.add(preset.id);
				counts.set(preset.id, (counts.get(preset.id) ?? 0) + 1);
			}
		}
		return counts;
	}, [profiles, getPresetsForProfile]);

	const isSharedPresetId = (presetId: string): boolean => {
		return (presetRefCounts.get(presetId) ?? 0) > 1;
	};

	const [editingPresetId, setEditingPresetId] = useState<string | null>(null);
	const pendingPresetIdRef = useRef<string | null>(null);
	const [localPresetName, setLocalPresetName] = useState<string>("");
	const [localPresetHintsText, setLocalPresetHintsText] = useState<string>("");

	const [presetEditorOpen, setPresetEditorOpen] = useState(false);
	const [deletePresetDialog, setDeletePresetDialog] = useState<null | {
		presetId: string;
		presetName: string;
		isShared: boolean;
	}>(null);

	const [linkPresetModalOpen, setLinkPresetModalOpen] = useState(false);
	const [linkSourceProfileId, setLinkSourceProfileId] = useState<string | null>(
		null,
	);
	const [linkSourcePresetId, setLinkSourcePresetId] = useState<string | null>(
		null,
	);

	const [localDefaultPresetDescription, setLocalDefaultPresetDescription] =
		useState<string>("");

	const selectedPreset: RewritePreset | null = useMemo(() => {
		if (!activeProfile) return null;
		if (!editingPresetId) return null;
		return presets.find((p) => p.id === editingPresetId) ?? null;
	}, [activeProfile, presets, editingPresetId]);

	const isEditingDefaultPreset = editingPresetId === EDIT_DEFAULT_PRESET;

	useEffect(() => {
		if (!activeProfile) {
			setEditingPresetId(null);
			pendingPresetIdRef.current = null;
			return;
		}

		// Keep selection stable where possible.
		if (
			editingPresetId &&
			(editingPresetId === EDIT_DEFAULT_PRESET ||
				presets.some((p) => p.id === editingPresetId))
		) {
			if (
				pendingPresetIdRef.current === editingPresetId &&
				presets.some((p) => p.id === editingPresetId)
			) {
				pendingPresetIdRef.current = null;
			}
			return;
		}

		// We may have just created/linked a preset and are waiting for settings to
		// propagate back into `activeProfile.presets`. Don't snap the selection back.
		if (editingPresetId && pendingPresetIdRef.current === editingPresetId) {
			return;
		}

		setEditingPresetId(presets[0]?.id ?? EDIT_DEFAULT_PRESET);
	}, [activeProfile, presets, editingPresetId]);

	useEffect(() => {
		if (!selectedPreset) {
			setLocalPresetName("");
			setLocalPresetHintsText("");
			return;
		}

		setLocalPresetName(selectedPreset.name);
		const lines = (selectedPreset.routing_hints ?? []).filter(Boolean);
		setLocalPresetHintsText(lines.join("\n"));
	}, [selectedPreset]);

	useEffect(() => {
		if (!activeProfile) {
			setLocalDefaultPresetDescription("");
			return;
		}
		setLocalDefaultPresetDescription(
			activeProfile.default_preset_description ?? "",
		);
	}, [activeProfile]);

	const savePresets = (
		nextPresets: RewritePreset[],
		extra?: Partial<RewriteProgramPromptProfile>,
	) => {
		if (!activeProfile) return;
		saveProfileMetadata({ presets: nextPresets, ...(extra ?? {}) });
	};

	const updatePreset = (presetId: string, patch: Partial<RewritePreset>) => {
		// When a preset id appears in more than one profile, treat it like a shared
		// entity: edits in any profile update all profiles that reference that id.
		if (isSharedPresetId(presetId)) {
			const updated = profiles.map((profile) => {
				const profilePresets = getPresetsForProfile(profile);
				if (!profilePresets.some((p) => p.id === presetId)) return profile;
				return {
					...profile,
					presets: profilePresets.map((p) =>
						p.id === presetId ? { ...p, ...patch } : p,
					),
				};
			});

			updateRewriteProgramPromptProfiles.mutate(updated, {
				onSuccess: () => {
					tauriAPI.emitSettingsChanged();
				},
			});
			return;
		}

		const next = presets.map((p) =>
			p.id === presetId ? { ...p, ...patch } : p,
		);
		savePresets(next);
	};

	const deletePreset = (presetId: string) => {
		if (!activeProfile) return;
		const nextPresets = presets.filter((p) => p.id !== presetId);

		const nextProfilePatch: Partial<RewriteProgramPromptProfile> = {};
		if (activeProfile.default_preset_id === presetId) {
			nextProfilePatch.default_preset_id = null;
		}
		if (activeProfile.active_preset_id === presetId) {
			nextProfilePatch.active_preset_id = null;
		}

		savePresets(nextPresets, nextProfilePatch);
		if (editingPresetId === presetId) {
			setEditingPresetId(nextPresets[0]?.id ?? null);
		}
	};

	const newPreset = () => {
		if (!activeProfile) return;
		const id = createId();
		const p: RewritePreset = {
			id,
			name: "New preset",
			routing_hints: null,
			cleanup_prompt_sections: null,
			// Default presets to rewrite "On".
			rewrite_llm_enabled: true,
			stt_provider: null,
			stt_model: null,
			stt_timeout_seconds: null,
			llm_provider: null,
			llm_model: null,
			openai_reasoning_effort: null,
			gemini_thinking_budget: null,
			gemini_thinking_level: null,
			anthropic_thinking_budget: null,
			sound_enabled: null,
			playing_audio_handling: null,
			overlay_mode: null,
			widget_position: null,
			output_mode: null,
			output_hit_enter: null,
		};

		const next = [...presets, p];
		savePresets(next);
		pendingPresetIdRef.current = id;
		setEditingPresetId(id);
	};

	const linkableProfiles: LinkableProfileOption[] = useMemo(() => {
		return profiles
			.filter((p) => p.id !== activeProfileId)
			.map((p) => {
				const profilePresets = getPresetsForProfile(p);
				return {
					id: p.id,
					label: p.name?.trim() || p.id,
					presets: profilePresets,
				};
			})
			.filter((p) => p.presets.length > 0);
	}, [profiles, activeProfileId, getPresetsForProfile]);

	const linkSourceProfile = useMemo(() => {
		if (!linkSourceProfileId) return null;
		return linkableProfiles.find((p) => p.id === linkSourceProfileId) ?? null;
	}, [linkSourceProfileId, linkableProfiles]);

	const linkSourcePreset = useMemo(() => {
		if (!linkSourceProfile) return null;
		if (!linkSourcePresetId) return null;
		return (
			linkSourceProfile.presets.find((p) => p.id === linkSourcePresetId) ?? null
		);
	}, [linkSourceProfile, linkSourcePresetId]);

	const openLinkPresetModal = () => {
		const firstProfile = linkableProfiles[0] ?? null;
		if (!firstProfile) return;
		setLinkSourceProfileId(firstProfile.id);
		setLinkSourcePresetId(firstProfile.presets[0]?.id ?? null);
		setLinkPresetModalOpen(true);
	};

	const confirmLinkPreset = () => {
		if (!activeProfile) return;
		if (!linkSourcePreset) return;

		// If it's already linked, just switch the editor to it.
		if (presets.some((p) => p.id === linkSourcePreset.id)) {
			setEditingPresetId(linkSourcePreset.id);
			setLinkPresetModalOpen(false);
			return;
		}

		// â€œHard linkâ€ semantics: we reuse the same preset id across profiles.
		// We still store an object in this profile, but updates propagate by id.
		const next = [...presets, { ...linkSourcePreset }];
		savePresets(next);
		pendingPresetIdRef.current = linkSourcePreset.id;
		setEditingPresetId(linkSourcePreset.id);
		setLinkPresetModalOpen(false);
	};

	const handleLinkSourceProfileChange = (value: string) => {
		setLinkSourceProfileId(value);
		const nextProfile = linkableProfiles.find((p) => p.id === value) ?? null;
		setLinkSourcePresetId(nextProfile?.presets[0]?.id ?? null);
	};

	const handleLinkSourcePresetChange = (value: string) => {
		setLinkSourcePresetId(value);
	};

	const handleConfirmDeletePreset = () => {
		const args = deletePresetDialog;
		if (!args) return;
		setDeletePresetDialog(null);
		deletePreset(args.presetId);
	};

	const handleConfirmResetDialog = () => {
		const confirm = resetDialog?.onConfirm;
		setResetDialog(null);
		confirm?.();
	};

	const normalizeRouter = useCallback(
		(router: IntentRouterSettings | null | undefined): IntentRouterSettings => {
			const r: Partial<IntentRouterSettings> = router ?? {};
			const openai_reasoning_effort = isOpenAiReasoningEffort(
				r.openai_reasoning_effort,
			)
				? r.openai_reasoning_effort
				: null;
			const gemini_thinking_level = isGeminiThinkingLevel(
				r.gemini_thinking_level,
			)
				? r.gemini_thinking_level
				: null;

			return {
				enabled: Boolean(r.enabled),
				strategy:
					r.strategy === "embeddings" || r.strategy === "llm"
						? r.strategy
						: "off",
				embedding_provider:
					r.embedding_provider === "openai" ||
					r.embedding_provider === "cohere" ||
					r.embedding_provider === "fireworks"
						? r.embedding_provider
						: null,
				embedding_model:
					typeof r.embedding_model === "string" ? r.embedding_model : null,
				pick_highest_score:
					typeof r.pick_highest_score === "boolean"
						? r.pick_highest_score
						: null,
				similarity_threshold:
					typeof r.similarity_threshold === "number" &&
					Number.isFinite(r.similarity_threshold)
						? r.similarity_threshold
						: null,
				similarity_margin:
					typeof r.similarity_margin === "number" &&
					Number.isFinite(r.similarity_margin)
						? r.similarity_margin
						: null,

				llm_provider:
					typeof r.llm_provider === "string" ? r.llm_provider : null,
				llm_model: typeof r.llm_model === "string" ? r.llm_model : null,
				openai_reasoning_effort,
				gemini_thinking_budget:
					typeof r.gemini_thinking_budget === "number" &&
					Number.isFinite(r.gemini_thinking_budget)
						? r.gemini_thinking_budget
						: null,
				gemini_thinking_level,
				anthropic_thinking_budget:
					typeof r.anthropic_thinking_budget === "number" &&
					Number.isFinite(r.anthropic_thinking_budget)
						? r.anthropic_thinking_budget
						: null,
				llm_system_prompt:
					typeof r.llm_system_prompt === "string" ? r.llm_system_prompt : null,
			};
		},
		[],
	);

	const effectiveRouter: IntentRouterSettings | null = useMemo(() => {
		if (!activeProfile) return null;
		return normalizeRouter(activeProfile.router);
	}, [activeProfile, normalizeRouter]);

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
		ollamaModelsQuery,
		getLlmModelOptionsForProvider,
	} = usePromptProviderOptions({
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
		effectiveRouterLlmProvider: effectiveRouter?.llm_provider ?? null,
	});

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

	// NOTE: Settings tabs unmount when switching (keepMounted=false). If we render
	// switches immediately, they first render with placeholder values then â€œjumpâ€
	// once settings load. We avoid that by showing a loader until we have settings
	// + defaults and local state is initialized.
	const [localSections, setLocalSections] = useState<LocalSections | null>(
		null,
	);

	const effectiveCurrentPrompt = useMemo(() => {
		if (localSections == null) return "";

		return (localSections.system.content ?? "").trim();
	}, [localSections]);

	// Keep PromptLab context aligned with current scope by default.
	useEffect(() => {
		if (!promptLabOpen) {
			setPromptLabContextPrompt(effectiveCurrentPrompt);
			setPromptLabContextLabel(activeProfileLabel);
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [effectiveCurrentPrompt, activeProfileLabel, promptLabOpen]);

	// When updating per-section prompt overrides quickly (e.g., toggling Advanced then
	// Dictionary immediately), relying on `activeProfile` can drop earlier changes
	// because the component may not have re-rendered with the optimistic update yet.
	// Keep a ref of the latest per-profile prompt overrides to merge safely.
	const profilePromptOverridesRef =
		useRef<CleanupPromptSectionsOverride | null>(null);

	useEffect(() => {
		profilePromptOverridesRef.current =
			activeProfile?.cleanup_prompt_sections ?? null;
	}, [activeProfile?.cleanup_prompt_sections]);

	useEffect(() => {
		if (settings !== undefined && defaultSections !== undefined) {
			const base = settings.cleanup_prompt_sections ?? DEFAULT_SECTIONS;

			const profileOverrides: CleanupPromptSectionsOverride | null | undefined =
				activeProfileId === "default"
					? null
					: profiles.find((p) => p.id === activeProfileId)
							?.cleanup_prompt_sections;

			const resolved: CleanupPromptSections =
				activeProfileId === "default"
					? base
					: {
							system: profileOverrides?.system ?? base.system,
						};

			setLocalSections({
				system: {
					content: resolved.system.content ?? defaultSections.system,
				},
			});
		}
	}, [settings, defaultSections, activeProfileId, profiles]);

	const isLoading =
		isLoadingSettings ||
		isLoadingDefaultSections ||
		isLoadingProviders ||
		settings === undefined ||
		defaultSections === undefined ||
		localSections === null;

	// One-time migration: ensure every profile has its own rewrite enable flag.
	// This prevents the Default toggle from affecting other profiles.
	const didEnsureDefaultProfile = useRef(false);
	useEffect(() => {
		if (didEnsureDefaultProfile.current) return;
		if (!settings) return;

		// Ensure the Default profile exists as a real, persisted profile object so it can
		// own presets/router configuration.
		const hasDefault = profiles.some((p) => p.id === "default");
		if (hasDefault) {
			didEnsureDefaultProfile.current = true;
			return;
		}

		didEnsureDefaultProfile.current = true;

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
			// Default profile uses the global rewrite toggle.
			rewrite_llm_enabled: null,
			stt_provider: null,
			stt_model: null,
			stt_timeout_seconds: null,
			llm_provider: null,
			llm_model: null,
			openai_reasoning_effort: null,
			gemini_thinking_budget: null,
			gemini_thinking_level: null,
			anthropic_thinking_budget: null,

			context_grab_method: null,

			sound_enabled: null,
			playing_audio_handling: null,
			overlay_mode: null,
			widget_position: null,
			output_mode: null,
			output_hit_enter: null,
		};

		// Insert Default first so it doesn't show up as a "program profile" elsewhere.
		updateRewriteProgramPromptProfiles.mutate([defaultProfile, ...profiles], {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
			},
		});
	}, [profiles, settings, updateRewriteProgramPromptProfiles]);

	const didMigrateProfileRewriteEnabled = useRef(false);
	useEffect(() => {
		if (didMigrateProfileRewriteEnabled.current) return;
		if (!settings) return;
		if (profiles.length === 0) return;

		const needsMigration = profiles.some(
			(p) => p.id !== "default" && typeof p.rewrite_llm_enabled !== "boolean",
		);
		if (!needsMigration) {
			didMigrateProfileRewriteEnabled.current = true;
			return;
		}

		didMigrateProfileRewriteEnabled.current = true;

		const migrated = profiles.map((p) => {
			if (p.id === "default") return p;
			const current = p.rewrite_llm_enabled;
			if (typeof current === "boolean") return p;
			return { ...p, rewrite_llm_enabled: defaultRewriteEnabled };
		});

		updateRewriteProgramPromptProfiles.mutate(migrated, {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
			},
		});
	}, [
		settings,
		profiles,
		defaultRewriteEnabled,
		updateRewriteProgramPromptProfiles,
	]);

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
			updateSTTTranscriptionPrompt.mutate(toStore, {
				onSuccess: () => {
					tauriAPI.emitSettingsChanged();
				},
			});
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

		updateLLMModel.mutate(first, {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
			},
		});
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
		if (!stt) return null;

		const minSecs =
			typeof stt.min_billed_secs === "number" ? stt.min_billed_secs : null;

		const withMinBill = (base: string) =>
			minSecs ? `${base} Â· min ${minSecs}s` : base;

		if (typeof stt.usd_micros_per_hour === "number") {
			const base = `${formatUsdRateFromMicros(stt.usd_micros_per_hour)}/hr`;
			return withMinBill(base);
		}

		// Some providers report pricing as USD/minute. For consistency in the UI,
		// normalize everything to USD/hour.
		if (typeof stt.usd_micros_per_minute === "number") {
			const perHourMicros = Math.round(stt.usd_micros_per_minute * 60);
			const base = `${formatUsdRateFromMicros(perHourMicros)}/hr`;
			return withMinBill(base);
		}

		return null;
	}, [sttPricing.data]);

	const llmPricingLabel = useMemo(() => {
		const llm = llmPricing.data?.llm;
		if (!llm) return null;

		const input = formatUsdRateFromMicros(llm.input_usd_micros_per_1m);
		const output = formatUsdRateFromMicros(llm.output_usd_micros_per_1m);
		return `in ${input} Â· out ${output} /1M tok`;
	}, [llmPricing.data]);

	// Thinking controls can be overridden per profile.
	const supportsOpenAiThinking =
		effectiveLlmProvider === "openai" &&
		!!effectiveLlmModel &&
		(effectiveLlmModel.startsWith("gpt-5") ||
			effectiveLlmModel.startsWith("o"));

	const supportsGeminiThinkingLevel =
		effectiveLlmProvider === "gemini" &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-3");

	const supportsGeminiThinkingBudget =
		effectiveLlmProvider === "gemini" &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-2.5") &&
		!effectiveLlmModel.includes("flash-lite");

	const supportsAnthropicThinkingBudget =
		effectiveLlmProvider === "anthropic" &&
		!!effectiveLlmModel &&
		// Extended thinking is supported by newer Claude families. Keep conservative.
		(effectiveLlmModel.includes("claude-3-7") ||
			effectiveLlmModel.includes("claude-4") ||
			effectiveLlmModel.includes("-4-"));

	const quickAskModelForThinking =
		selectedQuickAskModelForUi ?? effectiveQuickAskModel;

	const supportsQuickAskOpenAiThinking =
		effectiveQuickAskProvider === "openai" &&
		!!quickAskModelForThinking &&
		(quickAskModelForThinking.startsWith("gpt-5") ||
			quickAskModelForThinking.startsWith("o"));

	const supportsQuickAskGeminiThinkingLevel =
		effectiveQuickAskProvider === "gemini" &&
		!!quickAskModelForThinking &&
		quickAskModelForThinking.includes("gemini-3");

	const supportsQuickAskGeminiThinkingBudget =
		effectiveQuickAskProvider === "gemini" &&
		!!quickAskModelForThinking &&
		quickAskModelForThinking.includes("gemini-2.5") &&
		!quickAskModelForThinking.includes("flash-lite");

	const supportsQuickAskAnthropicThinkingBudget =
		effectiveQuickAskProvider === "anthropic" &&
		!!quickAskModelForThinking &&
		(quickAskModelForThinking.includes("claude-3-7") ||
			quickAskModelForThinking.includes("claude-4") ||
			quickAskModelForThinking.includes("-4-"));

	// Mantine Select requires option values to be strings.
	const SELECT_DEFAULT = "default";

	const openAiThinkingEffortsForModel = (model: string): string[] => {
		// OpenAI docs (2025-12):
		// - gpt-5.1 supports: none, low, medium, high
		// - models before gpt-5.1 do not support `none`
		// - gpt-5-pro defaults to and only supports `high`
		if (model.startsWith("gpt-5-pro")) {
			return ["high"];
		}
		if (model.startsWith("gpt-5.2") || model.startsWith("gpt-5.1")) {
			return ["none", "low", "medium", "high"];
		}
		if (model.startsWith("gpt-5")) {
			return ["low", "medium", "high"];
		}
		if (model.startsWith("o")) {
			return ["low", "medium", "high"];
		}
		return [];
	};

	const openAiDefaultReasoningEffortForModel = (model: string): string => {
		// OpenAI docs (2025-12):
		// - gpt-5.1 defaults to `none`
		// - models before gpt-5.1 default to `medium`
		// - gpt-5-pro defaults to `high`
		if (model.startsWith("gpt-5-pro")) return "high";
		if (model.startsWith("gpt-5.2") || model.startsWith("gpt-5.1"))
			return "none";
		return "medium";
	};

	const openAiThinkingOptions =
		!supportsOpenAiThinking || !effectiveLlmModel
			? []
			: [
					{
						value: SELECT_DEFAULT,
						label: "Default",
					},
					...openAiThinkingEffortsForModel(effectiveLlmModel).map((v) => ({
						value: v,
						label:
							v === "none" ? "None" : v.charAt(0).toUpperCase() + v.slice(1),
					})),
				];

	const quickAskOpenAiThinkingOptions =
		!supportsQuickAskOpenAiThinking || !quickAskModelForThinking
			? []
			: [
					{
						value: SELECT_DEFAULT,
						label: "Default",
					},
					...openAiThinkingEffortsForModel(quickAskModelForThinking).map(
						(v) => ({
							value: v,
							label:
								v === "none" ? "None" : v.charAt(0).toUpperCase() + v.slice(1),
						}),
					),
				];

	const isGemini3Flash =
		supportsGeminiThinkingLevel &&
		effectiveLlmModel?.includes("gemini-3-flash");
	const isGemini3Pro =
		supportsGeminiThinkingLevel && effectiveLlmModel?.includes("gemini-3-pro");

	const isQuickAskGemini3Flash =
		supportsQuickAskGeminiThinkingLevel &&
		quickAskModelForThinking?.includes("gemini-3-flash");

	const geminiThinkingLevelOptions = isGemini3Flash
		? [
				{
					value: SELECT_DEFAULT,
					label: "Default",
				},
				{ value: "minimal", label: "Minimal" },
				{ value: "low", label: "Low" },
				{ value: "medium", label: "Medium" },
				{ value: "high", label: "High" },
			]
		: [
				{
					value: SELECT_DEFAULT,
					label: "Default",
				},
				{ value: "low", label: "Low" },
				{ value: "high", label: "High" },
			];

	const quickAskGeminiThinkingLevelOptions = isQuickAskGemini3Flash
		? [
				{
					value: SELECT_DEFAULT,
					label: "Default",
				},
				{ value: "minimal", label: "Minimal" },
				{ value: "low", label: "Low" },
				{ value: "medium", label: "Medium" },
				{ value: "high", label: "High" },
			]
		: [
				{
					value: SELECT_DEFAULT,
					label: "Default",
				},
				{ value: "low", label: "Low" },
				{ value: "high", label: "High" },
			];

	const canDisableGemini25Thinking =
		supportsGeminiThinkingBudget &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-2.5-flash") &&
		!effectiveLlmModel.includes("gemini-2.5-pro");

	const isGemini25Pro =
		supportsGeminiThinkingBudget &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-2.5-pro");

	const gemini25MaxBudget = isGemini25Pro ? 32768 : 24576;
	const gemini25MinBudget = isGemini25Pro ? 128 : 0;

	const geminiThinkingBudgetOptions: Array<{ value: string; label: string }> = [
		{ value: SELECT_DEFAULT, label: "Default" },
		{ value: "-1", label: "Dynamic (-1)" },
		...(canDisableGemini25Thinking ? [{ value: "0", label: "Off (0)" }] : []),
		...(isGemini25Pro
			? [{ value: String(gemini25MinBudget), label: "Minimal (128)" }]
			: []),
		{ value: "1024", label: "Light (1024)" },
		{ value: "4096", label: "Medium (4096)" },
		{ value: "16384", label: "High (16384)" },
		...(gemini25MaxBudget > 16384
			? [
					{
						value: String(gemini25MaxBudget),
						label: `Max (${gemini25MaxBudget})`,
					},
				]
			: []),
	];

	const canDisableQuickAskGemini25Thinking =
		supportsQuickAskGeminiThinkingBudget &&
		!!quickAskModelForThinking &&
		quickAskModelForThinking.includes("gemini-2.5-flash") &&
		!quickAskModelForThinking.includes("gemini-2.5-pro");

	const isQuickAskGemini25Pro =
		supportsQuickAskGeminiThinkingBudget &&
		!!quickAskModelForThinking &&
		quickAskModelForThinking.includes("gemini-2.5-pro");

	const quickAskGemini25MaxBudget = isQuickAskGemini25Pro ? 32768 : 24576;
	const quickAskGemini25MinBudget = isQuickAskGemini25Pro ? 128 : 0;

	const quickAskGeminiThinkingBudgetOptions: Array<{
		value: string;
		label: string;
	}> = [
		{ value: SELECT_DEFAULT, label: "Default" },
		{ value: "-1", label: "Dynamic (-1)" },
		...(canDisableQuickAskGemini25Thinking
			? [{ value: "0", label: "Off (0)" }]
			: []),
		...(isQuickAskGemini25Pro
			? [
					{
						value: String(quickAskGemini25MinBudget),
						label: "Minimal (128)",
					},
				]
			: []),
		{ value: "1024", label: "Light (1024)" },
		{ value: "4096", label: "Medium (4096)" },
		{ value: "16384", label: "High (16384)" },
		...(quickAskGemini25MaxBudget > 16384
			? [
					{
						value: String(quickAskGemini25MaxBudget),
						label: `Max (${quickAskGemini25MaxBudget})`,
					},
				]
			: []),
	];

	// Anthropic "extended thinking" is controlled via a numeric token budget.
	// We present it as a simple level selector and map levels -> budgets.
	const ANTHROPIC_THINKING_LEVEL_BUDGETS = [2000, 4000, 8000, 32000] as const;

	const anthropicThinkingLevelOptions: Array<{ value: string; label: string }> =
		[
			{ value: SELECT_DEFAULT, label: "Default" },
			// Allow profiles to explicitly turn off thinking even if Default enables it.
			...(!isDefaultScope ? [{ value: "0", label: "Off" }] : []),
			{ value: String(ANTHROPIC_THINKING_LEVEL_BUDGETS[0]), label: "Low" },
			{ value: String(ANTHROPIC_THINKING_LEVEL_BUDGETS[1]), label: "Medium" },
			{ value: String(ANTHROPIC_THINKING_LEVEL_BUDGETS[2]), label: "High" },
			{ value: String(ANTHROPIC_THINKING_LEVEL_BUDGETS[3]), label: "Max" },
		];

	const anthropicThinkingLevelOptionsWithCustom = (() => {
		const vRaw = isDefaultScope
			? settings?.anthropic_thinking_budget
			: localProfileAnthropicThinkingBudget === SELECT_DEFAULT
				? null
				: Number(localProfileAnthropicThinkingBudget);
		const v =
			typeof vRaw === "number" && Number.isFinite(vRaw)
				? Math.trunc(vRaw)
				: null;
		if (v == null) return anthropicThinkingLevelOptions;

		const asString = String(v);
		const exists = anthropicThinkingLevelOptions.some(
			(o) => o.value === asString,
		);
		if (exists) return anthropicThinkingLevelOptions;

		return [
			...anthropicThinkingLevelOptions,
			{ value: asString, label: `Custom (${v})` },
		];
	})();

	const quickAskAnthropicThinkingLevelOptionsWithCustom = (() => {
		const vRaw = isDefaultScope
			? settings?.quick_ask_anthropic_thinking_budget
			: localProfileQuickAskAnthropicThinkingBudget === SELECT_DEFAULT
				? null
				: Number(localProfileQuickAskAnthropicThinkingBudget);
		const v =
			typeof vRaw === "number" && Number.isFinite(vRaw)
				? Math.trunc(vRaw)
				: null;
		if (v == null) return anthropicThinkingLevelOptions;

		const asString = String(v);
		const exists = anthropicThinkingLevelOptions.some(
			(o) => o.value === asString,
		);
		if (exists) return anthropicThinkingLevelOptions;

		return [
			...anthropicThinkingLevelOptions,
			{ value: asString, label: `Custom (${v})` },
		];
	})();

	const formatThinkingBudgetShort = (budgetTokens: number): string => {
		if (!Number.isFinite(budgetTokens) || budgetTokens <= 0)
			return String(budgetTokens);
		if (budgetTokens >= 1000) {
			const k = budgetTokens / 1000;
			const pretty = Number.isInteger(k)
				? String(k)
				: k.toFixed(1).replace(/\.0$/, "");
			return `${pretty}k`;
		}
		return String(budgetTokens);
	};

	const handleOpenAiThinkingChange = (value: string | null) => {
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
	};

	const handleGeminiThinkingLevelChange = (value: string | null) => {
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
	};

	const handleGeminiThinkingBudgetChange = (value: string | null) => {
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
	};

	const handleAnthropicThinkingBudgetChange = (value: string | null) => {
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
	};

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

	const buildSections = (overrides?: {
		key: SectionKey;
		content?: string | null;
	}): CleanupPromptSections => {
		if (localSections === null) {
			return DEFAULT_SECTIONS;
		}

		const getContent = (key: SectionKey): string | null => {
			const content =
				overrides?.key === key && overrides.content !== undefined
					? overrides.content
					: localSections[key].content;

			// Return null if content matches default (to use server default)
			if (content === defaultSections?.[key]) {
				return null;
			}
			return content || null;
		};

		return {
			system: { content: getContent("system") },
		};
	};

	const saveAllSections = (sections: CleanupPromptSections) => {
		// Only used for Default scope. Per-profile prompt changes are stored as per-section overrides.
		updateCleanupPromptSections.mutate(sections, {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
			},
		});
	};

	const normalizePromptOverrides = (
		overrides: CleanupPromptSectionsOverride,
	): CleanupPromptSectionsOverride | null => {
		const hasAny = overrides.system != null;
		return hasAny ? overrides : null;
	};

	const saveProfileSectionOverride = (
		key: SectionKey,
		section: CleanupPromptSections[SectionKey] | null,
	) => {
		const current: CleanupPromptSectionsOverride =
			profilePromptOverridesRef.current ?? {};
		const next: CleanupPromptSectionsOverride = { ...current, [key]: section };
		const normalized = normalizePromptOverrides(next);
		profilePromptOverridesRef.current = normalized;

		saveProfileMetadata({ cleanup_prompt_sections: normalized });
	};

	const buildSectionOverride = (
		key: SectionKey,
		overrides?: {
			content?: string | null;
		},
	): CleanupPromptSections[SectionKey] => {
		if (localSections === null) {
			return DEFAULT_SECTIONS[key];
		}

		const contentRaw =
			overrides?.content !== undefined
				? overrides.content
				: localSections[key].content;

		const contentToStore =
			contentRaw === defaultSections?.[key] ? null : contentRaw || null;

		return {
			content: contentToStore,
		};
	};

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
			};

			const updated = [...profiles, { ...defaultProfile, ...next }];
			updateRewriteProgramPromptProfiles.mutate(updated, {
				onSuccess: () => {
					tauriAPI.emitSettingsChanged();
				},
			});
			return;
		}

		const updated = profiles.map((p) =>
			p.id === activeProfileId ? { ...p, ...next } : p,
		);

		updateRewriteProgramPromptProfiles.mutate(updated, {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
			},
		});
	};

	const handleSave = (key: SectionKey, content: string) => {
		setLocalSections((prev) => {
			if (prev === null) return prev;
			return { ...prev, [key]: { ...prev[key], content } };
		});

		if (activeProfileId === "default") {
			saveAllSections(buildSections({ key, content }));
			return;
		}

		saveProfileSectionOverride(key, buildSectionOverride(key, { content }));
	};

	const handleReset = (key: SectionKey) => {
		const defaultContent = defaultSections?.[key] ?? "";
		setLocalSections((prev) => {
			if (prev === null) return prev;
			return { ...prev, [key]: { ...prev[key], content: defaultContent } };
		});

		if (activeProfileId === "default") {
			saveAllSections(buildSections({ key, content: null }));
			return;
		}

		saveProfileSectionOverride(
			key,
			buildSectionOverride(key, { content: null }),
		);
	};

	const handleDefaultSTTProviderChange = (value: string | null) => {
		if (!value) return;
		updateSTTProvider.mutate(value, {
			onSuccess: () => {
				const models = STT_MODELS[value];
				const firstModel = models?.[0];
				if (firstModel) {
					updateSTTModel.mutate(firstModel.value);
				}
				tauriAPI.emitSettingsChanged();
			},
		});
	};

	const handleDefaultSTTModelChange = (value: string | null) => {
		if (!value) return;
		updateSTTModel.mutate(value, {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
			},
		});
	};

	const handleDefaultLLMProviderChange = (value: string | null) => {
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
	};

	const handleDefaultLLMModelChange = (value: string | null) => {
		if (!value) return;
		updateLLMModel.mutate(value, {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
			},
		});
	};

	const handleDefaultQuickAskProviderChange = (value: string | null) => {
		if (!value) return;
		updateQuickAskProvider.mutate(value, {
			onSuccess: () => {
				const models = getLlmModelOptionsForProvider(value);
				const firstModel = models?.[0];
				if (firstModel) {
					updateQuickAskModel.mutate(firstModel.value);
				}
				tauriAPI.emitSettingsChanged();
			},
		});
	};

	const handleDefaultQuickAskModelChange = (value: string | null) => {
		if (!value) return;
		updateQuickAskModel.mutate(value, {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
			},
		});
	};

	const handleDefaultSTTTimeoutChange = (value: number) => {
		updateSTTTimeout.mutate(value, {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
			},
		});
	};

	const handleWhisperServerModelDraftBlur = () => {
		const trimmed = whisperServerModelDraft.trim();
		const toStore = trimmed.length > 0 ? trimmed : null;

		if (isDefaultScope) {
			const stored = settings?.stt_model?.trim() || null;
			if (toStore === stored) return;
			updateSTTModel.mutate(toStore, {
				onSuccess: () => {
					tauriAPI.emitSettingsChanged();
				},
			});
			return;
		}

		setSttModelInheriting(false);
		setLocalProfileSttModel(toStore);
		saveProfileMetadata({ stt_model: toStore });
	};

	const handleSttProviderChange = (value: string | null) => {
		if (!value) return;
		if (isDefaultScope) {
			handleDefaultSTTProviderChange(value);
			return;
		}

		setSttProviderInheriting(false);
		setSttModelInheriting(false);
		setLocalProfileSttProvider(value);
		const models = STT_MODELS[value] ?? [];
		const firstModel = models[0]?.value ?? null;
		setLocalProfileSttModel(firstModel);
		saveProfileMetadata({
			stt_provider: value,
			stt_model: firstModel,
		});
	};

	const handleSttModelChange = (value: string | null) => {
		if (!value) return;
		if (isDefaultScope) {
			handleDefaultSTTModelChange(value);
			return;
		}

		setSttModelInheriting(false);
		setLocalProfileSttModel(value);
		saveProfileMetadata({ stt_model: value });
	};

	const handleSttTimeoutChange = (value: number | string) => {
		// Keep local state permissive so typing feels natural (e.g., allow clearing the field
		// or temporarily typing an out-of-range intermediate value like "1" before "10").
		setLocalProfileSttTimeout(value);

		// Only persist when the value is a valid in-range number.
		if (typeof value !== "number" || Number.isNaN(value)) return;
		if (value < 5 || value > 120) return;

		if (isDefaultScope) {
			handleDefaultSTTTimeoutChange(value);
			return;
		}

		setSttTimeoutInheriting(false);
		saveProfileMetadata({ stt_timeout_seconds: value });
	};

	const handleSttTimeoutBlur = () => {
		// On blur, clamp and normalize.
		// If the user cleared the input, revert to the effective value without saving.
		if (localProfileSttTimeout === "") {
			const fallback = isDefaultScope
				? (settings?.stt_timeout_seconds ?? DEFAULT_STT_TIMEOUT)
				: (activeProfile?.stt_timeout_seconds ??
					settings?.stt_timeout_seconds ??
					DEFAULT_STT_TIMEOUT);
			setLocalProfileSttTimeout(fallback);
			return;
		}

		if (
			typeof localProfileSttTimeout !== "number" ||
			Number.isNaN(localProfileSttTimeout)
		) {
			return;
		}

		const clamped = Math.max(5, Math.min(120, localProfileSttTimeout));
		if (clamped !== localProfileSttTimeout) {
			setLocalProfileSttTimeout(clamped);
		}

		if (isDefaultScope) {
			handleDefaultSTTTimeoutChange(clamped);
			return;
		}

		setSttTimeoutInheriting(false);
		saveProfileMetadata({ stt_timeout_seconds: clamped });
	};

	const handleDisableSttProviderOverride = () => {
		openDisableOverrideDialog({
			title: "Disable Speech-to-Text Provider override?",
			onConfirm: () => {
				setSttProviderInheriting(true);
				setSttModelInheriting(true);
				setLocalProfileSttProvider(settings?.stt_provider ?? null);
				setLocalProfileSttModel(settings?.stt_model ?? null);
				saveProfileMetadata({
					stt_provider: null,
					stt_model: null,
				});
			},
		});
	};

	const handleDisableSttModelOverride = () => {
		openDisableOverrideDialog({
			title: "Disable STT Model override?",
			onConfirm: () => {
				setSttModelInheriting(true);
				setLocalProfileSttModel(settings?.stt_model ?? null);
				saveProfileMetadata({ stt_model: null });
			},
		});
	};

	const handleDisableSttTimeoutOverride = () => {
		openDisableOverrideDialog({
			title: "Disable STT Timeout override?",
			onConfirm: () => {
				setSttTimeoutInheriting(true);
				setLocalProfileSttTimeout(
					settings?.stt_timeout_seconds ?? DEFAULT_STT_TIMEOUT,
				);
				saveProfileMetadata({ stt_timeout_seconds: null });
			},
		});
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
				message: `Cached ${res.cached_now} / ${res.total_hints} hints (${res.skipped_existing} already cached) Â· ${res.provider} / ${res.model}`,
				color: "gray",
			});
		} catch (e) {
			notifications.show({
				title: "Failed to store embeddings",
				message: errorToMessage(e),
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

	const handleOpenPresetPromptLab = (
		preset: RewritePreset,
		key: SectionKey,
		initialContent: string,
	) => {
		const presetLabel = preset.name?.trim() || preset.id;
		setPromptLabContextPrompt(initialContent.trim());
		setPromptLabContextLabel(`${activeProfileLabel} Â· ${presetLabel}`);
		setPromptLabApplyTarget({ type: "preset", presetId: preset.id, key });
		setPromptLabOpen(true);
	};

	const handleOpenDefaultPromptLab = () => {
		setPromptLabContextPrompt(effectiveCurrentPrompt);
		setPromptLabContextLabel(activeProfileLabel);
		setPromptLabApplyTarget({ type: "profile", key: "system" });
		setPromptLabOpen(true);
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
				onClose={() => {
					setPromptLabOpen(false);
					setPromptLabApplyTarget(null);
				}}
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

			<TranscribeSettingsSection
				activeProfileId={activeProfileId}
				isDefaultScope={isDefaultScope}
				inheritTooltip={INHERIT_TOOLTIP}
				sttProviderInheriting={sttProviderInheriting}
				sttModelInheriting={sttModelInheriting}
				sttTimeoutInheriting={sttTimeoutInheriting}
				effectiveSttProvider={effectiveSttProvider}
				sttProviderOptions={sttProviderOptions}
				isSttProviderOptionsDisabled={
					sttCloudProviders.length === 0 && sttLocalProviders.length === 0
				}
				sttProviderIsWhisperServer={sttProviderIsWhisperServer}
				sttModelOptions={sttModelOptions}
				selectedSttModelForUi={selectedSttModelForUi}
				sttPricingLabel={sttPricingLabel}
				whisperServerModelDraft={whisperServerModelDraft}
				onWhisperServerModelDraftChange={setWhisperServerModelDraft}
				onWhisperServerModelBlur={handleWhisperServerModelDraftBlur}
				onSttProviderChange={handleSttProviderChange}
				onSttModelChange={handleSttModelChange}
				onDisableSttProviderOverride={handleDisableSttProviderOverride}
				onDisableSttModelOverride={handleDisableSttModelOverride}
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
			/>

			<div className="settings-mini-header">
				<span className="settings-mini-header__text">Rewrite</span>
			</div>

			<div className="settings-row">
				<div>
					<p className="settings-label">Rewrite Transcription</p>
					<p className="settings-description">
						Enable or disable rewriting the transcription with an LLM
					</p>
				</div>
				<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
					{!isDefaultScope && rewriteEnabledInheriting && (
						<Tooltip label={INHERIT_TOOLTIP} withArrow>
							<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
						</Tooltip>
					)}
					{!isDefaultScope && !rewriteEnabledInheriting && (
						<Tooltip label="Disable override (inherit from Default)" withArrow>
							<ActionIcon
								variant="subtle"
								color="gray"
								size="sm"
								onClick={() =>
									openDisableOverrideDialog({
										title: "Disable Rewrite Transcription override?",
										onConfirm: () => {
											setRewriteEnabledInheriting(true);
											setLocalProfileRewriteEnabled(defaultRewriteEnabled);
											saveProfileMetadata({ rewrite_llm_enabled: null });
										},
									})
								}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</ActionIcon>
						</Tooltip>
					)}
					<Switch
						checked={
							isDefaultScope
								? defaultRewriteEnabled
								: localProfileRewriteEnabled
						}
						onChange={(e) => {
							const enabled = e.currentTarget.checked;
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
						}}
						color="gray"
						size="md"
					/>
				</div>
			</div>

			<div className="settings-row">
				<div>
					<p className="settings-label">Include Clipboard Context</p>
					<p className="settings-description">
						When enabled, Kolboo reads your clipboard text and includes it as
						optional context during the Rewrite step.
					</p>
				</div>
				<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
					{!isDefaultScope && rewriteIncludeClipboardContextInheriting && (
						<Tooltip label={INHERIT_TOOLTIP} withArrow>
							<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
						</Tooltip>
					)}
					{!isDefaultScope && !rewriteIncludeClipboardContextInheriting && (
						<Tooltip label="Disable override (inherit from Default)" withArrow>
							<ActionIcon
								variant="subtle"
								color="gray"
								size="sm"
								onClick={() =>
									openDisableOverrideDialog({
										title: "Disable Rewrite Clipboard Context override?",
										onConfirm: () => {
											setRewriteIncludeClipboardContextInheriting(true);
											setLocalProfileRewriteIncludeClipboardContext(
												defaultRewriteIncludeClipboardContext,
											);
											saveProfileMetadata({
												rewrite_include_clipboard_context: null,
											});
										},
									})
								}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</ActionIcon>
						</Tooltip>
					)}
					<Switch
						checked={localProfileRewriteIncludeClipboardContext}
						onChange={(e) => {
							const enabled = e.currentTarget.checked;
							if (!isDefaultScope) {
								setRewriteIncludeClipboardContextInheriting(false);
							}
							setLocalProfileRewriteIncludeClipboardContext(enabled);
							saveProfileMetadata({
								rewrite_include_clipboard_context: enabled,
							});
						}}
						color="gray"
						size="md"
					/>
				</div>
			</div>

			<div className="settings-row">
				<div>
					<p className="settings-label">Language Model Provider</p>
					<p className="settings-description">AI service for text formatting</p>
				</div>
				<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
					{!isDefaultScope && llmProviderInheriting && (
						<Tooltip label={INHERIT_TOOLTIP} withArrow>
							<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
						</Tooltip>
					)}
					{!isDefaultScope && !llmProviderInheriting && (
						<Tooltip label="Disable override (inherit from Default)" withArrow>
							<ActionIcon
								variant="subtle"
								color="gray"
								size="sm"
								onClick={() =>
									openDisableOverrideDialog({
										title: "Disable Language Model Provider override?",
										onConfirm: () => {
											setLlmProviderInheriting(true);
											setLlmModelInheriting(true);
											setLocalProfileLlmProvider(
												settings?.llm_provider ?? null,
											);
											setLocalProfileLlmModel(settings?.llm_model ?? null);
											saveProfileMetadata({
												llm_provider: null,
												llm_model: null,
											});
										},
									})
								}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</ActionIcon>
						</Tooltip>
					)}
					<Select
						data={llmProviderOptions}
						value={effectiveLlmProvider}
						onChange={(value) => {
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
						}}
						placeholder="Select provider"
						withCheckIcon={false}
						disabled={
							llmCloudProviders.length === 0 && llmLocalProviders.length === 0
						}
						styles={{
							input: {
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							},
						}}
					/>
				</div>
			</div>

			{llmModelOptions.length > 0 ? (
				<div className="settings-row">
					<div>
						<p className="settings-label">Rewrite LLM Model</p>
						<p className="settings-description">
							LLM Model used to rewrite the transcription.
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && llmModelInheriting && (
							<Tooltip label={INHERIT_TOOLTIP} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}
						{!isDefaultScope && !llmModelInheriting && (
							<Tooltip
								label="Disable override (inherit from Default)"
								withArrow
							>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={() =>
										openDisableOverrideDialog({
											title: "Disable Rewrite LLM Model override?",
											onConfirm: () => {
												setLlmModelInheriting(true);
												setLocalProfileLlmModel(settings?.llm_model ?? null);
												saveProfileMetadata({ llm_model: null });
											},
										})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}
						{llmPricingLabel ? (
							<Text
								size="xs"
								c="dimmed"
								style={{ whiteSpace: "nowrap", lineHeight: 1 }}
							>
								{llmPricingLabel}
							</Text>
						) : null}
						<Select
							data={llmModelOptions}
							value={
								isDefaultScope
									? (settings?.llm_model ?? llmModelOptions[0]?.value ?? null)
									: localProfileLlmModel
							}
							onChange={(value) => {
								if (!value) return;
								if (isDefaultScope) {
									handleDefaultLLMModelChange(value);
									return;
								}

								setLlmModelInheriting(false);
								setLocalProfileLlmModel(value);
								saveProfileMetadata({ llm_model: value });
							}}
							placeholder="Select model"
							withCheckIcon={false}
							styles={{
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
									minWidth: 200,
								},
							}}
						/>
					</div>
				</div>
			) : null}

			{supportsOpenAiThinking && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Thinking</p>
						<p className="settings-description">
							Set the reasoning effort for this model.
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && openAiReasoningEffortInheriting && (
							<Tooltip label={INHERIT_TOOLTIP} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}
						{!isDefaultScope && !openAiReasoningEffortInheriting && (
							<Tooltip
								label="Disable override (inherit from Default)"
								withArrow
							>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={() =>
										openDisableOverrideDialog({
											title: "Disable Thinking override?",
											onConfirm: () => {
												setOpenAiReasoningEffortInheriting(true);
												setLocalProfileOpenAiReasoningEffort(SELECT_DEFAULT);
												saveProfileMetadata({
													openai_reasoning_effort: null,
												});
											},
										})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}

						<HintSelect
							data={openAiThinkingOptions}
							value={
								isDefaultScope
									? (settings?.openai_reasoning_effort ?? SELECT_DEFAULT)
									: localProfileOpenAiReasoningEffort
							}
							onChange={(value) => {
								if (isDefaultScope) {
									handleOpenAiThinkingChange(value);
									return;
								}

								if (value == null || value === SELECT_DEFAULT) {
									setOpenAiReasoningEffortInheriting(true);
									setLocalProfileOpenAiReasoningEffort(SELECT_DEFAULT);
									saveProfileMetadata({ openai_reasoning_effort: null });
									return;
								}

								setOpenAiReasoningEffortInheriting(false);
								setLocalProfileOpenAiReasoningEffort(value);
								const effort = isOpenAiReasoningEffort(value) ? value : null;
								if (!effort) return;
								saveProfileMetadata({
									openai_reasoning_effort: effort,
								});
							}}
							placeholder="Default"
							inputStyle={{
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							}}
							renderSelected={({ option, placeholder }) => {
								if (!option) {
									return (
										<Text size="sm" c="dimmed">
											{placeholder}
										</Text>
									);
								}

								if (option.value !== SELECT_DEFAULT) {
									return <Text size="sm">{option.label}</Text>;
								}

								const hint = isDefaultScope
									? effectiveLlmModel
										? openAiDefaultReasoningEffortForModel(effectiveLlmModel)
										: "medium"
									: (settings?.openai_reasoning_effort ??
										(effectiveLlmModel
											? openAiDefaultReasoningEffortForModel(effectiveLlmModel)
											: "medium"));

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<span style={{ fontSize: 14 }}>{option.label}</span>
										<span
											style={{
												fontSize: 11,
												color: "var(--text-muted)",
												opacity: 0.9,
												lineHeight: 1,
											}}
										>
											Â· {hint}
										</span>
									</div>
								);
							}}
							renderOption={({ option }) => {
								if (option.value !== SELECT_DEFAULT) {
									return <Text size="sm">{option.label}</Text>;
								}

								const hint = isDefaultScope
									? effectiveLlmModel
										? openAiDefaultReasoningEffortForModel(effectiveLlmModel)
										: "medium"
									: (settings?.openai_reasoning_effort ??
										(effectiveLlmModel
											? openAiDefaultReasoningEffortForModel(effectiveLlmModel)
											: "medium"));

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<span style={{ fontSize: 14 }}>{option.label}</span>
										<span
											style={{
												fontSize: 11,
												color: "var(--text-muted)",
												opacity: 0.9,
												lineHeight: 1,
											}}
										>
											Â· {hint}
										</span>
									</div>
								);
							}}
						/>
					</div>
				</div>
			)}

			{supportsGeminiThinkingLevel && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Thinking Level</p>
						<p className="settings-description">
							{isGemini3Pro
								? "Gemini 3 Pro supports low/high (default high)."
								: "Gemini 3 Flash supports minimal/low/medium/high (default high)."}
						</p>
					</div>

					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && geminiThinkingLevelInheriting && (
							<Tooltip label={INHERIT_TOOLTIP} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}

						{!isDefaultScope && !geminiThinkingLevelInheriting && (
							<Tooltip
								label="Disable override (inherit from Default)"
								withArrow
							>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={() =>
										openDisableOverrideDialog({
											title: "Disable Thinking Level override?",
											onConfirm: () => {
												setGeminiThinkingLevelInheriting(true);
												setLocalProfileGeminiThinkingLevel(SELECT_DEFAULT);
												saveProfileMetadata({ gemini_thinking_level: null });
											},
										})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}

						<HintSelect
							data={geminiThinkingLevelOptions}
							value={
								isDefaultScope
									? (settings?.gemini_thinking_level ?? SELECT_DEFAULT)
									: localProfileGeminiThinkingLevel
							}
							onChange={(value) => {
								if (isDefaultScope) {
									handleGeminiThinkingLevelChange(value);
									return;
								}

								if (value == null || value === SELECT_DEFAULT) {
									setGeminiThinkingLevelInheriting(true);
									setLocalProfileGeminiThinkingLevel(SELECT_DEFAULT);
									saveProfileMetadata({ gemini_thinking_level: null });
									return;
								}

								const v =
									value === "minimal" ||
									value === "low" ||
									value === "medium" ||
									value === "high"
										? value
										: null;
								if (v == null) return;

								setGeminiThinkingLevelInheriting(false);
								setLocalProfileGeminiThinkingLevel(v);
								saveProfileMetadata({ gemini_thinking_level: v });
							}}
							placeholder="Default"
							inputStyle={{
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							}}
							renderSelected={({ option, placeholder }) => {
								if (!option) {
									return (
										<Text size="sm" c="dimmed">
											{placeholder}
										</Text>
									);
								}
								if (option.value !== SELECT_DEFAULT) {
									return <Text size="sm">{option.label}</Text>;
								}

								const hint = isDefaultScope
									? "high"
									: (settings?.gemini_thinking_level ?? "high");

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<span style={{ fontSize: 14 }}>{option.label}</span>
										<span
											style={{
												fontSize: 11,
												color: "var(--text-muted)",
												opacity: 0.9,
												lineHeight: 1,
											}}
										>
											Â· {hint}
										</span>
									</div>
								);
							}}
							renderOption={({ option }) => {
								if (option.value !== SELECT_DEFAULT) {
									return <Text size="sm">{option.label}</Text>;
								}

								const hint = isDefaultScope
									? "high"
									: (settings?.gemini_thinking_level ?? "high");

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<span style={{ fontSize: 14 }}>{option.label}</span>
										<span
											style={{
												fontSize: 11,
												color: "var(--text-muted)",
												opacity: 0.9,
												lineHeight: 1,
											}}
										>
											Â· {hint}
										</span>
									</div>
								);
							}}
						/>
					</div>
				</div>
			)}

			{supportsGeminiThinkingBudget && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Thinking Budget</p>
						<p className="settings-description">
							Token budget for Gemini 2.5 thinking.
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && geminiThinkingBudgetInheriting && (
							<Tooltip label={INHERIT_TOOLTIP} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}
						{!isDefaultScope && !geminiThinkingBudgetInheriting && (
							<Tooltip
								label="Disable override (inherit from Default)"
								withArrow
							>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={() =>
										openDisableOverrideDialog({
											title: "Disable Thinking Budget override?",
											onConfirm: () => {
												setGeminiThinkingBudgetInheriting(true);
												setLocalProfileGeminiThinkingBudget(SELECT_DEFAULT);
												saveProfileMetadata({ gemini_thinking_budget: null });
											},
										})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}
						<HintSelect
							data={geminiThinkingBudgetOptions}
							value={
								isDefaultScope
									? settings?.gemini_thinking_budget == null
										? SELECT_DEFAULT
										: String(settings.gemini_thinking_budget)
									: localProfileGeminiThinkingBudget
							}
							onChange={(value) => {
								if (isDefaultScope) {
									handleGeminiThinkingBudgetChange(value);
									return;
								}

								if (value == null || value === SELECT_DEFAULT) {
									setGeminiThinkingBudgetInheriting(true);
									setLocalProfileGeminiThinkingBudget(SELECT_DEFAULT);
									saveProfileMetadata({ gemini_thinking_budget: null });
									return;
								}

								const parsed = Number(value);
								if (!Number.isFinite(parsed)) return;
								const asInt = Math.trunc(parsed);
								setGeminiThinkingBudgetInheriting(false);
								setLocalProfileGeminiThinkingBudget(String(asInt));
								saveProfileMetadata({ gemini_thinking_budget: asInt });
							}}
							placeholder="Default"
							inputStyle={{
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							}}
							renderSelected={({ option, placeholder }) => {
								if (!option) {
									return (
										<Text size="sm" c="dimmed">
											{placeholder}
										</Text>
									);
								}
								if (option.value !== SELECT_DEFAULT)
									return <Text size="sm">{option.label}</Text>;

								const inherited = settings?.gemini_thinking_budget;
								const hint = isDefaultScope
									? "dynamic"
									: inherited == null
										? "dynamic"
										: inherited === 0
											? "off"
											: inherited === -1
												? "dynamic"
												: String(inherited);

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<span style={{ fontSize: 14 }}>{option.label}</span>
										<span
											style={{
												fontSize: 11,
												color: "var(--text-muted)",
												opacity: 0.9,
												lineHeight: 1,
											}}
										>
											Â· {hint}
										</span>
									</div>
								);
							}}
							renderOption={({ option }) => {
								if (option.value !== SELECT_DEFAULT) {
									return <Text size="sm">{option.label}</Text>;
								}

								const inherited = settings?.gemini_thinking_budget;
								const hint = isDefaultScope
									? "dynamic"
									: inherited == null
										? "dynamic"
										: inherited === 0
											? "off"
											: inherited === -1
												? "dynamic"
												: String(inherited);

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<span style={{ fontSize: 14 }}>{option.label}</span>
										<span
											style={{
												fontSize: 11,
												color: "var(--text-muted)",
												opacity: 0.9,
												lineHeight: 1,
											}}
										>
											Â· {hint}
										</span>
									</div>
								);
							}}
						/>
					</div>
				</div>
			)}

			{supportsAnthropicThinkingBudget && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Thinking</p>
						<p className="settings-description">
							Extended thinking level for Claude models.
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && anthropicThinkingBudgetInheriting && (
							<Tooltip label={INHERIT_TOOLTIP} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}
						{!isDefaultScope && !anthropicThinkingBudgetInheriting && (
							<Tooltip
								label="Disable override (inherit from Default)"
								withArrow
							>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={() =>
										openDisableOverrideDialog({
											title: "Disable Thinking override?",
											onConfirm: () => {
												setAnthropicThinkingBudgetInheriting(true);
												setLocalProfileAnthropicThinkingBudget(SELECT_DEFAULT);
												saveProfileMetadata({
													anthropic_thinking_budget: null,
												});
											},
										})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}
						<HintSelect
							data={anthropicThinkingLevelOptionsWithCustom}
							value={
								isDefaultScope
									? settings?.anthropic_thinking_budget == null
										? SELECT_DEFAULT
										: String(settings.anthropic_thinking_budget)
									: localProfileAnthropicThinkingBudget
							}
							onChange={(value) => {
								if (isDefaultScope) {
									handleAnthropicThinkingBudgetChange(value);
									return;
								}

								if (value == null || value === SELECT_DEFAULT) {
									setAnthropicThinkingBudgetInheriting(true);
									setLocalProfileAnthropicThinkingBudget(SELECT_DEFAULT);
									saveProfileMetadata({ anthropic_thinking_budget: null });
									return;
								}

								const parsed = Number(value);
								if (!Number.isFinite(parsed)) return;
								const asInt = Math.trunc(parsed);
								setAnthropicThinkingBudgetInheriting(false);
								setLocalProfileAnthropicThinkingBudget(String(asInt));
								saveProfileMetadata({ anthropic_thinking_budget: asInt });
							}}
							placeholder="Default"
							inputStyle={{
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							}}
							renderSelected={({ option, placeholder }) => {
								if (!option) {
									return (
										<Text size="sm" c="dimmed">
											{placeholder}
										</Text>
									);
								}

								if (option.value === SELECT_DEFAULT) {
									const inheritedBudget = settings?.anthropic_thinking_budget;
									const hint = isDefaultScope
										? "off"
										: inheritedBudget == null
											? "off"
											: formatThinkingBudgetShort(inheritedBudget);

									return (
										<div
											style={{
												display: "flex",
												alignItems: "baseline",
												gap: 8,
											}}
										>
											<span style={{ fontSize: 14 }}>{option.label}</span>
											<span
												style={{
													fontSize: 11,
													color: "var(--text-muted)",
													opacity: 0.9,
													lineHeight: 1,
												}}
											>
												Â· {hint}
											</span>
										</div>
									);
								}

								// Closed state: keep it simple (label only) unless it's a custom token budget.
								if (option.label.startsWith("Custom")) {
									const n = Number(option.value);
									const suffix = Number.isFinite(n)
										? formatThinkingBudgetShort(n)
										: null;
									return (
										<div
											style={{
												display: "flex",
												alignItems: "baseline",
												gap: 8,
											}}
										>
											<Text size="sm">{option.label}</Text>
											{suffix && (
												<Text size="xs" c="dimmed" style={{ lineHeight: 1 }}>
													{suffix}
												</Text>
											)}
										</div>
									);
								}

								return <Text size="sm">{option.label}</Text>;
							}}
							renderOption={({ option }) => {
								if (option.value === SELECT_DEFAULT) {
									const inheritedBudget = settings?.anthropic_thinking_budget;
									const hint = isDefaultScope
										? "off"
										: inheritedBudget == null
											? "off"
											: formatThinkingBudgetShort(inheritedBudget);

									return (
										<div
											style={{
												display: "flex",
												alignItems: "baseline",
												gap: 8,
											}}
										>
											<span style={{ fontSize: 14 }}>{option.label}</span>
											<span
												style={{
													fontSize: 11,
													color: "var(--text-muted)",
													opacity: 0.9,
													lineHeight: 1,
												}}
											>
												Â· {hint}
											</span>
										</div>
									);
								}

								const n = Number(option.value);
								const suffix = Number.isFinite(n)
									? formatThinkingBudgetShort(n)
									: null;

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<Text size="sm">{option.label}</Text>
										{suffix && (
											<Text size="xs" c="dimmed" style={{ lineHeight: 1 }}>
												{suffix}
											</Text>
										)}
									</div>
								);
							}}
						/>
					</div>
				</div>
			)}

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
										isSavingProfiles={
											updateRewriteProgramPromptProfiles.isPending
										}
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
							normalizeRouter={normalizeRouter}
							saveRouter={saveRouter}
							openAiThinkingEffortsForModel={openAiThinkingEffortsForModel}
							openAiDefaultReasoningEffortForModel={
								openAiDefaultReasoningEffortForModel
							}
							isOpenAiReasoningEffort={isOpenAiReasoningEffort}
							formatThinkingBudgetShort={formatThinkingBudgetShort}
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
				defaultQuickReplaceEnabled={defaultQuickReplaceEnabled}
				defaultQuickReplaceIncludeClipboardContext={
					defaultQuickReplaceIncludeClipboardContext
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
				effectiveQuickAskProvider={effectiveQuickAskProvider}
				effectiveQuickAskModel={effectiveQuickAskModel}
				quickAskIncludeSelectedText={quickAskIncludeSelectedText}
				quickAskConversationHistoryEnabled={quickAskConversationHistoryEnabled}
				quickAskConversationHistoryCount={quickAskConversationHistoryCount}
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
				defaultQuickAskIncludeClipboardContext={
					defaultQuickAskIncludeClipboardContext
				}
				localProfileQuickAskIncludeClipboardContext={
					localProfileQuickAskIncludeClipboardContext
				}
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
				setLocalProfileQuickAskIncludeClipboardContext={
					setLocalProfileQuickAskIncludeClipboardContext
				}
				setLocalProfileQuickAskProvider={setLocalProfileQuickAskProvider}
				setLocalProfileQuickAskModel={setLocalProfileQuickAskModel}
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
				errorToMessage={errorToMessage}
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
