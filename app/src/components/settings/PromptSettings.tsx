import {
  Accordion,
  ActionIcon,
  Button,
  Divider,
  Group,
  Loader,
  Modal,
  NumberInput,
  Select,
  Switch,
  Textarea,
  Text,
  TextInput,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { Info, RotateCcw, Trash2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  useDefaultSections,
  useSettings,
  useAvailableProviders,
  useTestLlmRewrite,
  useIterateRewritePrompt,
  useTestRewriteWithPrompt,
  useTestSttTranscribeLastAudio,
  useHasLastAudioForSttTest,
  useUpdateCleanupPromptSections,
  useUpdateLLMModel,
  useUpdateLLMProvider,
  useUpdateOpenAiReasoningEffort,
  useUpdateAnthropicThinkingBudget,
  useUpdateRewriteProgramPromptProfiles,
  useUpdateRewriteLlmEnabled,
  useUpdateSTTModel,
  useUpdateSTTProvider,
  useUpdateSTTTranscriptionPrompt,
  useUpdateSTTTimeout,
  useUpdateGeminiThinkingBudget,
  useUpdateGeminiThinkingLevel,
  useModelPricing,
  useUpdateQuickAskProvider,
  useUpdateQuickAskModel,
  useUpdateQuickAskSystemPrompt,
  useUpdateQuickAskIncludeSelectedText,
  useUpdateQuickAskConversationHistoryEnabled,
  useUpdateQuickAskConversationHistoryCount,
  useUpdateQuickAskOpenAiReasoningEffort,
  useUpdateQuickAskAnthropicThinkingBudget,
  useUpdateQuickAskGeminiThinkingBudget,
  useUpdateQuickAskGeminiThinkingLevel,
  useFireworksModels,
  useOllamaModels,
} from "../../lib/queries";
import {
  type CleanupPromptSections,
  type CleanupPromptSectionsOverride,
  type IntentRouterSettings,
  type OpenAiReasoningEffort,
  type RewritePreset,
  type RewriteProgramPromptProfile,
  llmAPI,
  tauriAPI,
} from "../../lib/tauri";
import {
  EMBEDDING_MODELS,
  LLM_MODELS,
  STT_MODELS,
} from "../../lib/modelOptions";
import { HintSelect } from "../HintSelect";
import { PromptSectionEditor } from "./PromptSectionEditor";
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
  if (err && typeof err === "object") {
    const anyErr = err as any;
    if (typeof anyErr.message === "string") return anyErr.message;
    if (typeof anyErr.error === "string") return anyErr.error;
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
  const updateQuickAskIncludeSelectedText = useUpdateQuickAskIncludeSelectedText();
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

  const [localProfileSttProvider, setLocalProfileSttProvider] = useState<
    string | null
  >(null);
  const [localProfileSttModel, setLocalProfileSttModel] = useState<
    string | null
  >(null);
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
  >(DEFAULT_STT_TIMEOUT);

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

  const [rewriteTestInput, setRewriteTestInput] = useState<string>("");
  const [rewriteTestOutput, setRewriteTestOutput] = useState<string>("");
  const [rewriteTestError, setRewriteTestError] = useState<string>("");
  const [rewriteTestDurationMs, setRewriteTestDurationMs] = useState<
    number | null
  >(null);
  const rewriteTestStartRef = useRef<number | null>(null);

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

  const getPresetsForProfile = (
    p: RewriteProgramPromptProfile
  ): RewritePreset[] => {
    const raw = (p as any).presets;
    return Array.isArray(raw) ? raw : [];
  };

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
  }, [profiles]);

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
    null
  );
  const [linkSourcePresetId, setLinkSourcePresetId] = useState<string | null>(
    null
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
  }, [activeProfileId, activeProfile, presets, editingPresetId]);

  useEffect(() => {
    if (!selectedPreset) {
      setLocalPresetName("");
      setLocalPresetHintsText("");
      return;
    }

    setLocalPresetName(selectedPreset.name);
    const lines = (selectedPreset.routing_hints ?? []).filter(Boolean);
    setLocalPresetHintsText(lines.join("\n"));
  }, [selectedPreset?.id]);

  useEffect(() => {
    if (!activeProfile) {
      setLocalDefaultPresetDescription("");
      return;
    }
    setLocalDefaultPresetDescription(
      activeProfile.default_preset_description ?? ""
    );
  }, [activeProfileId, activeProfile?.default_preset_description]);

  const savePresets = (
    nextPresets: RewritePreset[],
    extra?: Partial<RewriteProgramPromptProfile>
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
            p.id === presetId ? { ...p, ...patch } : p
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
      p.id === presetId ? { ...p, ...patch } : p
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

  const linkableProfiles = useMemo(() => {
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
  }, [profiles, activeProfileId]);

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

    // “Hard link” semantics: we reuse the same preset id across profiles.
    // We still store an object in this profile, but updates propagate by id.
    const next = [...presets, { ...linkSourcePreset }];
    savePresets(next);
    pendingPresetIdRef.current = linkSourcePreset.id;
    setEditingPresetId(linkSourcePreset.id);
    setLinkPresetModalOpen(false);
  };

  const normalizeRouter = (
    router: IntentRouterSettings | null | undefined
  ): IntentRouterSettings => {
    const r = router ?? ({} as any);
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
        typeof r.pick_highest_score === "boolean" ? r.pick_highest_score : null,
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

      llm_provider: typeof r.llm_provider === "string" ? r.llm_provider : null,
      llm_model: typeof r.llm_model === "string" ? r.llm_model : null,
      openai_reasoning_effort:
        typeof r.openai_reasoning_effort === "string"
          ? (r.openai_reasoning_effort as any)
          : null,
      gemini_thinking_budget:
        typeof r.gemini_thinking_budget === "number" &&
        Number.isFinite(r.gemini_thinking_budget)
          ? r.gemini_thinking_budget
          : null,
      gemini_thinking_level:
        typeof r.gemini_thinking_level === "string"
          ? (r.gemini_thinking_level as any)
          : null,
      anthropic_thinking_budget:
        typeof r.anthropic_thinking_budget === "number" &&
        Number.isFinite(r.anthropic_thinking_budget)
          ? r.anthropic_thinking_budget
          : null,
      llm_system_prompt:
        typeof r.llm_system_prompt === "string" ? r.llm_system_prompt : null,
    };
  };

  const effectiveRouter: IntentRouterSettings | null = useMemo(() => {
    if (!activeProfile) return null;
    return normalizeRouter(activeProfile.router);
  }, [activeProfile?.router, activeProfileId]);

  const saveRouter = (router: IntentRouterSettings | null) => {
    if (!activeProfile) return;
    saveProfileMetadata({ router });
  };

  const runRewriteTest = (promptOverride?: string) => {
    setRewriteTestError("");
    setRewriteTestOutput("");
    setRewriteTestDurationMs(null);
    rewriteTestStartRef.current = performance.now();

    if (typeof promptOverride === "string") {
      testRewriteWithPrompt.mutate(
        {
          transcript: rewriteTestInput,
          prompt: promptOverride,
          profileId: activeProfileId,
        },
        {
          onSuccess: (res) => {
            const startedAt = rewriteTestStartRef.current;
            rewriteTestStartRef.current = null;
            if (typeof startedAt === "number") {
              setRewriteTestDurationMs(performance.now() - startedAt);
            }
            setRewriteTestOutput(res.output);
          },
          onError: (err) => {
            const startedAt = rewriteTestStartRef.current;
            rewriteTestStartRef.current = null;
            if (typeof startedAt === "number") {
              setRewriteTestDurationMs(performance.now() - startedAt);
            }
            setRewriteTestError(errorToMessage(err));
          },
        }
      );
      return;
    }

    testLlmRewrite.mutate(
      {
        transcript: rewriteTestInput,
        profileId: activeProfileId,
      },
      {
        onSuccess: (res) => {
          const startedAt = rewriteTestStartRef.current;
          rewriteTestStartRef.current = null;
          if (typeof startedAt === "number") {
            setRewriteTestDurationMs(performance.now() - startedAt);
          }
          setRewriteTestOutput(res.output);
        },
        onError: (err) => {
          const startedAt = rewriteTestStartRef.current;
          rewriteTestStartRef.current = null;
          if (typeof startedAt === "number") {
            setRewriteTestDurationMs(performance.now() - startedAt);
          }
          setRewriteTestError(errorToMessage(err));
        },
      }
    );
  };

  const [sttTestOutput, setSttTestOutput] = useState<string>("");
  const [sttTestError, setSttTestError] = useState<string>("");
  const [sttTestDurationMs, setSttTestDurationMs] = useState<number | null>(
    null
  );
  const [localSttTranscriptionPrompt, setLocalSttTranscriptionPrompt] =
    useState<string>("");
  const sttTestStartRef = useRef<number | null>(null);

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

  // Track whether profile settings are inheriting (original value was null)
  const [sttProviderInheriting, setSttProviderInheriting] = useState(false);
  const [sttModelInheriting, setSttModelInheriting] = useState(false);
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

  // NOTE: Settings tabs unmount when switching (keepMounted=false). If we render
  // switches immediately, they first render with placeholder values then “jump”
  // once settings load. We avoid that by showing a loader until we have settings
  // + defaults and local state is initialized.
  const [localSections, setLocalSections] = useState<LocalSections | null>(
    null
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
  }, [activeProfileId, activeProfile?.cleanup_prompt_sections]);

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

  useEffect(() => {
    if (activeProfile) {
      // Track whether each setting is inheriting (null in the profile)
      const sttProviderIsNull =
        activeProfile.stt_provider === null ||
        activeProfile.stt_provider === undefined;
      const sttModelIsNull =
        activeProfile.stt_model === null ||
        activeProfile.stt_model === undefined;
      const sttTimeoutIsNull =
        activeProfile.stt_timeout_seconds === null ||
        activeProfile.stt_timeout_seconds === undefined;
      const llmProviderIsNull =
        activeProfile.llm_provider === null ||
        activeProfile.llm_provider === undefined;
      const llmModelIsNull =
        activeProfile.llm_model === null ||
        activeProfile.llm_model === undefined;
      const rewriteEnabledIsNull =
        activeProfile.rewrite_llm_enabled === null ||
        activeProfile.rewrite_llm_enabled === undefined;

      const openAiReasoningEffortIsNull =
        activeProfile.openai_reasoning_effort === null ||
        activeProfile.openai_reasoning_effort === undefined;
      const geminiThinkingLevelIsNull =
        activeProfile.gemini_thinking_level === null ||
        activeProfile.gemini_thinking_level === undefined;
      const geminiThinkingBudgetIsNull =
        activeProfile.gemini_thinking_budget === null ||
        activeProfile.gemini_thinking_budget === undefined;
      const anthropicThinkingBudgetIsNull =
        activeProfile.anthropic_thinking_budget === null ||
        activeProfile.anthropic_thinking_budget === undefined;

      const quickAskProviderIsNull =
        activeProfile.quick_ask_provider === null ||
        activeProfile.quick_ask_provider === undefined;
      const quickAskModelIsNull =
        activeProfile.quick_ask_model === null ||
        activeProfile.quick_ask_model === undefined;
      const quickAskSystemPromptIsNull =
        activeProfile.quick_ask_system_prompt === null ||
        activeProfile.quick_ask_system_prompt === undefined;

      const defaultProfile = profiles.find((p) => p.id === "default") ?? null;

      // Quick Replace inherits from the Default profile. If Default has never been
      // configured, we fall back to the legacy global toggle for backward
      // compatibility.
      const baseQuickReplaceEnabled =
        typeof defaultProfile?.quick_replace_enabled === "boolean"
          ? defaultProfile.quick_replace_enabled
          : settings?.quick_replace_enabled ?? false;
      const baseQuickReplaceProvider =
        defaultProfile?.quick_replace_provider ??
        settings?.llm_provider ??
        null;
      const baseQuickReplaceModel =
        defaultProfile?.quick_replace_model ?? settings?.llm_model ?? null;
      const baseQuickReplaceSystemPrompt =
        defaultProfile?.quick_replace_system_prompt ??
        DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT;

      const baseRewriteIncludeClipboardContext =
        typeof defaultProfile?.rewrite_include_clipboard_context === "boolean"
          ? defaultProfile.rewrite_include_clipboard_context
          : false;
      const baseQuickReplaceIncludeClipboardContext =
        typeof defaultProfile?.quick_replace_include_clipboard_context ===
        "boolean"
          ? defaultProfile.quick_replace_include_clipboard_context
          : false;
      const baseQuickAskIncludeClipboardContext =
        typeof defaultProfile?.quick_ask_include_clipboard_context === "boolean"
          ? defaultProfile.quick_ask_include_clipboard_context
          : false;

      const quickReplaceEnabledIsNull =
        activeProfile.quick_replace_enabled === null ||
        activeProfile.quick_replace_enabled === undefined;
      const quickReplaceProviderIsNull =
        activeProfile.quick_replace_provider === null ||
        activeProfile.quick_replace_provider === undefined;
      const quickReplaceModelIsNull =
        activeProfile.quick_replace_model === null ||
        activeProfile.quick_replace_model === undefined;
      const quickReplaceSystemPromptIsNull =
        activeProfile.quick_replace_system_prompt === null ||
        activeProfile.quick_replace_system_prompt === undefined;

      const rewriteIncludeClipboardContextIsNull =
        activeProfile.rewrite_include_clipboard_context === null ||
        activeProfile.rewrite_include_clipboard_context === undefined;
      const quickReplaceIncludeClipboardContextIsNull =
        activeProfile.quick_replace_include_clipboard_context === null ||
        activeProfile.quick_replace_include_clipboard_context === undefined;
      const quickAskIncludeClipboardContextIsNull =
        activeProfile.quick_ask_include_clipboard_context === null ||
        activeProfile.quick_ask_include_clipboard_context === undefined;

      const quickAskOpenAiReasoningEffortIsNull =
        activeProfile.quick_ask_openai_reasoning_effort === null ||
        activeProfile.quick_ask_openai_reasoning_effort === undefined;
      const quickAskGeminiThinkingLevelIsNull =
        activeProfile.quick_ask_gemini_thinking_level === null ||
        activeProfile.quick_ask_gemini_thinking_level === undefined;
      const quickAskGeminiThinkingBudgetIsNull =
        activeProfile.quick_ask_gemini_thinking_budget === null ||
        activeProfile.quick_ask_gemini_thinking_budget === undefined;
      const quickAskAnthropicThinkingBudgetIsNull =
        activeProfile.quick_ask_anthropic_thinking_budget === null ||
        activeProfile.quick_ask_anthropic_thinking_budget === undefined;

      setSttProviderInheriting(sttProviderIsNull);
      setSttModelInheriting(sttModelIsNull);
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

      setQuickReplaceEnabledInheriting(quickReplaceEnabledIsNull);
      setQuickReplaceProviderInheriting(quickReplaceProviderIsNull);
      setQuickReplaceModelInheriting(quickReplaceModelIsNull);
      setQuickReplaceSystemPromptInheriting(quickReplaceSystemPromptIsNull);

      setRewriteIncludeClipboardContextInheriting(
        rewriteIncludeClipboardContextIsNull
      );
      setQuickReplaceIncludeClipboardContextInheriting(
        quickReplaceIncludeClipboardContextIsNull
      );
      setQuickAskIncludeClipboardContextInheriting(
        quickAskIncludeClipboardContextIsNull
      );

      setQuickAskOpenAiReasoningEffortInheriting(
        quickAskOpenAiReasoningEffortIsNull
      );
      setQuickAskGeminiThinkingLevelInheriting(
        quickAskGeminiThinkingLevelIsNull
      );
      setQuickAskGeminiThinkingBudgetInheriting(
        quickAskGeminiThinkingBudgetIsNull
      );
      setQuickAskAnthropicThinkingBudgetInheriting(
        quickAskAnthropicThinkingBudgetIsNull
      );

      // Set local state (falling back to global defaults for display)
      setLocalProfileSttProvider(
        activeProfile.stt_provider ?? settings?.stt_provider ?? null
      );
      setLocalProfileSttModel(
        activeProfile.stt_model ?? settings?.stt_model ?? null
      );
      setLocalProfileLlmProvider(
        activeProfile.llm_provider ?? settings?.llm_provider ?? null
      );
      setLocalProfileLlmModel(
        activeProfile.llm_model ?? settings?.llm_model ?? null
      );

      setLocalProfileQuickAskProvider(
        activeProfile.quick_ask_provider ??
          settings?.quick_ask_provider ??
          settings?.llm_provider ??
          null
      );
      setLocalProfileQuickAskModel(
        activeProfile.quick_ask_model ??
          settings?.quick_ask_model ??
          settings?.llm_model ??
          null
      );
      setLocalQuickAskSystemPrompt(
        activeProfile.quick_ask_system_prompt ??
          settings?.quick_ask_system_prompt ??
          ""
      );

      setLocalProfileQuickReplaceEnabled(
        activeProfileId === "default"
          ? typeof activeProfile.quick_replace_enabled === "boolean"
            ? activeProfile.quick_replace_enabled
            : settings?.quick_replace_enabled ?? false
          : activeProfile.quick_replace_enabled ?? baseQuickReplaceEnabled
      );
      setLocalProfileQuickReplaceProvider(
        activeProfileId === "default"
          ? activeProfile.quick_replace_provider ??
              settings?.llm_provider ??
              null
          : activeProfile.quick_replace_provider ?? baseQuickReplaceProvider
      );
      setLocalProfileQuickReplaceModel(
        activeProfileId === "default"
          ? activeProfile.quick_replace_model ?? settings?.llm_model ?? null
          : activeProfile.quick_replace_model ?? baseQuickReplaceModel
      );
      setLocalQuickReplaceSystemPrompt(
        activeProfileId === "default"
          ? activeProfile.quick_replace_system_prompt ??
              DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT
          : activeProfile.quick_replace_system_prompt ??
              baseQuickReplaceSystemPrompt
      );

      setLocalProfileRewriteIncludeClipboardContext(
        activeProfileId === "default"
          ? typeof activeProfile.rewrite_include_clipboard_context === "boolean"
            ? activeProfile.rewrite_include_clipboard_context
            : false
          : activeProfile.rewrite_include_clipboard_context ??
              baseRewriteIncludeClipboardContext
      );

      setLocalProfileQuickReplaceIncludeClipboardContext(
        activeProfileId === "default"
          ? typeof activeProfile.quick_replace_include_clipboard_context ===
            "boolean"
            ? activeProfile.quick_replace_include_clipboard_context
            : false
          : activeProfile.quick_replace_include_clipboard_context ??
              baseQuickReplaceIncludeClipboardContext
      );

      setLocalProfileQuickAskIncludeClipboardContext(
        activeProfileId === "default"
          ? typeof activeProfile.quick_ask_include_clipboard_context ===
            "boolean"
            ? activeProfile.quick_ask_include_clipboard_context
            : false
          : activeProfile.quick_ask_include_clipboard_context ??
              baseQuickAskIncludeClipboardContext
      );

      setLocalProfileOpenAiReasoningEffort(
        activeProfile.openai_reasoning_effort ?? "default"
      );
      setLocalProfileGeminiThinkingLevel(
        activeProfile.gemini_thinking_level ?? "default"
      );
      setLocalProfileGeminiThinkingBudget(
        activeProfile.gemini_thinking_budget == null
          ? "default"
          : String(activeProfile.gemini_thinking_budget)
      );
      setLocalProfileAnthropicThinkingBudget(
        activeProfile.anthropic_thinking_budget == null
          ? "default"
          : String(activeProfile.anthropic_thinking_budget)
      );
      setLocalProfileRewriteEnabled(
        activeProfile.rewrite_llm_enabled ?? defaultRewriteEnabled
      );
      setLocalProfileSttTimeout(
        activeProfile.stt_timeout_seconds ??
          settings?.stt_timeout_seconds ??
          DEFAULT_STT_TIMEOUT
      );

      setLocalProfileQuickAskOpenAiReasoningEffort(
        activeProfile.quick_ask_openai_reasoning_effort ?? "default"
      );
      setLocalProfileQuickAskGeminiThinkingLevel(
        activeProfile.quick_ask_gemini_thinking_level ?? "default"
      );
      setLocalProfileQuickAskGeminiThinkingBudget(
        activeProfile.quick_ask_gemini_thinking_budget == null
          ? "default"
          : String(activeProfile.quick_ask_gemini_thinking_budget)
      );
      setLocalProfileQuickAskAnthropicThinkingBudget(
        activeProfile.quick_ask_anthropic_thinking_budget == null
          ? "default"
          : String(activeProfile.quick_ask_anthropic_thinking_budget)
      );
    } else {
      // Default scope - not inheriting
      setSttProviderInheriting(false);
      setSttModelInheriting(false);
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

      setQuickReplaceEnabledInheriting(false);
      setQuickReplaceProviderInheriting(false);
      setQuickReplaceModelInheriting(false);
      setQuickReplaceSystemPromptInheriting(false);

      setRewriteIncludeClipboardContextInheriting(false);
      setQuickReplaceIncludeClipboardContextInheriting(false);
      setQuickAskIncludeClipboardContextInheriting(false);

      setQuickAskOpenAiReasoningEffortInheriting(false);
      setQuickAskGeminiThinkingLevelInheriting(false);
      setQuickAskGeminiThinkingBudgetInheriting(false);
      setQuickAskAnthropicThinkingBudgetInheriting(false);

      setLocalProfileSttProvider(null);
      setLocalProfileSttModel(null);
      setLocalProfileLlmProvider(null);
      setLocalProfileLlmModel(null);
      setLocalProfileQuickAskProvider(null);
      setLocalProfileQuickAskModel(null);
      setLocalQuickAskSystemPrompt(settings?.quick_ask_system_prompt ?? "");

      setLocalProfileQuickReplaceEnabled(
        settings?.quick_replace_enabled ?? false
      );
      setLocalProfileQuickReplaceProvider(settings?.llm_provider ?? null);
      setLocalProfileQuickReplaceModel(settings?.llm_model ?? null);
      setLocalQuickReplaceSystemPrompt(DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT);

      setLocalProfileRewriteIncludeClipboardContext(false);
      setLocalProfileQuickReplaceIncludeClipboardContext(false);
      setLocalProfileQuickAskIncludeClipboardContext(false);
      setLocalProfileRewriteEnabled(defaultRewriteEnabled);
      setLocalProfileSttTimeout(
        settings?.stt_timeout_seconds ?? DEFAULT_STT_TIMEOUT
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
    settings?.stt_timeout_seconds,
    settings?.stt_provider,
    settings?.stt_model,
    settings?.llm_provider,
    settings?.llm_model,
    settings?.quick_ask_provider,
    settings?.quick_ask_model,
    settings?.quick_ask_system_prompt,
    defaultRewriteEnabled,
    profiles,
    settings?.quick_replace_enabled,
  ]);

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
      (p) => p.id !== "default" && typeof p.rewrite_llm_enabled !== "boolean"
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
  // dropdown (e.g. on a fresh install before API keys are configured). This
  // keeps model pickers hidden/disabled until a real provider is selectable.
  const sttProviderValueSet = new Set(
    [...sttCloudProviders, ...sttLocalProviders].map((p) => p.value)
  );
  const llmProviderValueSet = new Set(
    [...llmCloudProviders, ...llmLocalProviders].map((p) => p.value)
  );

  const rawSttProvider =
    activeProfileId === "default"
      ? settings?.stt_provider ?? null
      : localProfileSttProvider ?? settings?.stt_provider ?? null;
  const effectiveSttProvider =
    rawSttProvider && sttProviderValueSet.has(rawSttProvider)
      ? rawSttProvider
      : null;

  const effectiveSttModel =
    effectiveSttProvider === null
      ? null
      : activeProfileId === "default"
      ? settings?.stt_model ?? null
      : localProfileSttModel ?? settings?.stt_model ?? null;

  const rawLlmProvider =
    activeProfileId === "default"
      ? settings?.llm_provider ?? null
      : localProfileLlmProvider ?? settings?.llm_provider ?? null;
  const effectiveLlmProvider =
    rawLlmProvider && llmProviderValueSet.has(rawLlmProvider)
      ? rawLlmProvider
      : null;

  const rawQuickAskProvider =
    activeProfileId === "default"
      ? settings?.quick_ask_provider ?? settings?.llm_provider ?? null
      : localProfileQuickAskProvider ??
        settings?.quick_ask_provider ??
        settings?.llm_provider ??
        null;
  const effectiveQuickAskProvider =
    rawQuickAskProvider && llmProviderValueSet.has(rawQuickAskProvider)
      ? rawQuickAskProvider
      : null;

  const defaultProfile = profiles.find((p) => p.id === "default") ?? null;

  const defaultQuickReplaceEnabled =
    typeof defaultProfile?.quick_replace_enabled === "boolean"
      ? defaultProfile.quick_replace_enabled
      : settings?.quick_replace_enabled ?? false;
  const defaultQuickReplaceProvider =
    defaultProfile?.quick_replace_provider ?? settings?.llm_provider ?? null;
  const defaultQuickReplaceModel =
    defaultProfile?.quick_replace_model ?? settings?.llm_model ?? null;
  const defaultQuickReplaceSystemPrompt =
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
      ? localProfileQuickReplaceProvider ?? settings?.llm_provider ?? null
      : localProfileQuickReplaceProvider ?? defaultQuickReplaceProvider;
  const effectiveQuickReplaceProvider =
    rawQuickReplaceProvider && llmProviderValueSet.has(rawQuickReplaceProvider)
      ? rawQuickReplaceProvider
      : null;

  const effectiveQuickAskModel =
    effectiveQuickAskProvider === null
      ? null
      : activeProfileId === "default"
      ? settings?.quick_ask_model ?? settings?.llm_model ?? null
      : localProfileQuickAskModel ??
        settings?.quick_ask_model ??
        settings?.llm_model ??
        null;

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

  const routerLlmProvider = effectiveRouter?.llm_provider ?? null;

  const fireworksModelsQuery = useFireworksModels(
    effectiveLlmProvider === "fireworks" ||
      effectiveQuickAskProvider === "fireworks" ||
      effectiveQuickReplaceProvider === "fireworks" ||
      routerLlmProvider === "fireworks"
  );

  const ollamaModelsQuery = useOllamaModels(
    effectiveLlmProvider === "ollama" ||
      effectiveQuickAskProvider === "ollama" ||
      effectiveQuickReplaceProvider === "ollama" ||
      routerLlmProvider === "ollama"
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
    ? STT_MODELS[effectiveSttProvider] ?? []
    : [];

  const llmModelOptions = getLlmModelOptionsForProvider(effectiveLlmProvider);
  const quickAskModelOptions = getLlmModelOptionsForProvider(
    effectiveQuickAskProvider
  );
  const quickReplaceModelOptions = getLlmModelOptionsForProvider(
    effectiveQuickReplaceProvider
  );

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

  const selectedSttModelForUi =
    sttModelOptions.length === 0
      ? null
      : isDefaultScope
      ? settings?.stt_model ?? sttModelOptions[0]?.value ?? null
      : localProfileSttModel;

  const [whisperServerModelDraft, setWhisperServerModelDraft] = useState("");

  useEffect(() => {
    if (!sttProviderIsWhisperServer) return;
    setWhisperServerModelDraft(selectedSttModelForUi ?? "");
  }, [sttProviderIsWhisperServer, selectedSttModelForUi]);

  const selectedQuickAskModelForUi =
    quickAskModelOptions.length === 0
      ? null
      : isDefaultScope
      ? settings?.quick_ask_model ??
        (effectiveQuickAskProvider === effectiveLlmProvider
          ? settings?.llm_model
          : null) ??
        quickAskModelOptions[0]?.value ??
        null
      : localProfileQuickAskModel;

  const selectedQuickReplaceModelForUi =
    quickReplaceModelOptions.length === 0
      ? null
      : isDefaultScope
      ? localProfileQuickReplaceModel ??
        (effectiveQuickReplaceProvider === effectiveLlmProvider
          ? settings?.llm_model
          : null) ??
        quickReplaceModelOptions[0]?.value ??
        null
      : localProfileQuickReplaceModel;

  const effectiveLlmModel =
    effectiveLlmProvider === null
      ? null
      : activeProfileId === "default"
      ? settings?.llm_model ?? null
      : localProfileLlmModel ?? settings?.llm_model ?? null;

  const selectedLlmModelForUi =
    llmModelOptions.length === 0
      ? null
      : isDefaultScope
      ? settings?.llm_model ?? llmModelOptions[0]?.value ?? null
      : localProfileLlmModel;

  const sttPricing = useModelPricing(
    effectiveSttProvider,
    "stt",
    selectedSttModelForUi
  );
  const llmPricing = useModelPricing(
    effectiveLlmProvider,
    "llm",
    selectedLlmModelForUi
  );

  const sttPricingLabel = useMemo(() => {
    const stt = sttPricing.data?.stt;
    if (!stt) return null;

    const minSecs =
      typeof stt.min_billed_secs === "number" ? stt.min_billed_secs : null;

    const withMinBill = (base: string) =>
      minSecs ? `${base} · min ${minSecs}s` : base;

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
    return `in ${input} · out ${output} /1M tok`;
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
            })
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
      (o) => o.value === asString
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
      (o) => o.value === asString
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

  const handleQuickAskOpenAiThinkingChange = (value: string | null) => {
    if (value == null || value === SELECT_DEFAULT) {
      updateQuickAskOpenAiReasoningEffort.mutate(null, {
        onSuccess: () => {
          tauriAPI.emitSettingsChanged();
        },
      });
      return;
    }

    updateQuickAskOpenAiReasoningEffort.mutate(value as any, {
      onSuccess: () => {
        tauriAPI.emitSettingsChanged();
      },
    });
  };

  const handleQuickAskGeminiThinkingLevelChange = (value: string | null) => {
    const v =
      value === "minimal" ||
      value === "low" ||
      value === "medium" ||
      value === "high"
        ? value
        : null;
    updateQuickAskGeminiThinkingLevel.mutate(v, {
      onSuccess: () => {
        tauriAPI.emitSettingsChanged();
      },
    });
  };

  const handleQuickAskGeminiThinkingBudgetChange = (value: string | null) => {
    if (value == null || value === SELECT_DEFAULT) {
      updateQuickAskGeminiThinkingBudget.mutate(null, {
        onSuccess: () => {
          tauriAPI.emitSettingsChanged();
        },
      });
      return;
    }

    const parsed = Number(value);
    if (!Number.isFinite(parsed)) return;
    updateQuickAskGeminiThinkingBudget.mutate(parsed, {
      onSuccess: () => {
        tauriAPI.emitSettingsChanged();
      },
    });
  };

  const handleQuickAskAnthropicThinkingBudgetChange = (
    value: string | null
  ) => {
    if (value == null || value === SELECT_DEFAULT) {
      updateQuickAskAnthropicThinkingBudget.mutate(null, {
        onSuccess: () => {
          tauriAPI.emitSettingsChanged();
        },
      });
      return;
    }

    const parsed = Number(value);
    if (!Number.isFinite(parsed)) return;
    updateQuickAskAnthropicThinkingBudget.mutate(parsed, {
      onSuccess: () => {
        tauriAPI.emitSettingsChanged();
      },
    });
  };

  const baseStoredSections: CleanupPromptSections =
    settings?.cleanup_prompt_sections &&
    (settings.cleanup_prompt_sections as any).system
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
    overrides: CleanupPromptSectionsOverride
  ): CleanupPromptSectionsOverride | null => {
    const hasAny = overrides.system != null;
    return hasAny ? overrides : null;
  };

  const saveProfileSectionOverride = (
    key: SectionKey,
    section: CleanupPromptSections[SectionKey] | null
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
    }
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
      p.id === activeProfileId ? { ...p, ...next } : p
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
      buildSectionOverride(key, { content: null })
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
    activeProfile?.default_target_rewrite_llm_enabled ?? true ? "on" : "off";

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

  const profilePromptDefaultContent = localSections.system.content ?? "";

  const getPresetPromptOverride = (
    preset: RewritePreset,
    key: SectionKey
  ): CleanupPromptSections[SectionKey] | null => {
    const o = preset.cleanup_prompt_sections ?? null;
    if (!o) return null;
    const v = (o as any)[key];
    return v ?? null;
  };

  const savePresetSectionOverride = (
    preset: RewritePreset,
    key: SectionKey,
    section: CleanupPromptSections[SectionKey] | null
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

  return (
    <>
      <Modal
        opened={linkPresetModalOpen}
        onClose={() => setLinkPresetModalOpen(false)}
        title="Add preset from another profile"
        centered
        zIndex={1200}
      >
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          <Select
            label="Source profile"
            data={linkableProfiles.map((p) => ({
              value: p.id,
              label: p.label,
            }))}
            value={linkSourceProfileId}
            onChange={(value) => {
              if (!value) return;
              setLinkSourceProfileId(value);
              const nextProfile =
                linkableProfiles.find((p) => p.id === value) ?? null;
              setLinkSourcePresetId(nextProfile?.presets[0]?.id ?? null);
            }}
            placeholder={
              linkableProfiles.length === 0
                ? "No other profiles"
                : "Select profile"
            }
            withCheckIcon={false}
          />

          <Select
            label="Preset"
            data={(linkSourceProfile?.presets ?? []).map((p) => ({
              value: p.id,
              label: p.name?.trim() || p.id,
            }))}
            value={linkSourcePresetId}
            onChange={(value) => {
              if (!value) return;
              setLinkSourcePresetId(value);
            }}
            disabled={!linkSourceProfile}
            placeholder={
              !linkSourceProfile ? "Select a profile first" : "Select preset"
            }
            withCheckIcon={false}
          />
        </div>

        <Group justify="flex-end" mt="md" gap="sm">
          <Button
            variant="default"
            onClick={() => setLinkPresetModalOpen(false)}
          >
            Cancel
          </Button>
          <Button
            color="gray"
            onClick={confirmLinkPreset}
            disabled={!linkSourcePreset}
          >
            Add Preset
          </Button>
        </Group>
      </Modal>

      <Modal
        opened={deletePresetDialog !== null}
        onClose={() => setDeletePresetDialog(null)}
        title="Delete preset?"
        centered
        zIndex={1300}
      >
        <Text size="sm" c="dimmed" style={{ lineHeight: 1.4 }}>
          {deletePresetDialog?.isShared
            ? "This preset is shared. Deleting it here only removes it from this profile; other profiles will keep it."
            : "This will remove the preset from this profile."}
        </Text>

        <Text size="sm" mt="xs" style={{ lineHeight: 1.4 }}>
          {deletePresetDialog?.presetName ?? ""}
        </Text>

        <Group justify="flex-end" mt="md" gap="sm">
          <Button variant="default" onClick={() => setDeletePresetDialog(null)}>
            Cancel
          </Button>
          <Button
            color="red"
            onClick={() => {
              const args = deletePresetDialog;
              if (!args) return;
              setDeletePresetDialog(null);
              deletePreset(args.presetId);
            }}
          >
            Delete
          </Button>
        </Group>
      </Modal>

      <Modal
        opened={resetDialog !== null}
        onClose={() => setResetDialog(null)}
        title={resetDialog?.title ?? ""}
        centered
      >
        <Text size="sm" c="dimmed" style={{ lineHeight: 1.4 }}>
          This setting is currently overriding the Default profile. Disable the
          override to inherit from Default.
        </Text>
        <Group justify="flex-end" mt="md" gap="sm">
          <Button variant="default" onClick={() => setResetDialog(null)}>
            Keep override
          </Button>
          <Button
            color="gray"
            onClick={() => {
              const confirm = resetDialog?.onConfirm;
              setResetDialog(null);
              confirm?.();
            }}
          >
            Disable override
          </Button>
        </Group>
      </Modal>

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

      <div className="settings-mini-header settings-mini-header--first">
        <span className="settings-mini-header__text">Transcribe</span>
      </div>

      <div className="settings-row">
        <div>
          <p className="settings-label">Speech-to-Text Provider</p>
          <p className="settings-description">Service for transcribing audio</p>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {!isDefaultScope && sttProviderInheriting && (
            <Tooltip label={INHERIT_TOOLTIP} withArrow>
              <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
            </Tooltip>
          )}
          {!isDefaultScope && !sttProviderInheriting && (
            <Tooltip label="Disable override (inherit from Default)" withArrow>
              <ActionIcon
                variant="subtle"
                color="gray"
                size="sm"
                onClick={() =>
                  openDisableOverrideDialog({
                    title: "Disable Speech-to-Text Provider override?",
                    onConfirm: () => {
                      setSttProviderInheriting(true);
                      setSttModelInheriting(true);
                      setLocalProfileSttProvider(
                        settings?.stt_provider ?? null
                      );
                      setLocalProfileSttModel(settings?.stt_model ?? null);
                      saveProfileMetadata({
                        stt_provider: null,
                        stt_model: null,
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
            data={sttProviderOptions}
            value={effectiveSttProvider}
            onChange={(value) => {
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
            }}
            placeholder="Select provider"
            withCheckIcon={false}
            disabled={
              sttCloudProviders.length === 0 && sttLocalProviders.length === 0
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

      {sttProviderIsWhisperServer || sttModelOptions.length > 0 ? (
        <div className="settings-row">
          <div>
            <p className="settings-label">STT Model</p>
            <p className="settings-description">
              Model to use for transcription
            </p>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            {!isDefaultScope && sttModelInheriting && (
              <Tooltip label={INHERIT_TOOLTIP} withArrow>
                <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
              </Tooltip>
            )}
            {!isDefaultScope && !sttModelInheriting && (
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
                      title: "Disable STT Model override?",
                      onConfirm: () => {
                        setSttModelInheriting(true);
                        setLocalProfileSttModel(settings?.stt_model ?? null);
                        saveProfileMetadata({ stt_model: null });
                      },
                    })
                  }
                >
                  <RotateCcw size={14} style={{ opacity: 0.65 }} />
                </ActionIcon>
              </Tooltip>
            )}
            {sttPricingLabel ? (
              <Text
                size="xs"
                c="dimmed"
                style={{ whiteSpace: "nowrap", lineHeight: 1 }}
              >
                {sttPricingLabel}
              </Text>
            ) : null}
            {sttProviderIsWhisperServer ? (
              <TextInput
                value={whisperServerModelDraft}
                onChange={(e) =>
                  setWhisperServerModelDraft(e.currentTarget.value)
                }
                onBlur={() => {
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
                }}
                placeholder="whisper-1"
                styles={{
                  input: {
                    backgroundColor: "var(--bg-elevated)",
                    borderColor: "var(--border-default)",
                    color: "var(--text-primary)",
                    minWidth: 200,
                  },
                }}
              />
            ) : (
              <Select
                data={sttModelOptions}
                value={selectedSttModelForUi}
                onChange={(value) => {
                  if (!value) return;
                  if (isDefaultScope) {
                    handleDefaultSTTModelChange(value);
                    return;
                  }
                  setSttModelInheriting(false);
                  setLocalProfileSttModel(value);
                  saveProfileMetadata({ stt_model: value });
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
            )}
          </div>
        </div>
      ) : null}

      <div className="settings-row no-divider">
        <div>
          <p className="settings-label">STT Timeout</p>
          <p className="settings-description">
            Increase if nothing is getting transcribed (seconds)
          </p>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {!isDefaultScope && sttTimeoutInheriting && (
            <Tooltip label={INHERIT_TOOLTIP} withArrow>
              <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
            </Tooltip>
          )}
          {!isDefaultScope && !sttTimeoutInheriting && (
            <Tooltip label="Disable override (inherit from Default)" withArrow>
              <ActionIcon
                variant="subtle"
                color="gray"
                size="sm"
                onClick={() =>
                  openDisableOverrideDialog({
                    title: "Disable STT Timeout override?",
                    onConfirm: () => {
                      setSttTimeoutInheriting(true);
                      setLocalProfileSttTimeout(
                        settings?.stt_timeout_seconds ?? DEFAULT_STT_TIMEOUT
                      );
                      saveProfileMetadata({ stt_timeout_seconds: null });
                    },
                  })
                }
              >
                <RotateCcw size={14} style={{ opacity: 0.65 }} />
              </ActionIcon>
            </Tooltip>
          )}

          <NumberInput
            value={localProfileSttTimeout}
            onChange={(value) => {
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
            }}
            onBlur={() => {
              // On blur, clamp and normalize.
              // If the user cleared the input, revert to the effective value without saving.
              if (localProfileSttTimeout === "") {
                const fallback = isDefaultScope
                  ? settings?.stt_timeout_seconds ?? DEFAULT_STT_TIMEOUT
                  : activeProfile?.stt_timeout_seconds ??
                    settings?.stt_timeout_seconds ??
                    DEFAULT_STT_TIMEOUT;
                setLocalProfileSttTimeout(fallback);
                return;
              }

              if (
                typeof localProfileSttTimeout !== "number" ||
                Number.isNaN(localProfileSttTimeout)
              ) {
                return;
              }

              const clamped = Math.max(
                5,
                Math.min(120, localProfileSttTimeout)
              );
              if (clamped !== localProfileSttTimeout) {
                setLocalProfileSttTimeout(clamped);
              }

              if (isDefaultScope) {
                handleDefaultSTTTimeoutChange(clamped);
                return;
              }

              setSttTimeoutInheriting(false);
              saveProfileMetadata({ stt_timeout_seconds: clamped });
            }}
            min={5}
            max={120}
            step={1}
            clampBehavior="blur"
            styles={{
              input: {
                backgroundColor: "var(--bg-elevated)",
                borderColor: "var(--border-default)",
                color: "var(--text-primary)",
                width: 140,
              },
            }}
          />
        </div>
      </div>

      <div style={{ marginTop: 0, marginBottom: 16 }}>
        <Accordion variant="separated" radius="md">
          <Accordion.Item value={`${activeProfileId}-stt-prompt`}>
            <Accordion.Control>
              <Tooltip
                label={sttPromptDisabledReason}
                withArrow
                disabled={sttPromptSupported}
              >
                <div style={{ opacity: sttPromptSupported ? 1 : 0.5 }}>
                  <p className="settings-label">Transcription prompt</p>
                  <p className="settings-description">
                    Optional context used during transcription.
                  </p>
                </div>
              </Tooltip>
            </Accordion.Control>
            <Accordion.Panel>
              <div
                style={{ display: "flex", flexDirection: "column", gap: 10 }}
              >
                <div style={{ width: "100%" }}>
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      gap: 12,
                      marginBottom: 6,
                    }}
                  >
                    {isPrompt224CharLimited ? (
                      <Text size="xs" c="dimmed">
                        {localSttTranscriptionPrompt.length}/{promptMaxChars}{" "}
                        chars
                      </Text>
                    ) : null}
                  </div>

                  <Textarea
                    value={localSttTranscriptionPrompt}
                    onChange={(e) => {
                      const next = e.currentTarget.value;

                      if (
                        isPrompt224CharLimited &&
                        next.length > promptMaxChars
                      ) {
                        setLocalSttTranscriptionPrompt(
                          next.slice(0, promptMaxChars)
                        );
                        return;
                      }

                      setLocalSttTranscriptionPrompt(next);
                    }}
                    disabled={!sttPromptSupported}
                    placeholder={"Prompt"}
                    autosize
                    minRows={2}
                    maxLength={
                      isPrompt224CharLimited ? promptMaxChars : undefined
                    }
                    styles={{
                      input: {
                        backgroundColor: "var(--bg-elevated)",
                        borderColor: "var(--border-default)",
                        color: "var(--text-primary)",
                        fontFamily: "monospace",
                        fontSize: "13px",
                      },
                    }}
                  />
                </div>
              </div>
            </Accordion.Panel>
          </Accordion.Item>

          <Accordion.Item value={`${activeProfileId}-stt-test`}>
            <Accordion.Control>
              <div>
                <p className="settings-label">Test transcription</p>
                <p className="settings-description">
                  Run STT on the last recorded audio to validate provider/model
                  settings.
                </p>
              </div>
            </Accordion.Control>
            <Accordion.Panel>
              <div
                style={{ display: "flex", flexDirection: "column", gap: 10 }}
              >
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    gap: 12,
                  }}
                >
                  <Text size="sm" c="dimmed">
                    {testSttLastAudio.isPending
                      ? "Duration: running…"
                      : sttTestDurationMs === null
                      ? "Duration: —"
                      : `Duration: ${(sttTestDurationMs / 1000).toFixed(2)}s`}
                  </Text>

                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 12,
                      marginLeft: "auto",
                    }}
                  >
                    <Text size="sm" c="dimmed">
                      Test with last created audio (and test audio settings)
                    </Text>
                    <Tooltip
                      label={
                        hasLastAudioForSttTest
                          ? undefined
                          : "No previous audio found. Record once (toggle/hold hotkey), then come back and click Test."
                      }
                      withArrow
                      disabled={Boolean(hasLastAudioForSttTest)}
                    >
                      <span>
                        <Button
                          color="gray"
                          loading={testSttLastAudio.isPending}
                          disabled={!hasLastAudioForSttTest}
                          onClick={() => {
                            setSttTestError("");
                            setSttTestOutput("");
                            setSttTestDurationMs(null);
                            sttTestStartRef.current = performance.now();

                            testSttLastAudio.mutate(
                              {
                                profileId: activeProfileId,
                              },
                              {
                                onSuccess: (res) => {
                                  const startedAt = sttTestStartRef.current;
                                  sttTestStartRef.current = null;
                                  if (typeof startedAt === "number") {
                                    setSttTestDurationMs(
                                      performance.now() - startedAt
                                    );
                                  }

                                  setSttTestOutput(res);
                                },
                                onError: (err) => {
                                  const startedAt = sttTestStartRef.current;
                                  sttTestStartRef.current = null;
                                  if (typeof startedAt === "number") {
                                    setSttTestDurationMs(
                                      performance.now() - startedAt
                                    );
                                  }

                                  setSttTestError(errorToMessage(err));
                                },
                              }
                            );
                          }}
                        >
                          Test
                        </Button>
                      </span>
                    </Tooltip>
                  </div>
                </div>

                <div style={{ width: "100%" }}>
                  {sttTestError ? (
                    <Text size="sm" c="red" style={{ marginBottom: 8 }}>
                      {sttTestError}
                    </Text>
                  ) : null}

                  {sttPromptSupported &&
                  (settings?.stt_transcription_prompt ?? "").trim().length >
                    0 ? (
                    <Text size="xs" c="dimmed" style={{ marginBottom: 8 }}>
                      Test transcription will include your global transcription
                      prompt.
                    </Text>
                  ) : null}

                  <Textarea
                    value={sttTestOutput}
                    readOnly
                    placeholder="Transcript will appear here"
                    autosize
                    minRows={3}
                    styles={{
                      input: {
                        backgroundColor: "var(--bg-elevated)",
                        borderColor: "var(--border-default)",
                        color: "var(--text-primary)",
                        fontFamily: "monospace",
                        fontSize: "13px",
                      },
                    }}
                  />
                </div>
              </div>
            </Accordion.Panel>
          </Accordion.Item>
        </Accordion>
      </div>

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
                        defaultRewriteIncludeClipboardContext
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
                        settings?.llm_provider ?? null
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
                  ? settings?.llm_model ?? llmModelOptions[0]?.value ?? null
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
                  ? settings?.openai_reasoning_effort ?? SELECT_DEFAULT
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
                saveProfileMetadata({
                  openai_reasoning_effort: value as any,
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
                  : settings?.openai_reasoning_effort ??
                    (effectiveLlmModel
                      ? openAiDefaultReasoningEffortForModel(effectiveLlmModel)
                      : "medium");

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
                      · {hint}
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
                  : settings?.openai_reasoning_effort ??
                    (effectiveLlmModel
                      ? openAiDefaultReasoningEffortForModel(effectiveLlmModel)
                      : "medium");

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
                      · {hint}
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
                  ? settings?.gemini_thinking_level ?? SELECT_DEFAULT
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
                  : settings?.gemini_thinking_level ?? "high";

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
                      · {hint}
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
                  : settings?.gemini_thinking_level ?? "high";

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
                      · {hint}
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
                      · {hint}
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
                      · {hint}
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
                        · {hint}
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
                        · {hint}
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
        <>
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

                    <Modal
                      opened={presetEditorOpen}
                      onClose={() => setPresetEditorOpen(false)}
                      title="Edit presets"
                      centered
                      size="xl"
                      keepMounted={false}
                      zIndex={1000}
                      styles={{
                        body: {
                          height: "70vh",
                          overflowY: "auto",
                        },
                      }}
                    >
                      <Group
                        justify="space-between"
                        align="flex-end"
                        wrap="wrap"
                        gap={12}
                        mb="sm"
                      >
                        <div style={{ flex: 1, minWidth: 260 }}>
                          <Text size="xs" c="dimmed" mb={4}>
                            Editing preset
                          </Text>
                          <Select
                            data={[
                              {
                                value: EDIT_DEFAULT_PRESET,
                                label: "Default",
                              },
                              ...presetSelectOptions,
                            ]}
                            value={editingPresetId ?? EDIT_DEFAULT_PRESET}
                            onChange={(value) => {
                              setEditingPresetId(value ?? EDIT_DEFAULT_PRESET);
                            }}
                            comboboxProps={{ withinPortal: true, zIndex: 1400 }}
                            placeholder="Default"
                            withCheckIcon={false}
                            styles={{
                              input: {
                                backgroundColor: "var(--bg-elevated)",
                                borderColor: "var(--border-default)",
                                color: "var(--text-primary)",
                              },
                            }}
                          />
                        </div>

                        <Group gap={8} wrap="wrap">
                          <Button color="gray" onClick={newPreset}>
                            New
                          </Button>
                          <Tooltip
                            label={
                              linkableProfiles.length === 0
                                ? "No presets found in other profiles"
                                : "Add a shared preset from another profile"
                            }
                            disabled={linkableProfiles.length > 0}
                          >
                            <Button
                              color="gray"
                              variant="light"
                              onClick={openLinkPresetModal}
                              disabled={linkableProfiles.length === 0}
                            >
                              Add
                            </Button>
                          </Tooltip>

                          <ActionIcon
                            variant="light"
                            color="red"
                            size={36}
                            disabled={isEditingDefaultPreset || !selectedPreset}
                            onClick={() => {
                              if (isEditingDefaultPreset) return;
                              if (!selectedPreset) return;
                              setDeletePresetDialog({
                                presetId: selectedPreset.id,
                                presetName:
                                  selectedPreset.name?.trim() ||
                                  selectedPreset.id,
                                isShared: isSharedPresetId(selectedPreset.id),
                              });
                            }}
                            aria-label="Delete preset"
                          >
                            <Trash2 size={16} />
                          </ActionIcon>
                        </Group>
                      </Group>

                      <div
                        style={{
                          display: "flex",
                          flexDirection: "column",
                          gap: 12,
                        }}
                      >
                        {selectedPreset ? (
                          <div
                            style={{
                              display: "flex",
                              flexDirection: "column",
                              gap: 12,
                            }}
                          >
                            <TextInput
                              label="Preset name"
                              value={localPresetName}
                              onChange={(e) =>
                                setLocalPresetName(e.currentTarget.value)
                              }
                              onBlur={() => {
                                const next = localPresetName.trim();
                                if (next && next !== selectedPreset.name) {
                                  updatePreset(selectedPreset.id, {
                                    name: next,
                                  });
                                }
                              }}
                              styles={{
                                label: { fontSize: 12 },
                                input: {
                                  backgroundColor: "var(--bg-elevated)",
                                  borderColor: "var(--border-default)",
                                  color: "var(--text-primary)",
                                },
                              }}
                            />

                            <div>
                              <Text size="xs" c="dimmed" mb={4}>
                                Rewrite step
                              </Text>
                              <Select
                                data={[
                                  { value: "on", label: "On" },
                                  { value: "off", label: "Off" },
                                ]}
                                value={
                                  selectedPreset.rewrite_llm_enabled
                                    ? "on"
                                    : "off"
                                }
                                onChange={(value) => {
                                  if (!value) return;
                                  updatePreset(selectedPreset.id, {
                                    rewrite_llm_enabled: value === "on",
                                  });
                                }}
                                comboboxProps={{
                                  withinPortal: true,
                                  zIndex: 1400,
                                }}
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

                            <Textarea
                              label="Routing hints (one per line)"
                              description="If empty, the router falls back to the preset name."
                              value={localPresetHintsText}
                              onChange={(e) =>
                                setLocalPresetHintsText(e.currentTarget.value)
                              }
                              onBlur={() => {
                                const lines = localPresetHintsText
                                  .split(/\r?\n/)
                                  .map((s) => s.trim())
                                  .filter(Boolean);
                                const next = lines.length === 0 ? null : lines;
                                const current =
                                  selectedPreset.routing_hints ?? null;
                                if (
                                  JSON.stringify(current) !==
                                  JSON.stringify(next)
                                ) {
                                  updatePreset(selectedPreset.id, {
                                    routing_hints: next,
                                  });
                                }
                              }}
                              autosize
                              minRows={3}
                              styles={{
                                label: { fontSize: 12 },
                                input: {
                                  backgroundColor: "var(--bg-elevated)",
                                  borderColor: "var(--border-default)",
                                  color: "var(--text-primary)",
                                  fontFamily: "monospace",
                                  fontSize: "13px",
                                },
                              }}
                            />

                            <div>
                              <Text size="sm" fw={600} mb={6}>
                                System Prompt override (relative to this
                                profile)
                              </Text>

                              <Accordion variant="separated" radius="md">
                                {(() => {
                                  const key: SectionKey = "system";
                                  const override = getPresetPromptOverride(
                                    selectedPreset,
                                    key
                                  );
                                  const baseContent =
                                    profilePromptDefaultContent;

                                  const initialContent =
                                    override && override.content != null
                                      ? override.content
                                      : baseContent;

                                  const presetLabel =
                                    selectedPreset.name?.trim() ||
                                    selectedPreset.id;

                                  return (
                                    <PromptSectionEditor
                                      key={`${activeProfileId}-${selectedPreset.id}-${key}`}
                                      sectionKey={`${activeProfileId}-preset-${selectedPreset.id}-${key}`}
                                      title="System Prompt"
                                      description="Override the profile System Prompt for this preset."
                                      enabled={true}
                                      hideToggle={true}
                                      headerActions={
                                        <Button
                                          variant="light"
                                          color="gray"
                                          disabled={
                                            updateRewriteProgramPromptProfiles.isPending
                                          }
                                          onClick={() => {
                                            setPromptLabContextPrompt(
                                              (initialContent ?? "").trim()
                                            );
                                            setPromptLabContextLabel(
                                              `${activeProfileLabel} · ${presetLabel}`
                                            );
                                            setPromptLabApplyTarget({
                                              type: "preset",
                                              presetId: selectedPreset.id,
                                              key,
                                            });
                                            setPromptLabOpen(true);
                                          }}
                                        >
                                          Prompt Lab
                                        </Button>
                                      }
                                      initialContent={initialContent}
                                      defaultContent={baseContent}
                                      hasCustom={override != null}
                                      inheritMode={
                                        override == null
                                          ? "inheriting"
                                          : "overriding"
                                      }
                                      inheritTooltip="Inheriting from the profile System Prompt"
                                      disableOverrideTooltip="Disable override (inherit from profile)"
                                      onDisableOverride={() =>
                                        savePresetSectionOverride(
                                          selectedPreset,
                                          key,
                                          null
                                        )
                                      }
                                      resetLabel="Reset to Profile"
                                      onToggle={() => {}}
                                      onSave={(content) => {
                                        const contentToStore =
                                          content === baseContent
                                            ? null
                                            : content || null;
                                        const next = {
                                          content: contentToStore,
                                        };
                                        savePresetSectionOverride(
                                          selectedPreset,
                                          key,
                                          next
                                        );
                                      }}
                                      onReset={() =>
                                        savePresetSectionOverride(
                                          selectedPreset,
                                          key,
                                          null
                                        )
                                      }
                                      isSaving={
                                        updateRewriteProgramPromptProfiles.isPending
                                      }
                                    />
                                  );
                                })()}
                              </Accordion>
                            </div>

                            <div style={{ marginTop: 12 }}>
                              <Accordion variant="separated" radius="md">
                                <Accordion.Item
                                  value={`${activeProfileId}-${selectedPreset.id}-test-rewrite`}
                                >
                                  <Accordion.Control>
                                    <div>
                                      <p className="settings-label">
                                        Test rewrite
                                      </p>
                                      <p className="settings-description">
                                        Paste a raw transcript and run it
                                        through this preset’s effective System
                                        Prompt.
                                      </p>
                                    </div>
                                  </Accordion.Control>
                                  <Accordion.Panel>
                                    {(() => {
                                      const baseContent =
                                        profilePromptDefaultContent;
                                      const override = getPresetPromptOverride(
                                        selectedPreset,
                                        "system"
                                      );
                                      const promptForTest =
                                        override && override.content != null
                                          ? override.content
                                          : baseContent;

                                      const isDisabled =
                                        rewriteTestInput.trim().length === 0 ||
                                        updateRewriteProgramPromptProfiles.isPending ||
                                        updateCleanupPromptSections.isPending ||
                                        updateRewriteLlmEnabled.isPending;

                                      return (
                                        <div
                                          style={{
                                            display: "flex",
                                            flexDirection: "column",
                                            gap: 10,
                                          }}
                                        >
                                          <Text size="xs" c="dimmed">
                                            Testing: {activeProfileLabel} ·{" "}
                                            {selectedPreset.name?.trim() ||
                                              selectedPreset.id}
                                          </Text>

                                          <Textarea
                                            value={rewriteTestInput}
                                            onChange={(e) => {
                                              setRewriteTestInput(
                                                e.currentTarget.value
                                              );
                                            }}
                                            onKeyDown={(e) => {
                                              if (
                                                (e.ctrlKey || e.metaKey) &&
                                                e.key === "Enter"
                                              ) {
                                                e.preventDefault();
                                                if (!isDisabled) {
                                                  runRewriteTest(promptForTest);
                                                }
                                              }
                                            }}
                                            placeholder="Raw transcript"
                                            autosize
                                            minRows={3}
                                            styles={{
                                              input: {
                                                backgroundColor:
                                                  "var(--bg-elevated)",
                                                borderColor:
                                                  "var(--border-default)",
                                                color: "var(--text-primary)",
                                                fontFamily: "monospace",
                                                fontSize: "13px",
                                              },
                                            }}
                                          />

                                          <div
                                            style={{
                                              display: "flex",
                                              alignItems: "center",
                                              gap: 12,
                                            }}
                                          >
                                            <Button
                                              color="gray"
                                              loading={
                                                testRewriteWithPrompt.isPending
                                              }
                                              disabled={isDisabled}
                                              onClick={() =>
                                                runRewriteTest(promptForTest)
                                              }
                                            >
                                              Test
                                            </Button>

                                            <Text size="sm" c="dimmed">
                                              {testRewriteWithPrompt.isPending
                                                ? "Duration: running…"
                                                : rewriteTestDurationMs === null
                                                ? "Duration: —"
                                                : `Duration: ${(
                                                    rewriteTestDurationMs / 1000
                                                  ).toFixed(2)}s`}
                                            </Text>
                                          </div>

                                          {rewriteTestError ? (
                                            <Text size="sm" c="red">
                                              {rewriteTestError}
                                            </Text>
                                          ) : null}

                                          {rewriteTestOutput ? (
                                            <Textarea
                                              value={rewriteTestOutput}
                                              readOnly
                                              autosize
                                              minRows={3}
                                              styles={{
                                                input: {
                                                  backgroundColor:
                                                    "var(--bg-elevated)",
                                                  borderColor:
                                                    "var(--border-default)",
                                                  color: "var(--text-primary)",
                                                  fontFamily: "monospace",
                                                  fontSize: "13px",
                                                },
                                              }}
                                            />
                                          ) : null}
                                        </div>
                                      );
                                    })()}
                                  </Accordion.Panel>
                                </Accordion.Item>
                              </Accordion>
                            </div>
                          </div>
                        ) : isEditingDefaultPreset ? (
                          <>
                            <div
                              style={{
                                display: "flex",
                                flexWrap: "wrap",
                                gap: 12,
                                alignItems: "flex-end",
                              }}
                            >
                              <div>
                                <Text size="xs" c="dimmed" mb={4}>
                                  Rewrite step
                                </Text>
                                <Select
                                  data={[
                                    { value: "on", label: "On" },
                                    { value: "off", label: "Off" },
                                  ]}
                                  value={defaultPresetRewriteStepValue}
                                  onChange={(value) => {
                                    if (!value) return;
                                    saveProfileMetadata({
                                      default_target_rewrite_llm_enabled:
                                        value === "on",
                                    });
                                  }}
                                  comboboxProps={{
                                    withinPortal: true,
                                    zIndex: 1400,
                                  }}
                                  withCheckIcon={false}
                                  styles={{
                                    input: {
                                      backgroundColor: "var(--bg-elevated)",
                                      borderColor: "var(--border-default)",
                                      color: "var(--text-primary)",
                                      minWidth: 240,
                                    },
                                  }}
                                />
                              </div>
                            </div>

                            <div style={{ marginTop: 8 }}>
                              <Accordion variant="separated" radius="md">
                                <PromptSectionEditor
                                  sectionKey={`${activeProfileId}-default-system-prompt`}
                                  title="System Prompt"
                                  description="Instructions used when rewriting the transcript"
                                  enabled={true}
                                  hideToggle={true}
                                  headerActions={
                                    <Button
                                      variant="light"
                                      color="gray"
                                      disabled={
                                        updateCleanupPromptSections.isPending ||
                                        updateRewriteProgramPromptProfiles.isPending ||
                                        updateRewriteLlmEnabled.isPending
                                      }
                                      onClick={() => {
                                        setPromptLabContextPrompt(
                                          effectiveCurrentPrompt
                                        );
                                        setPromptLabContextLabel(
                                          activeProfileLabel
                                        );
                                        setPromptLabApplyTarget({
                                          type: "profile",
                                          key: "system",
                                        });
                                        setPromptLabOpen(true);
                                      }}
                                    >
                                      Prompt Lab
                                    </Button>
                                  }
                                  initialContent={localSections!.system.content}
                                  defaultContent={defaultSections?.system ?? ""}
                                  hasCustom={hasCustomContent.system}
                                  inheritMode={
                                    isDefaultScope
                                      ? null
                                      : activeProfile?.cleanup_prompt_sections
                                          ?.system == null
                                      ? "inheriting"
                                      : "overriding"
                                  }
                                  onDisableOverride={
                                    isDefaultScope
                                      ? undefined
                                      : () =>
                                          openDisableOverrideDialog({
                                            title:
                                              "Disable System Prompt override?",
                                            onConfirm: () => {
                                              const base =
                                                settings?.cleanup_prompt_sections ??
                                                DEFAULT_SECTIONS;

                                              const current: CleanupPromptSectionsOverride =
                                                activeProfile?.cleanup_prompt_sections ??
                                                {};
                                              const next =
                                                normalizePromptOverrides({
                                                  ...current,
                                                  system: null,
                                                });
                                              profilePromptOverridesRef.current =
                                                next;

                                              const resolved: CleanupPromptSections =
                                                {
                                                  system:
                                                    next?.system ?? base.system,
                                                };

                                              setLocalSections({
                                                system: {
                                                  content:
                                                    resolved.system.content ??
                                                    defaultSections!.system,
                                                },
                                              });

                                              saveProfileMetadata({
                                                cleanup_prompt_sections: next,
                                              });
                                            },
                                          })
                                  }
                                  onToggle={() => {}}
                                  onSave={(content) =>
                                    handleSave("system", content)
                                  }
                                  onReset={() => handleReset("system")}
                                  isSaving={
                                    updateCleanupPromptSections.isPending ||
                                    updateRewriteProgramPromptProfiles.isPending
                                  }
                                />
                              </Accordion>
                            </div>

                            <Textarea
                              label="Default target routing hints (optional)"
                              description="Used by the intent router when deciding to use the profile defaults (no preset). You can put multiple lines here; the router will treat them as additional hints."
                              value={localDefaultPresetDescription}
                              onChange={(e) =>
                                setLocalDefaultPresetDescription(
                                  e.currentTarget.value
                                )
                              }
                              onBlur={() => {
                                const trimmed =
                                  localDefaultPresetDescription.trim();
                                const next =
                                  trimmed.length === 0 ? null : trimmed;
                                if (
                                  (activeProfile?.default_preset_description ??
                                    null) !== next
                                ) {
                                  saveProfileMetadata({
                                    default_preset_description: next,
                                  });
                                }
                              }}
                              autosize
                              minRows={2}
                              styles={{
                                label: { fontSize: 12 },
                                input: {
                                  backgroundColor: "var(--bg-elevated)",
                                  borderColor: "var(--border-default)",
                                  color: "var(--text-primary)",
                                },
                              }}
                            />

                            <div style={{ marginTop: 12 }}>
                              <Accordion variant="separated" radius="md">
                                <Accordion.Item
                                  value={`${activeProfileId}-default-test-rewrite`}
                                >
                                  <Accordion.Control>
                                    <div>
                                      <p className="settings-label">
                                        Test rewrite
                                      </p>
                                      <p className="settings-description">
                                        Paste a raw transcript and run it
                                        through the Default preset.
                                      </p>
                                    </div>
                                  </Accordion.Control>
                                  <Accordion.Panel>
                                    {(() => {
                                      const promptForTest =
                                        localSections!.system.content ?? "";
                                      const isDisabled =
                                        rewriteTestInput.trim().length === 0 ||
                                        updateRewriteProgramPromptProfiles.isPending ||
                                        updateCleanupPromptSections.isPending ||
                                        updateRewriteLlmEnabled.isPending;

                                      return (
                                        <div
                                          style={{
                                            display: "flex",
                                            flexDirection: "column",
                                            gap: 10,
                                          }}
                                        >
                                          <Text size="xs" c="dimmed">
                                            Testing: {activeProfileLabel} ·
                                            Default
                                          </Text>

                                          <Textarea
                                            value={rewriteTestInput}
                                            onChange={(e) => {
                                              setRewriteTestInput(
                                                e.currentTarget.value
                                              );
                                            }}
                                            onKeyDown={(e) => {
                                              if (
                                                (e.ctrlKey || e.metaKey) &&
                                                e.key === "Enter"
                                              ) {
                                                e.preventDefault();
                                                if (!isDisabled) {
                                                  runRewriteTest(promptForTest);
                                                }
                                              }
                                            }}
                                            placeholder="Raw transcript"
                                            autosize
                                            minRows={3}
                                            styles={{
                                              input: {
                                                backgroundColor:
                                                  "var(--bg-elevated)",
                                                borderColor:
                                                  "var(--border-default)",
                                                color: "var(--text-primary)",
                                                fontFamily: "monospace",
                                                fontSize: "13px",
                                              },
                                            }}
                                          />

                                          <div
                                            style={{
                                              display: "flex",
                                              alignItems: "center",
                                              gap: 12,
                                            }}
                                          >
                                            <Button
                                              color="gray"
                                              loading={
                                                testRewriteWithPrompt.isPending
                                              }
                                              disabled={isDisabled}
                                              onClick={() =>
                                                runRewriteTest(promptForTest)
                                              }
                                            >
                                              Test
                                            </Button>

                                            <Text size="sm" c="dimmed">
                                              {testRewriteWithPrompt.isPending
                                                ? "Duration: running…"
                                                : rewriteTestDurationMs === null
                                                ? "Duration: —"
                                                : `Duration: ${(
                                                    rewriteTestDurationMs / 1000
                                                  ).toFixed(2)}s`}
                                            </Text>
                                          </div>

                                          {rewriteTestError ? (
                                            <Text size="sm" c="red">
                                              {rewriteTestError}
                                            </Text>
                                          ) : null}

                                          {rewriteTestOutput ? (
                                            <Textarea
                                              value={rewriteTestOutput}
                                              readOnly
                                              autosize
                                              minRows={3}
                                              styles={{
                                                input: {
                                                  backgroundColor:
                                                    "var(--bg-elevated)",
                                                  borderColor:
                                                    "var(--border-default)",
                                                  color: "var(--text-primary)",
                                                  fontFamily: "monospace",
                                                  fontSize: "13px",
                                                },
                                              }}
                                            />
                                          ) : null}
                                        </div>
                                      );
                                    })()}
                                  </Accordion.Panel>
                                </Accordion.Item>
                              </Accordion>
                            </div>
                          </>
                        ) : null}
                      </div>
                    </Modal>
                  </div>
                </Accordion.Panel>
              </Accordion.Item>

              <Accordion.Item value={`${activeProfileId}-intent-router`}>
                <Accordion.Control>
                  <div>
                    <p className="settings-label">Intent router</p>
                    <p className="settings-description">
                      Automatically select a preset based on the transcript.
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
                    {presets.length === 0 ? (
                      <Text size="sm" c="dimmed">
                        Add at least one preset to enable routing.
                      </Text>
                    ) : null}

                    <div
                      className="settings-row no-divider"
                      style={{ paddingTop: 0 }}
                    >
                      <div>
                        <p className="settings-label">Router strategy</p>
                        <p className="settings-description">
                          Off disables routing completely. Embeddings is fast
                          and deterministic; LLM can be more flexible but costs
                          more.
                        </p>
                      </div>
                      <Select
                        data={[
                          { value: "off", label: "Off" },
                          { value: "embeddings", label: "Embeddings" },
                          { value: "llm", label: "LLM" },
                        ]}
                        value={routerStrategyValue}
                        onChange={(value) => {
                          if (!value) return;
                          if (value === "off") {
                            saveRouter({
                              enabled: false,
                              strategy: "off",
                              embedding_provider: null,
                              embedding_model: null,
                              pick_highest_score: null,
                              similarity_threshold: null,
                              similarity_margin: null,
                              llm_provider: null,
                              llm_model: null,
                              openai_reasoning_effort: null,
                              gemini_thinking_budget: null,
                              gemini_thinking_level: null,
                              anthropic_thinking_budget: null,
                              llm_system_prompt: null,
                            });
                            return;
                          }

                          if (value === "embeddings") {
                            const provider = "openai";
                            const modelOptions =
                              EMBEDDING_MODELS[provider] ?? [];
                            const modelValue = modelOptions[0]?.value ?? null;

                            saveRouter({
                              enabled: true,
                              strategy: "embeddings",
                              embedding_provider: provider,
                              embedding_model: modelValue,
                              pick_highest_score:
                                effectiveRouter?.pick_highest_score ?? true,
                              similarity_threshold:
                                effectiveRouter?.similarity_threshold ?? null,
                              similarity_margin:
                                effectiveRouter?.similarity_margin ?? null,
                              llm_provider: null,
                              llm_model: null,
                              openai_reasoning_effort: null,
                              gemini_thinking_budget: null,
                              gemini_thinking_level: null,
                              anthropic_thinking_budget: null,
                              llm_system_prompt: null,
                            });
                            return;
                          }

                          const seedProvider =
                            settings?.llm_provider ??
                            effectiveRouter?.llm_provider ??
                            "openai";
                          const modelOptions =
                            getLlmModelOptionsForProvider(seedProvider);
                          const seedModel =
                            effectiveRouter?.llm_model ??
                            settings?.llm_model ??
                            modelOptions[0]?.value ??
                            null;

                          saveRouter({
                            enabled: true,
                            strategy: "llm",
                            embedding_provider: null,
                            embedding_model: null,
                            pick_highest_score: null,
                            similarity_threshold: null,
                            similarity_margin: null,

                            llm_provider: seedProvider,
                            llm_model: seedModel,
                            openai_reasoning_effort:
                              settings?.openai_reasoning_effort ?? null,
                            gemini_thinking_budget:
                              settings?.gemini_thinking_budget ?? null,
                            gemini_thinking_level:
                              settings?.gemini_thinking_level ?? null,
                            anthropic_thinking_budget:
                              settings?.anthropic_thinking_budget ?? null,
                            llm_system_prompt: null,
                          });
                        }}
                        withCheckIcon={false}
                        disabled={presets.length === 0}
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

                    {routerStrategyValue === "embeddings" ? (
                      <>
                        <Text size="xs" c="dimmed">
                          Uses your{" "}
                          {embeddingProviderValue === "cohere"
                            ? "Cohere"
                            : "OpenAI"}{" "}
                          API key. Configure it in API Keys.
                        </Text>

                        <div className="settings-row">
                          <div>
                            <p className="settings-label">Embedding provider</p>
                            <p className="settings-description">
                              Provider used to embed the transcript and hints.
                            </p>
                          </div>
                          <Select
                            data={[
                              { value: "openai", label: "OpenAI" },
                              { value: "cohere", label: "Cohere" },
                            ]}
                            value={embeddingProviderValue}
                            onChange={(value) => {
                              if (!value) return;
                              const models = EMBEDDING_MODELS[value] ?? [];
                              const nextModel = models[0]?.value ?? null;
                              const next = normalizeRouter(
                                activeProfile.router
                              );
                              saveRouter({
                                ...next,
                                enabled: true,
                                strategy: "embeddings",
                                embedding_provider: value as any,
                                embedding_model: nextModel,
                              });
                            }}
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

                        <div className="settings-row">
                          <div>
                            <p className="settings-label">Embedding model</p>
                            <p className="settings-description">
                              Model used to embed the transcript and hints.
                            </p>
                          </div>
                          <Select
                            data={embeddingModels}
                            value={embeddingModelValue}
                            onChange={(value) => {
                              if (!value) return;
                              const next = normalizeRouter(
                                activeProfile.router
                              );
                              saveRouter({
                                ...next,
                                enabled: true,
                                strategy: "embeddings",
                                embedding_provider:
                                  embeddingProviderValue as any,
                                embedding_model: value,
                              });
                            }}
                            withCheckIcon={false}
                            styles={{
                              input: {
                                backgroundColor: "var(--bg-elevated)",
                                borderColor: "var(--border-default)",
                                color: "var(--text-primary)",
                                minWidth: 240,
                              },
                            }}
                          />
                        </div>

                        <div className="settings-row">
                          <div>
                            <p className="settings-label">Pick highest score</p>
                            <p className="settings-description">
                              Always selects the candidate with the highest
                              similarity score. Disables threshold + margin.
                            </p>
                          </div>
                          <Switch
                            checked={Boolean(
                              effectiveRouter?.pick_highest_score
                            )}
                            onChange={(e) => {
                              const enabled = e.currentTarget.checked;
                              const next = normalizeRouter(
                                activeProfile.router
                              );
                              saveRouter({
                                ...next,
                                enabled: true,
                                strategy: "embeddings",
                                pick_highest_score: enabled,
                              });
                            }}
                            color="gray"
                            size="md"
                            disabled={presets.length === 0}
                          />
                        </div>

                        <div className="settings-row">
                          <div>
                            <p className="settings-label">
                              Store preset embeddings
                            </p>
                            <p className="settings-description">
                              Precompute and store embeddings for preset hints
                              so routing doesn’t re-embed them every run.
                            </p>
                          </div>
                          <Button
                            color="gray"
                            loading={isCachingRouterEmbeddings}
                            disabled={
                              isCachingRouterEmbeddings ||
                              presets.length === 0 ||
                              activeProfileId === "default" ||
                              !embeddingModelValue
                            }
                            onClick={async () => {
                              if (activeProfileId === "default") return;
                              setIsCachingRouterEmbeddings(true);
                              try {
                                const res =
                                  await tauriAPI.cacheRouterEmbeddings({
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
                                  message: errorToMessage(e),
                                  color: "red",
                                });
                              } finally {
                                setIsCachingRouterEmbeddings(false);
                              }
                            }}
                          >
                            Store embeddings
                          </Button>
                        </div>

                        <div className="settings-row">
                          <div>
                            <p className="settings-label">
                              Similarity threshold
                            </p>
                            <p className="settings-description">
                              Minimum cosine similarity to accept a match.
                            </p>
                          </div>
                          <NumberInput
                            value={
                              effectiveRouter?.similarity_threshold ?? 0.78
                            }
                            onChange={(value) => {
                              if (
                                typeof value !== "number" ||
                                Number.isNaN(value)
                              )
                                return;
                              const next = normalizeRouter(
                                activeProfile.router
                              );
                              saveRouter({
                                ...next,
                                enabled: true,
                                strategy: "embeddings",
                                similarity_threshold: value,
                              });
                            }}
                            min={0}
                            max={1}
                            step={0.01}
                            clampBehavior="blur"
                            disabled={Boolean(
                              effectiveRouter?.pick_highest_score
                            )}
                            styles={{
                              input: {
                                backgroundColor: "var(--bg-elevated)",
                                borderColor: "var(--border-default)",
                                color: "var(--text-primary)",
                                width: 140,
                              },
                            }}
                          />
                        </div>

                        <div className="settings-row no-divider">
                          <div>
                            <p className="settings-label">Similarity margin</p>
                            <p className="settings-description">
                              Required gap between the best and second-best
                              preset.
                            </p>
                          </div>
                          <NumberInput
                            value={effectiveRouter?.similarity_margin ?? 0.05}
                            onChange={(value) => {
                              if (
                                typeof value !== "number" ||
                                Number.isNaN(value)
                              )
                                return;
                              const next = normalizeRouter(
                                activeProfile.router
                              );
                              saveRouter({
                                ...next,
                                enabled: true,
                                strategy: "embeddings",
                                similarity_margin: value,
                              });
                            }}
                            min={0}
                            max={1}
                            step={0.01}
                            clampBehavior="blur"
                            disabled={Boolean(
                              effectiveRouter?.pick_highest_score
                            )}
                            styles={{
                              input: {
                                backgroundColor: "var(--bg-elevated)",
                                borderColor: "var(--border-default)",
                                color: "var(--text-primary)",
                                width: 140,
                              },
                            }}
                          />
                        </div>
                      </>
                    ) : null}

                    {routerStrategyValue === "llm"
                      ? (() => {
                          const routerProvider =
                            effectiveRouter?.llm_provider ??
                            settings?.llm_provider ??
                            "openai";
                          const modelOptions =
                            getLlmModelOptionsForProvider(routerProvider);
                          const routerModel =
                            effectiveRouter?.llm_model ??
                            settings?.llm_model ??
                            modelOptions[0]?.value ??
                            null;

                          const supportsRouterOpenAiThinking =
                            routerProvider === "openai" &&
                            !!routerModel &&
                            (routerModel.startsWith("gpt-5") ||
                              routerModel.startsWith("o"));

                          const routerOpenAiThinkingOptions =
                            !supportsRouterOpenAiThinking || !routerModel
                              ? []
                              : [
                                  {
                                    value: SELECT_DEFAULT,
                                    label: "Default",
                                  },
                                  ...openAiThinkingEffortsForModel(
                                    routerModel
                                  ).map((v) => ({
                                    value: v,
                                    label:
                                      v === "none"
                                        ? "None"
                                        : v.charAt(0).toUpperCase() +
                                          v.slice(1),
                                  })),
                                ];

                          const supportsRouterGeminiThinkingLevel =
                            routerProvider === "gemini" &&
                            !!routerModel &&
                            routerModel.includes("gemini-3");

                          const routerIsGemini3Flash =
                            supportsRouterGeminiThinkingLevel &&
                            routerModel.includes("gemini-3-flash");

                          const routerIsGemini3Pro =
                            supportsRouterGeminiThinkingLevel &&
                            routerModel.includes("gemini-3-pro");

                          const routerGeminiThinkingLevelOptions =
                            !supportsRouterGeminiThinkingLevel
                              ? []
                              : routerIsGemini3Flash
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

                          const supportsRouterGeminiThinkingBudget =
                            routerProvider === "gemini" &&
                            !!routerModel &&
                            routerModel.includes("gemini-2.5") &&
                            !routerModel.includes("flash-lite");

                          const routerCanDisableGemini25Thinking =
                            supportsRouterGeminiThinkingBudget &&
                            routerModel.includes("gemini-2.5-flash") &&
                            !routerModel.includes("gemini-2.5-pro");

                          const routerIsGemini25Pro =
                            supportsRouterGeminiThinkingBudget &&
                            routerModel.includes("gemini-2.5-pro");

                          const routerGemini25MaxBudget = routerIsGemini25Pro
                            ? 32768
                            : 24576;

                          const routerGemini25MinBudget = routerIsGemini25Pro
                            ? 128
                            : 0;

                          const routerGeminiThinkingBudgetOptions: Array<{
                            value: string;
                            label: string;
                          }> = !supportsRouterGeminiThinkingBudget
                            ? []
                            : [
                                { value: SELECT_DEFAULT, label: "Default" },
                                { value: "-1", label: "Dynamic (-1)" },
                                ...(routerCanDisableGemini25Thinking
                                  ? [{ value: "0", label: "Off (0)" }]
                                  : []),
                                ...(routerIsGemini25Pro
                                  ? [
                                      {
                                        value: String(routerGemini25MinBudget),
                                        label: "Minimal (128)",
                                      },
                                    ]
                                  : []),
                                { value: "1024", label: "Light (1024)" },
                                { value: "4096", label: "Medium (4096)" },
                                { value: "16384", label: "High (16384)" },
                                ...(routerGemini25MaxBudget > 16384
                                  ? [
                                      {
                                        value: String(routerGemini25MaxBudget),
                                        label: `Max (${routerGemini25MaxBudget})`,
                                      },
                                    ]
                                  : []),
                              ];

                          const supportsRouterAnthropicThinkingBudget =
                            routerProvider === "anthropic" &&
                            !!routerModel &&
                            // Extended thinking is supported by newer Claude families. Keep conservative.
                            (routerModel.includes("claude-3-7") ||
                              routerModel.includes("claude-4") ||
                              routerModel.includes("-4-"));

                          const routerAnthropicThinkingLevelOptions: Array<{
                            value: string;
                            label: string;
                          }> = !supportsRouterAnthropicThinkingBudget
                            ? []
                            : [
                                { value: SELECT_DEFAULT, label: "Default" },
                                { value: "0", label: "Off" },
                                {
                                  value: String(
                                    ANTHROPIC_THINKING_LEVEL_BUDGETS[0]
                                  ),
                                  label: "Low",
                                },
                                {
                                  value: String(
                                    ANTHROPIC_THINKING_LEVEL_BUDGETS[1]
                                  ),
                                  label: "Medium",
                                },
                                {
                                  value: String(
                                    ANTHROPIC_THINKING_LEVEL_BUDGETS[2]
                                  ),
                                  label: "High",
                                },
                                {
                                  value: String(
                                    ANTHROPIC_THINKING_LEVEL_BUDGETS[3]
                                  ),
                                  label: "Max",
                                },
                              ];

                          const routerAnthropicThinkingLevelOptionsWithCustom =
                            (() => {
                              const vRaw =
                                effectiveRouter?.anthropic_thinking_budget;
                              const v =
                                typeof vRaw === "number" &&
                                Number.isFinite(vRaw)
                                  ? Math.trunc(vRaw)
                                  : null;
                              if (v == null)
                                return routerAnthropicThinkingLevelOptions;

                              const asString = String(v);
                              const exists =
                                routerAnthropicThinkingLevelOptions.some(
                                  (o) => o.value === asString
                                );
                              if (exists)
                                return routerAnthropicThinkingLevelOptions;

                              return [
                                ...routerAnthropicThinkingLevelOptions,
                                { value: asString, label: `Custom (${v})` },
                              ];
                            })();

                          return (
                            <>
                              <Text size="xs" c="dimmed">
                                Configure the provider/model used for routing.
                                The router uses structured output (JSON) when
                                the selected provider/model supports it.
                              </Text>

                              <div className="settings-row">
                                <div>
                                  <p className="settings-label">Provider</p>
                                  <p className="settings-description">
                                    LLM provider used only for routing.
                                  </p>
                                </div>
                                <Select
                                  data={[
                                    { value: "openai", label: "OpenAI" },
                                    { value: "gemini", label: "Gemini" },
                                    { value: "anthropic", label: "Anthropic" },
                                    { value: "groq", label: "Groq" },
                                    { value: "fireworks", label: "Fireworks" },
                                    { value: "ollama", label: "Ollama" },
                                  ]}
                                  value={routerProvider}
                                  onChange={(value) => {
                                    if (!value) return;
                                    const next = normalizeRouter(
                                      activeProfile.router
                                    );
                                    const nextModelOptions =
                                      getLlmModelOptionsForProvider(value);
                                    const nextModel =
                                      nextModelOptions[0]?.value ?? null;
                                    saveRouter({
                                      ...next,
                                      enabled: true,
                                      strategy: "llm",
                                      llm_provider: value,
                                      llm_model: nextModel,
                                    });
                                  }}
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

                              <div className="settings-row">
                                <div>
                                  <p className="settings-label">Model</p>
                                  <p className="settings-description">
                                    Model used for routing decisions.
                                  </p>
                                </div>
                                {modelOptions.length > 0 ? (
                                  <Select
                                    data={modelOptions}
                                    value={routerModel}
                                    onChange={(value) => {
                                      if (!value) return;
                                      const next = normalizeRouter(
                                        activeProfile.router
                                      );
                                      saveRouter({
                                        ...next,
                                        enabled: true,
                                        strategy: "llm",
                                        llm_provider: routerProvider,
                                        llm_model: value,
                                      });
                                    }}
                                    withCheckIcon={false}
                                    styles={{
                                      input: {
                                        backgroundColor: "var(--bg-elevated)",
                                        borderColor: "var(--border-default)",
                                        color: "var(--text-primary)",
                                        minWidth: 240,
                                      },
                                    }}
                                  />
                                ) : (
                                  <TextInput
                                    value={routerModel ?? ""}
                                    placeholder="Enter model id"
                                    onChange={(e) => {
                                      const value = e.currentTarget.value;
                                      const next = normalizeRouter(
                                        activeProfile.router
                                      );
                                      saveRouter({
                                        ...next,
                                        enabled: true,
                                        strategy: "llm",
                                        llm_provider: routerProvider,
                                        llm_model: value.trim().length
                                          ? value
                                          : null,
                                      });
                                    }}
                                    styles={{
                                      input: {
                                        backgroundColor: "var(--bg-elevated)",
                                        borderColor: "var(--border-default)",
                                        color: "var(--text-primary)",
                                        minWidth: 240,
                                      },
                                    }}
                                  />
                                )}
                              </div>

                              {supportsRouterOpenAiThinking ? (
                                <div className="settings-row">
                                  <div>
                                    <p className="settings-label">Thinking</p>
                                    <p className="settings-description">
                                      Reasoning effort for supported OpenAI
                                      models.
                                    </p>
                                  </div>
                                  <HintSelect
                                    data={routerOpenAiThinkingOptions}
                                    value={
                                      effectiveRouter?.openai_reasoning_effort ??
                                      SELECT_DEFAULT
                                    }
                                    onChange={(value) => {
                                      const next = normalizeRouter(
                                        activeProfile.router
                                      );
                                      if (
                                        value == null ||
                                        value === SELECT_DEFAULT
                                      ) {
                                        saveRouter({
                                          ...next,
                                          enabled: true,
                                          strategy: "llm",
                                          llm_provider: routerProvider,
                                          llm_model: routerModel,
                                          openai_reasoning_effort: null,
                                        });
                                        return;
                                      }

                                      saveRouter({
                                        ...next,
                                        enabled: true,
                                        strategy: "llm",
                                        llm_provider: routerProvider,
                                        llm_model: routerModel,
                                        openai_reasoning_effort: value as any,
                                      });
                                    }}
                                    placeholder="Default"
                                    inputStyle={{
                                      backgroundColor: "var(--bg-elevated)",
                                      borderColor: "var(--border-default)",
                                      color: "var(--text-primary)",
                                      minWidth: 200,
                                    }}
                                    renderSelected={({
                                      option,
                                      placeholder,
                                    }) => {
                                      if (!option) {
                                        return (
                                          <Text size="sm" c="dimmed">
                                            {placeholder}
                                          </Text>
                                        );
                                      }

                                      if (option.value !== SELECT_DEFAULT) {
                                        return (
                                          <Text size="sm">{option.label}</Text>
                                        );
                                      }

                                      const hint = routerModel
                                        ? settings?.openai_reasoning_effort ??
                                          openAiDefaultReasoningEffortForModel(
                                            routerModel
                                          )
                                        : settings?.openai_reasoning_effort ??
                                          "medium";

                                      return (
                                        <div
                                          style={{
                                            display: "flex",
                                            alignItems: "baseline",
                                            gap: 8,
                                          }}
                                        >
                                          <span style={{ fontSize: 14 }}>
                                            {option.label}
                                          </span>
                                          <span
                                            style={{
                                              fontSize: 11,
                                              color: "var(--text-muted)",
                                              opacity: 0.9,
                                              lineHeight: 1,
                                            }}
                                          >
                                            · {hint}
                                          </span>
                                        </div>
                                      );
                                    }}
                                    renderOption={({ option }) => {
                                      if (option.value !== SELECT_DEFAULT) {
                                        return (
                                          <Text size="sm">{option.label}</Text>
                                        );
                                      }

                                      const hint = routerModel
                                        ? settings?.openai_reasoning_effort ??
                                          openAiDefaultReasoningEffortForModel(
                                            routerModel
                                          )
                                        : settings?.openai_reasoning_effort ??
                                          "medium";

                                      return (
                                        <div
                                          style={{
                                            display: "flex",
                                            alignItems: "baseline",
                                            gap: 8,
                                          }}
                                        >
                                          <span style={{ fontSize: 14 }}>
                                            {option.label}
                                          </span>
                                          <span
                                            style={{
                                              fontSize: 11,
                                              color: "var(--text-muted)",
                                              opacity: 0.9,
                                              lineHeight: 1,
                                            }}
                                          >
                                            · {hint}
                                          </span>
                                        </div>
                                      );
                                    }}
                                  />
                                </div>
                              ) : null}

                              {supportsRouterGeminiThinkingLevel ? (
                                <>
                                  <div className="settings-row">
                                    <div>
                                      <p className="settings-label">
                                        Thinking level
                                      </p>
                                      <p className="settings-description">
                                        {routerIsGemini3Pro
                                          ? "Gemini 3 Pro supports low/high (default high)."
                                          : "Gemini 3 Flash supports minimal/low/medium/high (default high)."}
                                      </p>
                                    </div>
                                    <HintSelect
                                      data={routerGeminiThinkingLevelOptions}
                                      value={
                                        effectiveRouter?.gemini_thinking_level ??
                                        SELECT_DEFAULT
                                      }
                                      onChange={(value) => {
                                        const next = normalizeRouter(
                                          activeProfile.router
                                        );
                                        if (
                                          value == null ||
                                          value === SELECT_DEFAULT
                                        ) {
                                          saveRouter({
                                            ...next,
                                            enabled: true,
                                            strategy: "llm",
                                            llm_provider: routerProvider,
                                            llm_model: routerModel,
                                            gemini_thinking_level: null,
                                          });
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

                                        saveRouter({
                                          ...next,
                                          enabled: true,
                                          strategy: "llm",
                                          llm_provider: routerProvider,
                                          llm_model: routerModel,
                                          gemini_thinking_level: v,
                                        });
                                      }}
                                      placeholder="Default"
                                      inputStyle={{
                                        backgroundColor: "var(--bg-elevated)",
                                        borderColor: "var(--border-default)",
                                        color: "var(--text-primary)",
                                        minWidth: 200,
                                      }}
                                      renderSelected={({
                                        option,
                                        placeholder,
                                      }) => {
                                        if (!option) {
                                          return (
                                            <Text size="sm" c="dimmed">
                                              {placeholder}
                                            </Text>
                                          );
                                        }
                                        if (option.value !== SELECT_DEFAULT) {
                                          return (
                                            <Text size="sm">
                                              {option.label}
                                            </Text>
                                          );
                                        }

                                        const hint =
                                          settings?.gemini_thinking_level ??
                                          "high";
                                        return (
                                          <div
                                            style={{
                                              display: "flex",
                                              alignItems: "baseline",
                                              gap: 8,
                                            }}
                                          >
                                            <span style={{ fontSize: 14 }}>
                                              {option.label}
                                            </span>
                                            <span
                                              style={{
                                                fontSize: 11,
                                                color: "var(--text-muted)",
                                                opacity: 0.9,
                                                lineHeight: 1,
                                              }}
                                            >
                                              · {hint}
                                            </span>
                                          </div>
                                        );
                                      }}
                                      renderOption={({ option }) => {
                                        if (option.value !== SELECT_DEFAULT) {
                                          return (
                                            <Text size="sm">
                                              {option.label}
                                            </Text>
                                          );
                                        }

                                        const hint =
                                          settings?.gemini_thinking_level ??
                                          "high";
                                        return (
                                          <div
                                            style={{
                                              display: "flex",
                                              alignItems: "baseline",
                                              gap: 8,
                                            }}
                                          >
                                            <span style={{ fontSize: 14 }}>
                                              {option.label}
                                            </span>
                                            <span
                                              style={{
                                                fontSize: 11,
                                                color: "var(--text-muted)",
                                                opacity: 0.9,
                                                lineHeight: 1,
                                              }}
                                            >
                                              · {hint}
                                            </span>
                                          </div>
                                        );
                                      }}
                                    />
                                  </div>
                                </>
                              ) : null}

                              {supportsRouterGeminiThinkingBudget ? (
                                <div className="settings-row">
                                  <div>
                                    <p className="settings-label">
                                      Thinking budget
                                    </p>
                                    <p className="settings-description">
                                      Token budget for Gemini 2.5 thinking.
                                    </p>
                                  </div>
                                  <HintSelect
                                    data={routerGeminiThinkingBudgetOptions}
                                    value={
                                      effectiveRouter?.gemini_thinking_budget ==
                                      null
                                        ? SELECT_DEFAULT
                                        : String(
                                            effectiveRouter.gemini_thinking_budget
                                          )
                                    }
                                    onChange={(value) => {
                                      const next = normalizeRouter(
                                        activeProfile.router
                                      );

                                      if (
                                        value == null ||
                                        value === SELECT_DEFAULT
                                      ) {
                                        saveRouter({
                                          ...next,
                                          enabled: true,
                                          strategy: "llm",
                                          llm_provider: routerProvider,
                                          llm_model: routerModel,
                                          gemini_thinking_budget: null,
                                        });
                                        return;
                                      }

                                      const parsed = Number(value);
                                      if (!Number.isFinite(parsed)) return;
                                      const asInt = Math.trunc(parsed);
                                      saveRouter({
                                        ...next,
                                        enabled: true,
                                        strategy: "llm",
                                        llm_provider: routerProvider,
                                        llm_model: routerModel,
                                        gemini_thinking_budget: asInt,
                                      });
                                    }}
                                    placeholder="Default"
                                    inputStyle={{
                                      backgroundColor: "var(--bg-elevated)",
                                      borderColor: "var(--border-default)",
                                      color: "var(--text-primary)",
                                      minWidth: 200,
                                    }}
                                    renderSelected={({
                                      option,
                                      placeholder,
                                    }) => {
                                      if (!option) {
                                        return (
                                          <Text size="sm" c="dimmed">
                                            {placeholder}
                                          </Text>
                                        );
                                      }
                                      if (option.value !== SELECT_DEFAULT)
                                        return (
                                          <Text size="sm">{option.label}</Text>
                                        );

                                      const inherited =
                                        settings?.gemini_thinking_budget;
                                      const hint =
                                        inherited == null
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
                                          <span style={{ fontSize: 14 }}>
                                            {option.label}
                                          </span>
                                          <span
                                            style={{
                                              fontSize: 11,
                                              color: "var(--text-muted)",
                                              opacity: 0.9,
                                              lineHeight: 1,
                                            }}
                                          >
                                            · {hint}
                                          </span>
                                        </div>
                                      );
                                    }}
                                    renderOption={({ option }) => {
                                      if (option.value !== SELECT_DEFAULT) {
                                        return (
                                          <Text size="sm">{option.label}</Text>
                                        );
                                      }

                                      const inherited =
                                        settings?.gemini_thinking_budget;
                                      const hint =
                                        inherited == null
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
                                          <span style={{ fontSize: 14 }}>
                                            {option.label}
                                          </span>
                                          <span
                                            style={{
                                              fontSize: 11,
                                              color: "var(--text-muted)",
                                              opacity: 0.9,
                                              lineHeight: 1,
                                            }}
                                          >
                                            · {hint}
                                          </span>
                                        </div>
                                      );
                                    }}
                                  />
                                </div>
                              ) : null}

                              {supportsRouterAnthropicThinkingBudget ? (
                                <div className="settings-row">
                                  <div>
                                    <p className="settings-label">Thinking</p>
                                    <p className="settings-description">
                                      Extended thinking level for Claude models.
                                    </p>
                                  </div>
                                  <HintSelect
                                    data={
                                      routerAnthropicThinkingLevelOptionsWithCustom
                                    }
                                    value={
                                      effectiveRouter?.anthropic_thinking_budget ==
                                      null
                                        ? SELECT_DEFAULT
                                        : String(
                                            effectiveRouter.anthropic_thinking_budget
                                          )
                                    }
                                    onChange={(value) => {
                                      const next = normalizeRouter(
                                        activeProfile.router
                                      );

                                      if (
                                        value == null ||
                                        value === SELECT_DEFAULT
                                      ) {
                                        saveRouter({
                                          ...next,
                                          enabled: true,
                                          strategy: "llm",
                                          llm_provider: routerProvider,
                                          llm_model: routerModel,
                                          anthropic_thinking_budget: null,
                                        });
                                        return;
                                      }

                                      const parsed = Number(value);
                                      if (!Number.isFinite(parsed)) return;
                                      const asInt = Math.trunc(parsed);
                                      saveRouter({
                                        ...next,
                                        enabled: true,
                                        strategy: "llm",
                                        llm_provider: routerProvider,
                                        llm_model: routerModel,
                                        anthropic_thinking_budget: asInt,
                                      });
                                    }}
                                    placeholder="Default"
                                    inputStyle={{
                                      backgroundColor: "var(--bg-elevated)",
                                      borderColor: "var(--border-default)",
                                      color: "var(--text-primary)",
                                      minWidth: 200,
                                    }}
                                    renderSelected={({
                                      option,
                                      placeholder,
                                    }) => {
                                      if (!option) {
                                        return (
                                          <Text size="sm" c="dimmed">
                                            {placeholder}
                                          </Text>
                                        );
                                      }

                                      if (option.value === SELECT_DEFAULT) {
                                        const inheritedBudget =
                                          settings?.anthropic_thinking_budget;
                                        const hint =
                                          inheritedBudget == null
                                            ? "off"
                                            : formatThinkingBudgetShort(
                                                inheritedBudget
                                              );

                                        return (
                                          <div
                                            style={{
                                              display: "flex",
                                              alignItems: "baseline",
                                              gap: 8,
                                            }}
                                          >
                                            <span style={{ fontSize: 14 }}>
                                              {option.label}
                                            </span>
                                            <span
                                              style={{
                                                fontSize: 11,
                                                color: "var(--text-muted)",
                                                opacity: 0.9,
                                                lineHeight: 1,
                                              }}
                                            >
                                              · {hint}
                                            </span>
                                          </div>
                                        );
                                      }

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
                                            <Text size="sm">
                                              {option.label}
                                            </Text>
                                            {suffix && (
                                              <Text
                                                size="xs"
                                                c="dimmed"
                                                style={{ lineHeight: 1 }}
                                              >
                                                {suffix}
                                              </Text>
                                            )}
                                          </div>
                                        );
                                      }

                                      return (
                                        <Text size="sm">{option.label}</Text>
                                      );
                                    }}
                                    renderOption={({ option }) => {
                                      if (option.value === SELECT_DEFAULT) {
                                        const inheritedBudget =
                                          settings?.anthropic_thinking_budget;
                                        const hint =
                                          inheritedBudget == null
                                            ? "off"
                                            : formatThinkingBudgetShort(
                                                inheritedBudget
                                              );

                                        return (
                                          <div
                                            style={{
                                              display: "flex",
                                              alignItems: "baseline",
                                              gap: 8,
                                            }}
                                          >
                                            <span style={{ fontSize: 14 }}>
                                              {option.label}
                                            </span>
                                            <span
                                              style={{
                                                fontSize: 11,
                                                color: "var(--text-muted)",
                                                opacity: 0.9,
                                                lineHeight: 1,
                                              }}
                                            >
                                              · {hint}
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
                                            <Text
                                              size="xs"
                                              c="dimmed"
                                              style={{ lineHeight: 1 }}
                                            >
                                              {suffix}
                                            </Text>
                                          )}
                                        </div>
                                      );
                                    }}
                                  />
                                </div>
                              ) : null}

                              <div className="settings-row no-divider">
                                <div>
                                  <p className="settings-label">
                                    System prompt (advanced)
                                  </p>
                                  <p className="settings-description">
                                    Optional override for the router’s system
                                    prompt. Structured output rules are still
                                    enforced.
                                  </p>
                                </div>
                                <Textarea
                                  value={
                                    effectiveRouter?.llm_system_prompt ?? ""
                                  }
                                  placeholder="(leave empty to use default router prompt)"
                                  onChange={(e) => {
                                    const value = e.currentTarget.value;
                                    const next = normalizeRouter(
                                      activeProfile.router
                                    );
                                    saveRouter({
                                      ...next,
                                      enabled: true,
                                      strategy: "llm",
                                      llm_provider: routerProvider,
                                      llm_model: routerModel,
                                      llm_system_prompt: value.trim().length
                                        ? value
                                        : null,
                                    });
                                  }}
                                  autosize
                                  minRows={2}
                                  styles={{
                                    input: {
                                      backgroundColor: "var(--bg-elevated)",
                                      borderColor: "var(--border-default)",
                                      color: "var(--text-primary)",
                                      minWidth: 320,
                                    },
                                  }}
                                />
                              </div>
                            </>
                          );
                        })()
                      : null}
                  </div>
                </Accordion.Panel>
              </Accordion.Item>
            </Accordion>
          </div>
        </>
      ) : null}

      <div className="settings-mini-header">
        <span className="settings-mini-header__text">Quick Replace</span>
      </div>

      <div className="settings-row">
        <div>
          <p className="settings-label">Quick Replace</p>
          <p className="settings-description">
            If you have text highlighted when transcription starts, Kolboo will
            copy the selection, treat your transcript as instructions, rewrite
            the selected text with an LLM, then output using your output mode
            (Paste replaces the selection).
          </p>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {!isDefaultScope && quickReplaceEnabledInheriting && (
            <Tooltip label={INHERIT_TOOLTIP} withArrow>
              <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
            </Tooltip>
          )}
          {!isDefaultScope && !quickReplaceEnabledInheriting && (
            <Tooltip label="Disable override (inherit from Default)" withArrow>
              <ActionIcon
                variant="subtle"
                color="gray"
                size="sm"
                onClick={() =>
                  openDisableOverrideDialog({
                    title: "Disable Quick Replace override?",
                    onConfirm: () => {
                      setQuickReplaceEnabledInheriting(true);
                      setLocalProfileQuickReplaceEnabled(
                        defaultQuickReplaceEnabled
                      );
                      saveProfileMetadata({ quick_replace_enabled: null });
                    },
                  })
                }
              >
                <RotateCcw size={14} style={{ opacity: 0.65 }} />
              </ActionIcon>
            </Tooltip>
          )}
          <Switch
            checked={localProfileQuickReplaceEnabled}
            onChange={(e) => {
              const enabled = e.currentTarget.checked;
              if (!isDefaultScope) setQuickReplaceEnabledInheriting(false);
              setLocalProfileQuickReplaceEnabled(enabled);
              saveProfileMetadata({ quick_replace_enabled: enabled });
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
            optional context during Quick Replace.
          </p>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {!isDefaultScope && quickReplaceIncludeClipboardContextInheriting && (
            <Tooltip label={INHERIT_TOOLTIP} withArrow>
              <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
            </Tooltip>
          )}
          {!isDefaultScope &&
            !quickReplaceIncludeClipboardContextInheriting && (
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
                      title:
                        "Disable Quick Replace Clipboard Context override?",
                      onConfirm: () => {
                        setQuickReplaceIncludeClipboardContextInheriting(true);
                        setLocalProfileQuickReplaceIncludeClipboardContext(
                          defaultQuickReplaceIncludeClipboardContext
                        );
                        saveProfileMetadata({
                          quick_replace_include_clipboard_context: null,
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
            checked={localProfileQuickReplaceIncludeClipboardContext}
            onChange={(e) => {
              const enabled = e.currentTarget.checked;
              if (!isDefaultScope) {
                setQuickReplaceIncludeClipboardContextInheriting(false);
              }
              setLocalProfileQuickReplaceIncludeClipboardContext(enabled);
              saveProfileMetadata({
                quick_replace_include_clipboard_context: enabled,
              });
            }}
            color="gray"
            size="md"
          />
        </div>
      </div>

      <div className="settings-row">
        <div>
          <p className="settings-label">Provider</p>
          <p className="settings-description">
            AI service used to rewrite the highlighted text.
          </p>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {!isDefaultScope && quickReplaceProviderInheriting && (
            <Tooltip label={INHERIT_TOOLTIP} withArrow>
              <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
            </Tooltip>
          )}
          {isDefaultScope &&
            (activeProfile?.quick_replace_provider != null ||
              activeProfile?.quick_replace_model != null) && (
              <Tooltip label="Use Rewrite provider/model" withArrow>
                <ActionIcon
                  variant="subtle"
                  color="gray"
                  size="sm"
                  onClick={() => {
                    setLocalProfileQuickReplaceProvider(
                      settings?.llm_provider ?? null
                    );
                    setLocalProfileQuickReplaceModel(
                      settings?.llm_model ?? null
                    );
                    saveProfileMetadata({
                      quick_replace_provider: null,
                      quick_replace_model: null,
                    });
                  }}
                >
                  <RotateCcw size={14} style={{ opacity: 0.65 }} />
                </ActionIcon>
              </Tooltip>
            )}
          {!isDefaultScope && !quickReplaceProviderInheriting && (
            <Tooltip label="Disable override (inherit from Default)" withArrow>
              <ActionIcon
                variant="subtle"
                color="gray"
                size="sm"
                onClick={() =>
                  openDisableOverrideDialog({
                    title: "Disable Quick Replace Provider override?",
                    onConfirm: () => {
                      setQuickReplaceProviderInheriting(true);
                      setQuickReplaceModelInheriting(true);
                      setLocalProfileQuickReplaceProvider(
                        defaultQuickReplaceProvider
                      );
                      setLocalProfileQuickReplaceModel(
                        defaultQuickReplaceModel
                      );
                      saveProfileMetadata({
                        quick_replace_provider: null,
                        quick_replace_model: null,
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
            value={effectiveQuickReplaceProvider}
            onChange={(value) => {
              if (!value) return;

              if (!isDefaultScope) {
                setQuickReplaceProviderInheriting(false);
                setQuickReplaceModelInheriting(false);
              }

              setLocalProfileQuickReplaceProvider(value);
              const models = getLlmModelOptionsForProvider(value);
              const firstModel = models[0]?.value ?? null;
              setLocalProfileQuickReplaceModel(firstModel);
              saveProfileMetadata({
                quick_replace_provider: value,
                quick_replace_model: firstModel,
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

      {quickReplaceModelOptions.length > 0 ? (
        <div className="settings-row">
          <div>
            <p className="settings-label">Model</p>
            <p className="settings-description">
              LLM model used to rewrite the highlighted text.
            </p>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            {!isDefaultScope && quickReplaceModelInheriting && (
              <Tooltip label={INHERIT_TOOLTIP} withArrow>
                <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
              </Tooltip>
            )}
            {!isDefaultScope && !quickReplaceModelInheriting && (
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
                      title: "Disable Quick Replace Model override?",
                      onConfirm: () => {
                        setQuickReplaceModelInheriting(true);
                        setLocalProfileQuickReplaceModel(
                          defaultQuickReplaceModel
                        );
                        saveProfileMetadata({ quick_replace_model: null });
                      },
                    })
                  }
                >
                  <RotateCcw size={14} style={{ opacity: 0.65 }} />
                </ActionIcon>
              </Tooltip>
            )}
            <Select
              data={quickReplaceModelOptions}
              value={selectedQuickReplaceModelForUi}
              onChange={(value) => {
                if (!value) return;
                if (!isDefaultScope) setQuickReplaceModelInheriting(false);
                setLocalProfileQuickReplaceModel(value);
                saveProfileMetadata({ quick_replace_model: value });
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

      <div
        className="settings-accordion-block"
        style={{ marginTop: 0, marginBottom: 16 }}
      >
        <Accordion variant="separated" radius="md">
          <PromptSectionEditor
            sectionKey={`${activeProfileId}-quick-replace-system-prompt`}
            title="System Prompt"
            description="Optional instructions that apply to all Quick Replace rewrites."
            enabled={true}
            hideToggle={true}
            placeholder="(leave empty to use the default prompt)"
            initialContent={localQuickReplaceSystemPrompt}
            defaultContent={
              isDefaultScope
                ? DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT
                : defaultQuickReplaceSystemPrompt
            }
            hasCustom={
              isDefaultScope
                ? (() => {
                    const stored = activeProfile?.quick_replace_system_prompt;
                    if (stored == null) return false;
                    return stored !== DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT;
                  })()
                : activeProfile?.quick_replace_system_prompt !== null &&
                  activeProfile?.quick_replace_system_prompt !== undefined
            }
            inheritMode={
              isDefaultScope
                ? null
                : quickReplaceSystemPromptInheriting
                ? "inheriting"
                : "overriding"
            }
            inheritTooltip={INHERIT_TOOLTIP}
            disableOverrideTooltip="Disable override (inherit from Default)"
            onDisableOverride={
              isDefaultScope
                ? undefined
                : () =>
                    openDisableOverrideDialog({
                      title: "Disable Quick Replace System Prompt override?",
                      onConfirm: () => {
                        setQuickReplaceSystemPromptInheriting(true);
                        setLocalQuickReplaceSystemPrompt(
                          defaultQuickReplaceSystemPrompt
                        );
                        saveProfileMetadata({
                          quick_replace_system_prompt: null,
                        });
                      },
                    })
            }
            onToggle={() => {}}
            onSave={(content) => {
              if (isDefaultScope) {
                const normalized = content.trim();
                const toStore: string | null =
                  normalized.length > 0 &&
                  content !== DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT
                    ? content
                    : null;

                const nextLocal =
                  toStore == null
                    ? DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT
                    : content;

                setLocalQuickReplaceSystemPrompt(nextLocal);
                saveProfileMetadata({ quick_replace_system_prompt: toStore });
                return;
              }

              const base = defaultQuickReplaceSystemPrompt;
              const toStore = content === base ? null : content;
              const nextLocal = toStore == null ? base : content;

              setLocalQuickReplaceSystemPrompt(nextLocal);
              setQuickReplaceSystemPromptInheriting(toStore == null);
              saveProfileMetadata({ quick_replace_system_prompt: toStore });
            }}
            onReset={() => {
              if (isDefaultScope) {
                setLocalQuickReplaceSystemPrompt(
                  DEFAULT_QUICK_REPLACE_SYSTEM_PROMPT
                );
                saveProfileMetadata({ quick_replace_system_prompt: null });
                return;
              }

              const base = defaultQuickReplaceSystemPrompt;
              setLocalQuickReplaceSystemPrompt(base);
              setQuickReplaceSystemPromptInheriting(true);
              saveProfileMetadata({ quick_replace_system_prompt: null });
            }}
            isSaving={updateRewriteProgramPromptProfiles.isPending}
          />
        </Accordion>
      </div>

      <div className="settings-mini-header">
        <span className="settings-mini-header__text">Quick Ask</span>
      </div>

      <div className="settings-row">
        <div>
          <p className="settings-label">Include Highlighted Text</p>
          <p className="settings-description">
            When enabled, Kolboo will try to copy your highlighted text and
            include it as optional context during Quick Ask.
          </p>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          {!isDefaultScope && (
            <Tooltip label="Global setting (edit in Default profile)" withArrow>
              <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
            </Tooltip>
          )}

          <Switch
            checked={quickAskIncludeSelectedText}
            onChange={(e) => {
              if (!isDefaultScope) return;
              const enabled = e.currentTarget.checked;
              updateQuickAskIncludeSelectedText.mutate(enabled, {
                onSuccess: () => {
                  tauriAPI.emitSettingsChanged();
                },
              });
            }}
            color="gray"
            size="md"
            disabled={!isDefaultScope}
          />
        </div>
      </div>

      <div className="settings-row">
        <div>
          <p className="settings-label">Include Clipboard Context</p>
          <p className="settings-description">
            When enabled, Kolboo reads your clipboard text and includes it as
            optional context during Quick Ask.
          </p>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {!isDefaultScope && quickAskIncludeClipboardContextInheriting && (
            <Tooltip label={INHERIT_TOOLTIP} withArrow>
              <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
            </Tooltip>
          )}
          {!isDefaultScope && !quickAskIncludeClipboardContextInheriting && (
            <Tooltip label="Disable override (inherit from Default)" withArrow>
              <ActionIcon
                variant="subtle"
                color="gray"
                size="sm"
                onClick={() =>
                  openDisableOverrideDialog({
                    title: "Disable Quick Ask Clipboard Context override?",
                    onConfirm: () => {
                      setQuickAskIncludeClipboardContextInheriting(true);
                      setLocalProfileQuickAskIncludeClipboardContext(
                        defaultQuickAskIncludeClipboardContext
                      );
                      saveProfileMetadata({
                        quick_ask_include_clipboard_context: null,
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
            checked={localProfileQuickAskIncludeClipboardContext}
            onChange={(e) => {
              const enabled = e.currentTarget.checked;
              if (!isDefaultScope) {
                setQuickAskIncludeClipboardContextInheriting(false);
              }
              setLocalProfileQuickAskIncludeClipboardContext(enabled);
              saveProfileMetadata({
                quick_ask_include_clipboard_context: enabled,
              });
            }}
            color="gray"
            size="md"
          />
        </div>
      </div>

      <div className="settings-row">
        <div>
          <p className="settings-label">Conversation History</p>
          <p className="settings-description">
            When enabled, Quick Ask will include the last few Quick Ask questions
            and answers as additional context. This is kept in memory only (not
            saved to disk).
          </p>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          {!isDefaultScope && (
            <Tooltip
              label="Global setting (edit in Default profile)"
              withArrow
            >
              <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
            </Tooltip>
          )}

          {quickAskConversationHistoryEnabled && (
            <NumberInput
              value={quickAskConversationHistoryCount}
              onChange={(v) => {
                if (!isDefaultScope) return;
                const n = typeof v === "number" && Number.isFinite(v) ? v : 3;
                updateQuickAskConversationHistoryCount.mutate(n, {
                  onSuccess: () => {
                    tauriAPI.emitSettingsChanged();
                  },
                });
              }}
              min={1}
              max={20}
              step={1}
              w={96}
              disabled={!isDefaultScope}
              styles={{
                input: {
                  backgroundColor: "var(--bg-elevated)",
                  borderColor: "var(--border-default)",
                  color: "var(--text-primary)",
                  textAlign: "center",
                },
              }}
            />
          )}

          <Switch
            checked={quickAskConversationHistoryEnabled}
            onChange={(e) => {
              if (!isDefaultScope) return;
              const enabled = e.currentTarget.checked;
              updateQuickAskConversationHistoryEnabled.mutate(enabled, {
                onSuccess: () => {
                  tauriAPI.emitSettingsChanged();
                  // If enabling and count is missing/invalid, nudge it to default 3.
                  if (enabled) {
                    updateQuickAskConversationHistoryCount.mutate(
                      quickAskConversationHistoryCount,
                      {
                        onSuccess: () => {
                          tauriAPI.emitSettingsChanged();
                        },
                      }
                    );
                  }
                },
              });
            }}
            color="gray"
            size="md"
            disabled={!isDefaultScope}
          />
        </div>
      </div>

      <div className="settings-row">
        <div>
          <p className="settings-label">Quick Ask Provider</p>
          <p className="settings-description">
            AI service used to answer Quick Ask questions
          </p>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {!isDefaultScope && quickAskProviderInheriting && (
            <Tooltip label={INHERIT_TOOLTIP} withArrow>
              <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
            </Tooltip>
          )}
          {!isDefaultScope && !quickAskProviderInheriting && (
            <Tooltip label="Disable override (inherit from Default)" withArrow>
              <ActionIcon
                variant="subtle"
                color="gray"
                size="sm"
                onClick={() =>
                  openDisableOverrideDialog({
                    title: "Disable Quick Ask Provider override?",
                    onConfirm: () => {
                      setQuickAskProviderInheriting(true);
                      setQuickAskModelInheriting(true);
                      setLocalProfileQuickAskProvider(
                        settings?.quick_ask_provider ??
                          settings?.llm_provider ??
                          null
                      );
                      setLocalProfileQuickAskModel(
                        settings?.quick_ask_model ?? settings?.llm_model ?? null
                      );
                      saveProfileMetadata({
                        quick_ask_provider: null,
                        quick_ask_model: null,
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
            value={effectiveQuickAskProvider}
            onChange={(value) => {
              if (!value) return;
              if (isDefaultScope) {
                handleDefaultQuickAskProviderChange(value);
                return;
              }

              setQuickAskProviderInheriting(false);
              setQuickAskModelInheriting(false);
              setLocalProfileQuickAskProvider(value);
              const models = getLlmModelOptionsForProvider(value);
              const firstModel = models[0]?.value ?? null;
              setLocalProfileQuickAskModel(firstModel);
              saveProfileMetadata({
                quick_ask_provider: value,
                quick_ask_model: firstModel,
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

      {quickAskModelOptions.length > 0 ? (
        <div className="settings-row">
          <div>
            <p className="settings-label">Quick Ask Model</p>
            <p className="settings-description">
              LLM model used to answer Quick Ask questions.
            </p>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            {!isDefaultScope && quickAskModelInheriting && (
              <Tooltip label={INHERIT_TOOLTIP} withArrow>
                <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
              </Tooltip>
            )}
            {!isDefaultScope && !quickAskModelInheriting && (
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
                      title: "Disable Quick Ask Model override?",
                      onConfirm: () => {
                        setQuickAskModelInheriting(true);
                        setLocalProfileQuickAskModel(
                          settings?.quick_ask_model ??
                            settings?.llm_model ??
                            null
                        );
                        saveProfileMetadata({ quick_ask_model: null });
                      },
                    })
                  }
                >
                  <RotateCcw size={14} style={{ opacity: 0.65 }} />
                </ActionIcon>
              </Tooltip>
            )}
            <Select
              data={quickAskModelOptions}
              value={selectedQuickAskModelForUi}
              onChange={(value) => {
                if (!value) return;
                if (isDefaultScope) {
                  handleDefaultQuickAskModelChange(value);
                  return;
                }

                setQuickAskModelInheriting(false);
                setLocalProfileQuickAskModel(value);
                saveProfileMetadata({ quick_ask_model: value });
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

      {supportsQuickAskOpenAiThinking && (
        <div className="settings-row">
          <div>
            <p className="settings-label">Thinking</p>
            <p className="settings-description">
              Set the reasoning effort for this model.
            </p>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            {!isDefaultScope && quickAskOpenAiReasoningEffortInheriting && (
              <Tooltip label={INHERIT_TOOLTIP} withArrow>
                <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
              </Tooltip>
            )}
            {!isDefaultScope && !quickAskOpenAiReasoningEffortInheriting && (
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
                        setQuickAskOpenAiReasoningEffortInheriting(true);
                        setLocalProfileQuickAskOpenAiReasoningEffort(
                          SELECT_DEFAULT
                        );
                        saveProfileMetadata({
                          quick_ask_openai_reasoning_effort: null,
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
              data={quickAskOpenAiThinkingOptions}
              value={
                isDefaultScope
                  ? settings?.quick_ask_openai_reasoning_effort ??
                    SELECT_DEFAULT
                  : localProfileQuickAskOpenAiReasoningEffort
              }
              onChange={(value) => {
                if (isDefaultScope) {
                  if (value == null || value === SELECT_DEFAULT) {
                    updateQuickAskOpenAiReasoningEffort.mutate(null, {
                      onSuccess: () => {
                        tauriAPI.emitSettingsChanged();
                      },
                    });
                    return;
                  }

                  updateQuickAskOpenAiReasoningEffort.mutate(value as any, {
                    onSuccess: () => {
                      tauriAPI.emitSettingsChanged();
                    },
                  });
                  return;
                }

                if (value == null || value === SELECT_DEFAULT) {
                  setQuickAskOpenAiReasoningEffortInheriting(true);
                  setLocalProfileQuickAskOpenAiReasoningEffort(SELECT_DEFAULT);
                  saveProfileMetadata({
                    quick_ask_openai_reasoning_effort: null,
                  });
                  return;
                }

                setQuickAskOpenAiReasoningEffortInheriting(false);
                setLocalProfileQuickAskOpenAiReasoningEffort(value);
                saveProfileMetadata({
                  quick_ask_openai_reasoning_effort: value as any,
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

                const modelHint = quickAskModelForThinking
                  ? openAiDefaultReasoningEffortForModel(
                      quickAskModelForThinking
                    )
                  : "medium";
                const hint = isDefaultScope
                  ? modelHint
                  : settings?.quick_ask_openai_reasoning_effort ?? modelHint;

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
                      · {hint}
                    </span>
                  </div>
                );
              }}
              renderOption={({ option }) => {
                if (option.value !== SELECT_DEFAULT) {
                  return <Text size="sm">{option.label}</Text>;
                }

                const modelHint = quickAskModelForThinking
                  ? openAiDefaultReasoningEffortForModel(
                      quickAskModelForThinking
                    )
                  : "medium";
                const hint = isDefaultScope
                  ? modelHint
                  : settings?.quick_ask_openai_reasoning_effort ?? modelHint;

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
                      · {hint}
                    </span>
                  </div>
                );
              }}
            />
          </div>
        </div>
      )}

      {supportsQuickAskGeminiThinkingLevel && (
        <div className="settings-row">
          <div>
            <p className="settings-label">Thinking Level</p>
            <p className="settings-description">
              {quickAskModelForThinking?.includes("gemini-3-pro")
                ? "Gemini 3 Pro supports low/high (default high)."
                : "Gemini 3 Flash supports minimal/low/medium/high (default high)."}
            </p>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            {!isDefaultScope && quickAskGeminiThinkingLevelInheriting && (
              <Tooltip label={INHERIT_TOOLTIP} withArrow>
                <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
              </Tooltip>
            )}

            {!isDefaultScope && !quickAskGeminiThinkingLevelInheriting && (
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
                        setQuickAskGeminiThinkingLevelInheriting(true);
                        setLocalProfileQuickAskGeminiThinkingLevel(
                          SELECT_DEFAULT
                        );
                        saveProfileMetadata({
                          quick_ask_gemini_thinking_level: null,
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
              data={quickAskGeminiThinkingLevelOptions}
              value={
                isDefaultScope
                  ? settings?.quick_ask_gemini_thinking_level ?? SELECT_DEFAULT
                  : localProfileQuickAskGeminiThinkingLevel
              }
              onChange={(value) => {
                const v =
                  value === "minimal" ||
                  value === "low" ||
                  value === "medium" ||
                  value === "high"
                    ? value
                    : null;

                if (isDefaultScope) {
                  updateQuickAskGeminiThinkingLevel.mutate(v, {
                    onSuccess: () => {
                      tauriAPI.emitSettingsChanged();
                    },
                  });
                  return;
                }

                if (value == null || value === SELECT_DEFAULT) {
                  setQuickAskGeminiThinkingLevelInheriting(true);
                  setLocalProfileQuickAskGeminiThinkingLevel(SELECT_DEFAULT);
                  saveProfileMetadata({
                    quick_ask_gemini_thinking_level: null,
                  });
                  return;
                }

                if (v == null) return;

                setQuickAskGeminiThinkingLevelInheriting(false);
                setLocalProfileQuickAskGeminiThinkingLevel(v);
                saveProfileMetadata({ quick_ask_gemini_thinking_level: v });
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
                  : settings?.quick_ask_gemini_thinking_level ?? "high";

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
                      · {hint}
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
                  : settings?.quick_ask_gemini_thinking_level ?? "high";

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
                      · {hint}
                    </span>
                  </div>
                );
              }}
            />
          </div>
        </div>
      )}

      {supportsQuickAskGeminiThinkingBudget && (
        <div className="settings-row">
          <div>
            <p className="settings-label">Thinking Budget</p>
            <p className="settings-description">
              Token budget for Gemini 2.5 thinking.
            </p>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            {!isDefaultScope && quickAskGeminiThinkingBudgetInheriting && (
              <Tooltip label={INHERIT_TOOLTIP} withArrow>
                <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
              </Tooltip>
            )}
            {!isDefaultScope && !quickAskGeminiThinkingBudgetInheriting && (
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
                        setQuickAskGeminiThinkingBudgetInheriting(true);
                        setLocalProfileQuickAskGeminiThinkingBudget(
                          SELECT_DEFAULT
                        );
                        saveProfileMetadata({
                          quick_ask_gemini_thinking_budget: null,
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
              data={quickAskGeminiThinkingBudgetOptions}
              value={
                isDefaultScope
                  ? settings?.quick_ask_gemini_thinking_budget == null
                    ? SELECT_DEFAULT
                    : String(settings.quick_ask_gemini_thinking_budget)
                  : localProfileQuickAskGeminiThinkingBudget
              }
              onChange={(value) => {
                if (isDefaultScope) {
                  if (value == null || value === SELECT_DEFAULT) {
                    updateQuickAskGeminiThinkingBudget.mutate(null, {
                      onSuccess: () => {
                        tauriAPI.emitSettingsChanged();
                      },
                    });
                    return;
                  }

                  const parsed = Number(value);
                  if (!Number.isFinite(parsed)) return;
                  updateQuickAskGeminiThinkingBudget.mutate(
                    Math.trunc(parsed),
                    {
                      onSuccess: () => {
                        tauriAPI.emitSettingsChanged();
                      },
                    }
                  );
                  return;
                }

                if (value == null || value === SELECT_DEFAULT) {
                  setQuickAskGeminiThinkingBudgetInheriting(true);
                  setLocalProfileQuickAskGeminiThinkingBudget(SELECT_DEFAULT);
                  saveProfileMetadata({
                    quick_ask_gemini_thinking_budget: null,
                  });
                  return;
                }

                const parsed = Number(value);
                if (!Number.isFinite(parsed)) return;
                const asInt = Math.trunc(parsed);
                setQuickAskGeminiThinkingBudgetInheriting(false);
                setLocalProfileQuickAskGeminiThinkingBudget(String(asInt));
                saveProfileMetadata({
                  quick_ask_gemini_thinking_budget: asInt,
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
                if (option.value !== SELECT_DEFAULT)
                  return <Text size="sm">{option.label}</Text>;

                const inherited = settings?.quick_ask_gemini_thinking_budget;
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
                      · {hint}
                    </span>
                  </div>
                );
              }}
              renderOption={({ option }) => {
                if (option.value !== SELECT_DEFAULT) {
                  return <Text size="sm">{option.label}</Text>;
                }

                const inherited = settings?.quick_ask_gemini_thinking_budget;
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
                      · {hint}
                    </span>
                  </div>
                );
              }}
            />
          </div>
        </div>
      )}

      {supportsQuickAskAnthropicThinkingBudget && (
        <div className="settings-row">
          <div>
            <p className="settings-label">Thinking</p>
            <p className="settings-description">
              Extended thinking level for Claude models.
            </p>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            {!isDefaultScope && quickAskAnthropicThinkingBudgetInheriting && (
              <Tooltip label={INHERIT_TOOLTIP} withArrow>
                <Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
              </Tooltip>
            )}
            {!isDefaultScope && !quickAskAnthropicThinkingBudgetInheriting && (
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
                        setQuickAskAnthropicThinkingBudgetInheriting(true);
                        setLocalProfileQuickAskAnthropicThinkingBudget(
                          SELECT_DEFAULT
                        );
                        saveProfileMetadata({
                          quick_ask_anthropic_thinking_budget: null,
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
              data={quickAskAnthropicThinkingLevelOptionsWithCustom}
              value={
                isDefaultScope
                  ? settings?.quick_ask_anthropic_thinking_budget == null
                    ? SELECT_DEFAULT
                    : String(settings.quick_ask_anthropic_thinking_budget)
                  : localProfileQuickAskAnthropicThinkingBudget
              }
              onChange={(value) => {
                if (isDefaultScope) {
                  if (value == null || value === SELECT_DEFAULT) {
                    updateQuickAskAnthropicThinkingBudget.mutate(null, {
                      onSuccess: () => {
                        tauriAPI.emitSettingsChanged();
                      },
                    });
                    return;
                  }

                  const parsed = Number(value);
                  if (!Number.isFinite(parsed)) return;
                  updateQuickAskAnthropicThinkingBudget.mutate(
                    Math.trunc(parsed),
                    {
                      onSuccess: () => {
                        tauriAPI.emitSettingsChanged();
                      },
                    }
                  );
                  return;
                }

                if (value == null || value === SELECT_DEFAULT) {
                  setQuickAskAnthropicThinkingBudgetInheriting(true);
                  setLocalProfileQuickAskAnthropicThinkingBudget(
                    SELECT_DEFAULT
                  );
                  saveProfileMetadata({
                    quick_ask_anthropic_thinking_budget: null,
                  });
                  return;
                }

                const parsed = Number(value);
                if (!Number.isFinite(parsed)) return;
                const asInt = Math.trunc(parsed);
                setQuickAskAnthropicThinkingBudgetInheriting(false);
                setLocalProfileQuickAskAnthropicThinkingBudget(String(asInt));
                saveProfileMetadata({
                  quick_ask_anthropic_thinking_budget: asInt,
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

                if (option.value === SELECT_DEFAULT) {
                  const inheritedBudget =
                    settings?.quick_ask_anthropic_thinking_budget;
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
                        · {hint}
                      </span>
                    </div>
                  );
                }

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
                  const inheritedBudget =
                    settings?.quick_ask_anthropic_thinking_budget;
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
                        · {hint}
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

      <div
        className="settings-accordion-block"
        style={{ marginTop: 0, marginBottom: 16 }}
      >
        <Accordion variant="separated" radius="md">
          <PromptSectionEditor
            sectionKey={`${activeProfileId}-quick-ask-system-prompt`}
            title="System Prompt"
            description="Optional instructions that apply to all Quick Ask answers."
            enabled={true}
            hideToggle={true}
            placeholder="(leave empty to disable)"
            initialContent={localQuickAskSystemPrompt}
            defaultContent={
              isDefaultScope
                ? DEFAULT_QUICK_ASK_SYSTEM_PROMPT
                : settings?.quick_ask_system_prompt ?? ""
            }
            hasCustom={
              isDefaultScope
                ? (() => {
                    // While settings are loading, avoid flickering the reset button.
                    if (settings?.quick_ask_system_prompt === undefined)
                      return false;

                    // `null` means explicitly disabled, which is a deviation from the default.
                    if (settings?.quick_ask_system_prompt === null) return true;

                    // Otherwise, enable reset only if the stored value differs from the default.
                    return (
                      settings?.quick_ask_system_prompt !==
                      DEFAULT_QUICK_ASK_SYSTEM_PROMPT
                    );
                  })()
                : activeProfile?.quick_ask_system_prompt !== null &&
                  activeProfile?.quick_ask_system_prompt !== undefined
            }
            inheritMode={
              isDefaultScope
                ? null
                : quickAskSystemPromptInheriting
                ? "inheriting"
                : "overriding"
            }
            inheritTooltip={INHERIT_TOOLTIP}
            disableOverrideTooltip="Disable override (inherit from Default)"
            onDisableOverride={
              isDefaultScope
                ? undefined
                : () =>
                    openDisableOverrideDialog({
                      title: "Disable Quick Ask System Prompt override?",
                      onConfirm: () => {
                        setQuickAskSystemPromptInheriting(true);
                        setLocalQuickAskSystemPrompt(
                          settings?.quick_ask_system_prompt ?? ""
                        );
                        saveProfileMetadata({ quick_ask_system_prompt: null });
                      },
                    })
            }
            onToggle={() => {}}
            onSave={(content) => {
              if (isDefaultScope) {
                const normalized = content.trim();
                const toStore: string | null =
                  normalized.length > 0 ? content : null;

                // Keep the outer state in sync so the editor doesn't snap back.
                setLocalQuickAskSystemPrompt(content);

                updateQuickAskSystemPrompt.mutate(toStore, {
                  onSuccess: () => {
                    tauriAPI.emitSettingsChanged();
                  },
                });
                return;
              }

              const base = settings?.quick_ask_system_prompt ?? "";

              // If the user saves exactly the inherited value, treat it as inheriting.
              const toStore = content === base ? null : content;
              const nextLocal = toStore == null ? base : content;

              setLocalQuickAskSystemPrompt(nextLocal);
              setQuickAskSystemPromptInheriting(toStore == null);
              saveProfileMetadata({ quick_ask_system_prompt: toStore });
            }}
            onReset={() => {
              if (isDefaultScope) {
                setLocalQuickAskSystemPrompt(DEFAULT_QUICK_ASK_SYSTEM_PROMPT);
                updateQuickAskSystemPrompt.mutate(
                  DEFAULT_QUICK_ASK_SYSTEM_PROMPT,
                  {
                    onSuccess: () => {
                      tauriAPI.emitSettingsChanged();
                    },
                  }
                );
                return;
              }

              const base = settings?.quick_ask_system_prompt ?? "";
              setLocalQuickAskSystemPrompt(base);
              setQuickAskSystemPromptInheriting(true);
              saveProfileMetadata({ quick_ask_system_prompt: null });
            }}
            isSaving={
              isDefaultScope
                ? updateQuickAskSystemPrompt.isPending
                : updateRewriteProgramPromptProfiles.isPending
            }
          />

          <Accordion.Item value={`${activeProfileId}-quick-ask-test`}>
            <Accordion.Control>
              <div>
                <p className="settings-label">Test Quick Ask</p>
                <p className="settings-description">
                  Ask a question and preview the answer using the Quick Ask
                  settings above.
                </p>
              </div>
            </Accordion.Control>
            <Accordion.Panel>
              <div
                style={{ display: "flex", flexDirection: "column", gap: 10 }}
              >
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    gap: 12,
                  }}
                >
                  <Text size="sm" c="dimmed">
                    {quickAskTestPending
                      ? "Duration: running…"
                      : quickAskTestDurationMs === null
                      ? "Duration: —"
                      : `Duration: ${(quickAskTestDurationMs / 1000).toFixed(
                          2
                        )}s`}
                  </Text>

                  <Button
                    color="gray"
                    loading={quickAskTestPending}
                    disabled={
                      !effectiveQuickAskProvider || !quickAskTestInput.trim()
                    }
                    onClick={async () => {
                      if (!effectiveQuickAskProvider) return;
                      if (!quickAskTestInput.trim()) return;

                      setQuickAskTestError("");
                      setQuickAskTestOutput("");
                      setQuickAskTestDurationMs(null);
                      quickAskTestStartRef.current = performance.now();
                      setQuickAskTestPending(true);

                      const openAiReasoningEffort = isDefaultScope
                        ? settings?.quick_ask_openai_reasoning_effort ?? null
                        : localProfileQuickAskOpenAiReasoningEffort ===
                          SELECT_DEFAULT
                        ? null
                        : (localProfileQuickAskOpenAiReasoningEffort as any);

                      const geminiThinkingLevel = isDefaultScope
                        ? settings?.quick_ask_gemini_thinking_level ?? null
                        : localProfileQuickAskGeminiThinkingLevel ===
                          SELECT_DEFAULT
                        ? null
                        : (localProfileQuickAskGeminiThinkingLevel as any);

                      const geminiThinkingBudget = (() => {
                        if (isDefaultScope) {
                          return (
                            settings?.quick_ask_gemini_thinking_budget ?? null
                          );
                        }
                        if (
                          localProfileQuickAskGeminiThinkingBudget ===
                          SELECT_DEFAULT
                        )
                          return null;
                        const n = Number(
                          localProfileQuickAskGeminiThinkingBudget
                        );
                        return Number.isFinite(n) ? Math.trunc(n) : null;
                      })();

                      const anthropicThinkingBudget = (() => {
                        if (isDefaultScope) {
                          return (
                            settings?.quick_ask_anthropic_thinking_budget ??
                            null
                          );
                        }
                        if (
                          localProfileQuickAskAnthropicThinkingBudget ===
                          SELECT_DEFAULT
                        )
                          return null;
                        const n = Number(
                          localProfileQuickAskAnthropicThinkingBudget
                        );
                        return Number.isFinite(n) ? Math.trunc(n) : null;
                      })();

                      try {
                        const res = await llmAPI.complete({
                          provider: effectiveQuickAskProvider,
                          model:
                            selectedQuickAskModelForUi ??
                            effectiveQuickAskModel,
                          systemPrompt: localQuickAskSystemPrompt,
                          userPrompt: quickAskTestInput,
                          openAiReasoningEffort,
                          geminiThinkingBudget,
                          geminiThinkingLevel,
                          anthropicThinkingBudget,
                        });

                        setQuickAskTestOutput(
                          `[${res.provider_used}/${res.model_used}]\n\n${res.output}`
                        );
                      } catch (err) {
                        setQuickAskTestError(errorToMessage(err));
                      } finally {
                        const startedAt = quickAskTestStartRef.current;
                        quickAskTestStartRef.current = null;
                        if (typeof startedAt === "number") {
                          setQuickAskTestDurationMs(
                            performance.now() - startedAt
                          );
                        }
                        setQuickAskTestPending(false);
                      }
                    }}
                  >
                    Test
                  </Button>
                </div>

                <div style={{ width: "100%" }}>
                  <Textarea
                    value={quickAskTestInput}
                    onChange={(e) =>
                      setQuickAskTestInput(e.currentTarget.value)
                    }
                    placeholder="Ask a question…"
                    autosize
                    minRows={2}
                    styles={{
                      input: {
                        backgroundColor: "var(--bg-elevated)",
                        borderColor: "var(--border-default)",
                        color: "var(--text-primary)",
                        fontFamily: "monospace",
                        fontSize: "13px",
                      },
                    }}
                  />
                </div>

                <div style={{ width: "100%" }}>
                  {quickAskTestError ? (
                    <Text size="sm" c="red" style={{ marginBottom: 8 }}>
                      {quickAskTestError}
                    </Text>
                  ) : null}

                  <Textarea
                    value={quickAskTestOutput}
                    readOnly
                    placeholder="Answer will appear here"
                    autosize
                    minRows={3}
                    styles={{
                      input: {
                        backgroundColor: "var(--bg-elevated)",
                        borderColor: "var(--border-default)",
                        color: "var(--text-primary)",
                        fontFamily: "monospace",
                        fontSize: "13px",
                      },
                    }}
                  />
                </div>
              </div>
            </Accordion.Panel>
          </Accordion.Item>
        </Accordion>
      </div>
    </>
  );
}
