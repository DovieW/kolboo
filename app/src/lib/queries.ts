import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import {
  type AppSettings,
  audioSettingsTestAPI,
  type CleanupPromptSections,
  configAPI,
  dataAPI,
  type HotkeyConfig,
  type HistoryPageQuery,
  llmAPI,
  logsAPI,
  recordingsAPI,
  type SettingsGuideState,
  type AudioCue,
  type OutputMode,
  type PlayingAudioHandling,
  type RewriteProgramPromptProfile,
  type MainWindowCloseBehavior,
  sttAPI,
  tauriAPI,
  type TestLlmRewriteResponse,
  type IterateRewritePromptResponse,
  type TestRewriteWithPromptResponse,
  validateHotkeyNotDuplicate,
  type WidgetPosition,
  type CostTimeframe,
  type ModelPricingKind,
  type ProxySettings,
  type OpenAiReasoningEffort,
  type WhisperModelInfo,
  type OverlayMonitorTarget,
} from "./tauri";

export function useModelPricing(
  provider: string | null,
  kind: ModelPricingKind,
  model: string | null
) {
  return useQuery({
    queryKey: ["modelPricing", provider ?? "", kind, model ?? ""],
    enabled: Boolean(provider) && Boolean(model),
    queryFn: () =>
      tauriAPI.getModelPricing({
        provider: provider ?? "",
        kind,
        model: model ?? "",
      }),
    staleTime: Number.POSITIVE_INFINITY,
  });
}

export function useCostSummary(
  timeframe: CostTimeframe,
  filters?: {
    kind?: "all" | "stt" | "llm";
    sttModelKeys?: string[];
    llmModelKeys?: string[];
    excludeFreeTier?: boolean;
  }
) {
  const kind = filters?.kind;
  const sttModelKeys = (filters?.sttModelKeys ?? []).slice().sort();
  const llmModelKeys = (filters?.llmModelKeys ?? []).slice().sort();
  const excludeFreeTier = filters?.excludeFreeTier ?? true;

  return useQuery({
    queryKey: [
      "costSummary",
      timeframe,
      kind ?? "all",
      excludeFreeTier ? "exclude_free" : "include_free",
      sttModelKeys,
      llmModelKeys,
    ],
    queryFn: () =>
      tauriAPI.getCostSummary({
        timeframe,
        kind,
        sttModelKeys,
        llmModelKeys,
        excludeFreeTier,
      }),
    staleTime: 10_000,
    refetchOnWindowFocus: true,
  });
}

export function useCostByProvider(
  timeframe: CostTimeframe,
  filters?: {
    kind?: "all" | "stt" | "llm";
    sttModelKeys?: string[];
    llmModelKeys?: string[];
    excludeFreeTier?: boolean;
  }
) {
  const kind = filters?.kind;
  const sttModelKeys = (filters?.sttModelKeys ?? []).slice().sort();
  const llmModelKeys = (filters?.llmModelKeys ?? []).slice().sort();
  const excludeFreeTier = filters?.excludeFreeTier ?? true;

  return useQuery({
    queryKey: [
      "costByProvider",
      timeframe,
      kind ?? "all",
      excludeFreeTier ? "exclude_free" : "include_free",
      sttModelKeys,
      llmModelKeys,
    ],
    queryFn: () =>
      tauriAPI.getCostByProvider({
        timeframe,
        kind,
        sttModelKeys,
        llmModelKeys,
        excludeFreeTier,
      }),
    staleTime: 10_000,
    refetchOnWindowFocus: true,
  });
}

export function useTypeText() {
  return useMutation({
    mutationFn: (text: string) => invoke("type_text", { text }),
  });
}

export function useTestLlmRewrite() {
  return useMutation({
    mutationFn: (params: {
      transcript: string;
      profileId?: string | null;
    }): Promise<TestLlmRewriteResponse> => llmAPI.testLlmRewrite(params),
  });
}

export function useIterateRewritePrompt() {
  return useMutation({
    mutationFn: (params: {
      transcript: string;
      problemOutput: string;
      desiredOutput?: string | null;
      currentPrompt: string;
      profileId?: string | null;
      mode?: "fixed" | "new";

      llmProvider?: string | null;
      llmModel?: string | null;
      openAiReasoningEffort?: "none" | "low" | "medium" | "high" | null;
      geminiThinkingLevel?: "minimal" | "low" | "medium" | "high" | null;
      geminiThinkingBudget?: number | null;
      anthropicThinkingBudget?: number | null;
    }): Promise<IterateRewritePromptResponse> =>
      llmAPI.iterateRewritePrompt(params),
  });
}

export function useTestRewriteWithPrompt() {
  return useMutation({
    mutationFn: (params: {
      transcript: string;
      prompt: string;
      profileId?: string | null;
    }): Promise<TestRewriteWithPromptResponse> =>
      llmAPI.testRewriteWithPrompt(params),
  });
}

export function useTestSttTranscribeLastAudio() {
  return useMutation({
    mutationFn: (params: { profileId?: string | null }): Promise<string> =>
      sttAPI.testTranscribeLastAudio(params),
  });
}

export function useHasLastAudioForSttTest() {
  return useQuery({
    queryKey: ["sttLastAudioAvailable"],
    queryFn: () => sttAPI.hasLastAudio(),
    staleTime: 0,
    refetchOnWindowFocus: true,
    // Very cheap boolean check; polling keeps the UI in sync when the user
    // records audio via hotkey while the settings page is open.
    refetchInterval: 2000,
  });
}

// Settings queries and mutations
export function useSettings() {
  return useQuery({
    queryKey: ["settings"],
    queryFn: () => tauriAPI.getSettings(),
    staleTime: Number.POSITIVE_INFINITY,
  });
}

export function useSystemProxyInfo() {
  return useQuery({
    queryKey: ["systemProxyInfo"],
    queryFn: () => tauriAPI.getSystemProxyInfo(),
    staleTime: 0,
    refetchOnWindowFocus: true,
  });
}

export function useSettingsGuideState() {
  return useQuery({
    queryKey: ["settingsGuideState"],
    queryFn: () => tauriAPI.getSettingsGuideState(),
    staleTime: Number.POSITIVE_INFINITY,
  });
}

export function useSetSettingsGuideState() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (state: SettingsGuideState) =>
      tauriAPI.setSettingsGuideState(state),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settingsGuideState"] });
    },
  });
}

export function useUpdateToggleHotkey() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (hotkey: HotkeyConfig | null) => {
      // Get current settings for validation
      const settings = await tauriAPI.getSettings();
      const previous = settings.toggle_hotkey;

      // Validate no duplicate (unless unsetting)
      if (hotkey) {
        const error = validateHotkeyNotDuplicate(
          hotkey,
          {
            toggle: settings.toggle_hotkey,
            hold: settings.hold_hotkey,
            paste_last: settings.paste_last_hotkey,
            retry: settings.retry_hotkey,
            quick_ask_hold: settings.quick_ask_hold_hotkey,
            quick_ask_toggle: settings.quick_ask_toggle_hotkey,
          },
          "toggle"
        );
        if (error) throw new Error(error);
      }

      // Save and re-register
      await tauriAPI.updateToggleHotkey(hotkey);
      await tauriAPI.unregisterShortcuts();

      try {
        await tauriAPI.registerShortcuts();
      } catch (error) {
        // Defensive: don't leave the user with no registered shortcuts.
        // Revert setting and restore previous registrations.
        try {
          await tauriAPI.updateToggleHotkey(previous);
          await tauriAPI.unregisterShortcuts();
          await tauriAPI.registerShortcuts();
        } catch (restoreError) {
          console.error(
            "Failed to restore previous toggle hotkey:",
            restoreError
          );
        }
        throw error;
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateHoldHotkey() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (hotkey: HotkeyConfig | null) => {
      // Get current settings for validation
      const settings = await tauriAPI.getSettings();
      const previous = settings.hold_hotkey;

      // Validate no duplicate (unless unsetting)
      if (hotkey) {
        const error = validateHotkeyNotDuplicate(
          hotkey,
          {
            toggle: settings.toggle_hotkey,
            hold: settings.hold_hotkey,
            paste_last: settings.paste_last_hotkey,
            retry: settings.retry_hotkey,
            quick_ask_hold: settings.quick_ask_hold_hotkey,
            quick_ask_toggle: settings.quick_ask_toggle_hotkey,
          },
          "hold"
        );
        if (error) throw new Error(error);
      }

      // Save and re-register
      await tauriAPI.updateHoldHotkey(hotkey);
      await tauriAPI.unregisterShortcuts();

      try {
        await tauriAPI.registerShortcuts();
      } catch (error) {
        try {
          await tauriAPI.updateHoldHotkey(previous);
          await tauriAPI.unregisterShortcuts();
          await tauriAPI.registerShortcuts();
        } catch (restoreError) {
          console.error(
            "Failed to restore previous hold hotkey:",
            restoreError
          );
        }
        throw error;
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdatePasteLastHotkey() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (hotkey: HotkeyConfig | null) => {
      // Get current settings for validation
      const settings = await tauriAPI.getSettings();
      const previous = settings.paste_last_hotkey;

      // Validate no duplicate (unless unsetting)
      if (hotkey) {
        const error = validateHotkeyNotDuplicate(
          hotkey,
          {
            toggle: settings.toggle_hotkey,
            hold: settings.hold_hotkey,
            paste_last: settings.paste_last_hotkey,
            retry: settings.retry_hotkey,
            quick_ask_hold: settings.quick_ask_hold_hotkey,
            quick_ask_toggle: settings.quick_ask_toggle_hotkey,
          },
          "paste_last"
        );
        if (error) throw new Error(error);
      }

      // Save and re-register
      await tauriAPI.updatePasteLastHotkey(hotkey);
      await tauriAPI.unregisterShortcuts();

      try {
        await tauriAPI.registerShortcuts();
      } catch (error) {
        try {
          await tauriAPI.updatePasteLastHotkey(previous);
          await tauriAPI.unregisterShortcuts();
          await tauriAPI.registerShortcuts();
        } catch (restoreError) {
          console.error(
            "Failed to restore previous paste-last hotkey:",
            restoreError
          );
        }
        throw error;
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateRetryHotkey() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (hotkey: HotkeyConfig | null) => {
      // Get current settings for validation
      const settings = await tauriAPI.getSettings();
      const previous = settings.retry_hotkey;

      // Validate no duplicate (unless unsetting)
      if (hotkey) {
        const error = validateHotkeyNotDuplicate(
          hotkey,
          {
            toggle: settings.toggle_hotkey,
            hold: settings.hold_hotkey,
            paste_last: settings.paste_last_hotkey,
            retry: settings.retry_hotkey,
            quick_ask_hold: settings.quick_ask_hold_hotkey,
            quick_ask_toggle: settings.quick_ask_toggle_hotkey,
          },
          "retry"
        );
        if (error) throw new Error(error);
      }

      // Save and re-register
      await tauriAPI.updateRetryHotkey(hotkey);
      await tauriAPI.unregisterShortcuts();

      try {
        await tauriAPI.registerShortcuts();
      } catch (error) {
        try {
          await tauriAPI.updateRetryHotkey(previous);
          await tauriAPI.unregisterShortcuts();
          await tauriAPI.registerShortcuts();
        } catch (restoreError) {
          console.error(
            "Failed to restore previous retry hotkey:",
            restoreError
          );
        }
        throw error;
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuickAskHoldHotkey() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (hotkey: HotkeyConfig | null) => {
      // Get current settings for validation
      const settings = await tauriAPI.getSettings();
      const previous = settings.quick_ask_hold_hotkey;

      // Validate no duplicate (unless unsetting)
      if (hotkey) {
        const error = validateHotkeyNotDuplicate(
          hotkey,
          {
            toggle: settings.toggle_hotkey,
            hold: settings.hold_hotkey,
            paste_last: settings.paste_last_hotkey,
            retry: settings.retry_hotkey,
            quick_ask_hold: settings.quick_ask_hold_hotkey,
            quick_ask_toggle: settings.quick_ask_toggle_hotkey,
          },
          "quick_ask_hold"
        );
        if (error) throw new Error(error);
      }

      // Save and re-register
      await tauriAPI.updateQuickAskHoldHotkey(hotkey);
      await tauriAPI.unregisterShortcuts();

      try {
        await tauriAPI.registerShortcuts();
      } catch (error) {
        try {
          await tauriAPI.updateQuickAskHoldHotkey(previous);
          await tauriAPI.unregisterShortcuts();
          await tauriAPI.registerShortcuts();
        } catch (restoreError) {
          console.error(
            "Failed to restore previous quick ask hold hotkey:",
            restoreError
          );
        }
        throw error;
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuickAskToggleHotkey() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (hotkey: HotkeyConfig | null) => {
      // Get current settings for validation
      const settings = await tauriAPI.getSettings();
      const previous = settings.quick_ask_toggle_hotkey;

      // Validate no duplicate (unless unsetting)
      if (hotkey) {
        const error = validateHotkeyNotDuplicate(
          hotkey,
          {
            toggle: settings.toggle_hotkey,
            hold: settings.hold_hotkey,
            paste_last: settings.paste_last_hotkey,
            retry: settings.retry_hotkey,
            quick_ask_hold: settings.quick_ask_hold_hotkey,
            quick_ask_toggle: settings.quick_ask_toggle_hotkey,
          },
          "quick_ask_toggle"
        );
        if (error) throw new Error(error);
      }

      // Save and re-register
      await tauriAPI.updateQuickAskToggleHotkey(hotkey);
      await tauriAPI.unregisterShortcuts();

      try {
        await tauriAPI.registerShortcuts();
      } catch (error) {
        try {
          await tauriAPI.updateQuickAskToggleHotkey(previous);
          await tauriAPI.unregisterShortcuts();
          await tauriAPI.registerShortcuts();
        } catch (restoreError) {
          console.error(
            "Failed to restore previous quick ask toggle hotkey:",
            restoreError
          );
        }
        throw error;
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateSelectedMic() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (micId: string | null) => tauriAPI.updateSelectedMic(micId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateSoundEnabled() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (enabled: boolean) => tauriAPI.updateSoundEnabled(enabled),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateHotkeyDebugEnabled() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (enabled: boolean) => tauriAPI.updateHotkeyDebugEnabled(enabled),
    onMutate: async (enabled: boolean) => {
      await queryClient.cancelQueries({ queryKey: ["settings"] });

      const previous = queryClient.getQueryData<AppSettings>(["settings"]);
      if (previous) {
        queryClient.setQueryData<AppSettings>(["settings"], {
          ...previous,
          hotkey_debug_enabled: enabled,
        });
      }

      return { previous };
    },
    onError: (_error, _enabled, context) => {
      if (context?.previous) {
        queryClient.setQueryData<AppSettings>(["settings"], context.previous);
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateAudioCue() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (cue: AudioCue) => tauriAPI.updateAudioCue(cue),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateAccentColor() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (color: string | null) => tauriAPI.updateAccentColor(color),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (error) => {
      console.error("Update accent color failed:", error);
    },
  });
}

export function useUpdateMainWindowCloseBehavior() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (behavior: MainWindowCloseBehavior) =>
      tauriAPI.updateMainWindowCloseBehavior(behavior),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateRewriteLlmEnabled() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateRewriteLlmEnabled(enabled);
      // Gate the pipeline's LLM rewrite step immediately.
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdatePlayingAudioHandling() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (handling: PlayingAudioHandling) =>
      tauriAPI.updatePlayingAudioHandling(handling),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateOverlayMode() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (mode: "always" | "never" | "recording_only") =>
      tauriAPI.updateOverlayMode(mode),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateOverlayShowDetailedLoading() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (enabled: boolean) =>
      tauriAPI.updateOverlayShowDetailedLoading(enabled),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateOverlayMonitorTarget() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (target: OverlayMonitorTarget) =>
      tauriAPI.updateOverlayMonitorTarget(target),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateWidgetPosition() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (position: WidgetPosition) =>
      tauriAPI.updateWidgetPosition(position),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateOutputMode() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (mode: OutputMode) => tauriAPI.updateOutputMode(mode),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateOutputHitEnter() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (enabled: boolean) => tauriAPI.updateOutputHitEnter(enabled),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuietAudioGateEnabled() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateQuietAudioGateEnabled(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuietAudioMinDurationSecs() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (seconds: number) => {
      await tauriAPI.updateQuietAudioMinDurationSecs(seconds);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuietAudioRmsDbfsThreshold() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (dbfs: number) => {
      await tauriAPI.updateQuietAudioRmsDbfsThreshold(dbfs);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuietAudioPeakDbfsThreshold() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (dbfs: number) => {
      await tauriAPI.updateQuietAudioPeakDbfsThreshold(dbfs);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateNoiseGateStrength() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (strength: number) => {
      await tauriAPI.updateNoiseGateStrength(strength);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateNoiseGateThresholdDbfs() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (thresholdDbfs: number | null) => {
      await tauriAPI.updateNoiseGateThresholdDbfs(thresholdDbfs);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuietAudioRequireSpeech() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateQuietAudioRequireSpeech(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateHotMicEnabled() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateHotMicEnabled(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateHotMicPreRollMs() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (ms: number) => {
      await tauriAPI.updateHotMicPreRollMs(ms);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateMicAutoRecoverEnabled() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateMicAutoRecoverEnabled(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateAudioDownmixToMono() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateAudioDownmixToMono(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateAudioResampleTo16khz() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateAudioResampleTo16khz(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateAudioHighpassEnabled() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateAudioHighpassEnabled(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateAudioAgcEnabled() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateAudioAgcEnabled(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateAudioNoiseSuppressionEnabled() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateAudioNoiseSuppressionEnabled(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useLastRecordingDiagnostics() {
  return useQuery({
    queryKey: ["lastRecordingDiagnostics"],
    queryFn: () => sttAPI.getLastRecordingDiagnostics(),
    staleTime: 0,
    refetchOnWindowFocus: true,
    // Keep UI in sync if user records via hotkey while settings is open.
    refetchInterval: 2000,
  });
}

export function useAudioSettingsTestStartRecording() {
  return useMutation({
    mutationFn: () => audioSettingsTestAPI.startRecording(),
  });
}

export function useAudioSettingsTestStopRecording() {
  return useMutation({
    mutationFn: () => audioSettingsTestAPI.stopRecording(),
  });
}

export function useUpdateMaxSavedRecordings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (max: number) => tauriAPI.updateMaxSavedRecordings(max),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateRequestLogsRetention() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: {
      mode: "amount" | "time";
      amount: number;
      days: number;
    }) => tauriAPI.updateRequestLogsRetention(params),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
      queryClient.invalidateQueries({ queryKey: ["requestLogs"] });
    },
  });
}

export function useUpdateTranscriptionRetentionDays() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (days: number) =>
      tauriAPI.updateTranscriptionRetentionDays(days),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateTranscriptionRetention() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: { unit: "days" | "hours"; value: number }) =>
      tauriAPI.updateTranscriptionRetention(params),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateTranscriptionRetentionDeleteRecordings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (enabled: boolean) =>
      tauriAPI.updateTranscriptionRetentionDeleteRecordings(enabled),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useRecordingsStats() {
  return useQuery({
    queryKey: ["recordingsStats"],
    queryFn: () => recordingsAPI.getRecordingsStats(),
    staleTime: 0,
    refetchOnWindowFocus: true,
    refetchInterval: 10000,
  });
}

export function useDataStorageSummary() {
  return useQuery({
    queryKey: ["dataStorageSummary"],
    queryFn: () => dataAPI.getStorageSummary(),
    staleTime: 0,
    refetchOnWindowFocus: true,
    refetchInterval: 10000,
  });
}

export function useIsAudioMuteSupported() {
  return useQuery({
    queryKey: ["audioMuteSupported"],
    queryFn: () => tauriAPI.isAudioMuteSupported(),
    staleTime: Number.POSITIVE_INFINITY,
  });
}

export function useUpdateCleanupPromptSections() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (sections: CleanupPromptSections | null) => {
      await tauriAPI.updateCleanupPromptSections(sections);
      // Apply prompt changes immediately (LLM step reads prompts from pipeline config).
      await configAPI.syncPipelineConfig();
    },
    onMutate: async (sections: CleanupPromptSections | null) => {
      // Optimistically update the settings cache so the UI doesn't snap back
      // if the user navigates away before the mutation settles.
      await queryClient.cancelQueries({ queryKey: ["settings"] });

      const previousSettings = queryClient.getQueryData<AppSettings>([
        "settings",
      ]);

      if (previousSettings) {
        queryClient.setQueryData<AppSettings>(["settings"], {
          ...previousSettings,
          cleanup_prompt_sections: sections,
        });
      }

      return { previousSettings };
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (error, _sections, context) => {
      console.error("updateCleanupPromptSections failed:", error);

      // Roll back optimistic update.
      if (context?.previousSettings) {
        queryClient.setQueryData<AppSettings>(
          ["settings"],
          context.previousSettings
        );
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateRewriteProgramPromptProfiles() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (profiles: RewriteProgramPromptProfile[]) => {
      await tauriAPI.updateRewriteProgramPromptProfiles(profiles);
      await configAPI.syncPipelineConfig();
    },
    onMutate: async (nextProfiles: RewriteProgramPromptProfile[]) => {
      // Optimistically update the settings cache so toggles/selects don't
      // "snap back" while the write + pipeline sync is running.
      await queryClient.cancelQueries({ queryKey: ["settings"] });

      const previousSettings = queryClient.getQueryData<AppSettings>([
        "settings",
      ]);

      if (previousSettings) {
        queryClient.setQueryData<AppSettings>(["settings"], {
          ...previousSettings,
          rewrite_program_prompt_profiles: nextProfiles,
        });
      }

      return { previousSettings };
    },
    onError: (_error, _nextProfiles, context) => {
      console.error("updateRewriteProgramPromptProfiles failed:", _error);
      // Roll back optimistic update.
      if (context?.previousSettings) {
        queryClient.setQueryData<AppSettings>(
          ["settings"],
          context.previousSettings
        );
      }
    },
    onSettled: () => {
      // Ensure we reconcile with persisted settings.
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useResetHotkeysToDefaults() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      await tauriAPI.resetHotkeysToDefaults();
      await tauriAPI.unregisterShortcuts();
      await tauriAPI.registerShortcuts();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (error) => {
      console.error("Reset hotkeys failed:", error);
    },
  });
}

// History queries and mutations
export function useHistory(limit?: number) {
  return useQuery({
    queryKey: ["history", limit],
    queryFn: () => tauriAPI.getHistory(limit),
  });
}

// Fetch all history entries (unbounded). Intended for optional features like
// analysis where full history is required, but shouldn't load by default.
export function useHistoryAll(options?: { enabled?: boolean }) {
  return useQuery({
    queryKey: ["historyAll"],
    queryFn: () => tauriAPI.getHistory(undefined),
    enabled: options?.enabled ?? true,
  });
}

export function useHistoryPage(params: HistoryPageQuery) {
  const filterText = (params.filterText ?? "").toString();
  const showFailed = params.showFailed ?? true;
  const showEmptyTranscript = params.showEmptyTranscript ?? false;
  const selectedSttModelKeys = (params.selectedSttModelKeys ?? [])
    .slice()
    .sort();
  const selectedLlmModelKeys = (params.selectedLlmModelKeys ?? [])
    .slice()
    .sort();
  const page = params.page ?? 1;
  const pageSize = params.pageSize ?? 25;
  const includeUsageCounts = params.includeUsageCounts ?? true;

  return useQuery({
    queryKey: [
      "historyPage",
      filterText,
      showFailed,
      showEmptyTranscript,
      selectedSttModelKeys,
      selectedLlmModelKeys,
      page,
      pageSize,
      includeUsageCounts,
    ],
    queryFn: () =>
      tauriAPI.getHistoryPage({
        filterText,
        showFailed,
        showEmptyTranscript,
        selectedSttModelKeys,
        selectedLlmModelKeys,
        page,
        pageSize,
        includeUsageCounts,
      }),
    placeholderData: keepPreviousData,
    // Keep things feeling responsive while typing filters.
    refetchOnWindowFocus: true,
  });
}

export function useAddHistoryEntry() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (text: string) => tauriAPI.addHistoryEntry(text),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["history"] });
      queryClient.invalidateQueries({ queryKey: ["historyAll"] });
      queryClient.invalidateQueries({ queryKey: ["historyPage"] });
      // Notify other windows about history change
      tauriAPI.emitHistoryChanged();
    },
  });
}

export function useDeleteHistoryEntry() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => tauriAPI.deleteHistoryEntry(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["history"] });
      queryClient.invalidateQueries({ queryKey: ["historyAll"] });
      queryClient.invalidateQueries({ queryKey: ["historyPage"] });
      // Notify other windows about history change
      tauriAPI.emitHistoryChanged();
    },
  });
}

export function useClearHistory() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => tauriAPI.clearHistory(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["history"] });
      queryClient.invalidateQueries({ queryKey: ["historyAll"] });
      queryClient.invalidateQueries({ queryKey: ["historyPage"] });
      // Notify other windows about history change
      tauriAPI.emitHistoryChanged();
    },
  });
}

// Config API queries and mutations (now using Tauri commands)
export function useDefaultSections() {
  return useQuery({
    queryKey: ["defaultSections"],
    queryFn: () => configAPI.getDefaultSections(),
    staleTime: Number.POSITIVE_INFINITY, // Default prompts never change
  });
}

// Provider queries and mutations

export function useAvailableProviders() {
  return useQuery({
    queryKey: ["availableProviders"],
    queryFn: () => configAPI.getAvailableProviders(),
  });
}

export function useUpdateGroqFreeTier() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateGroqFreeTier(enabled);
      // Keep the pipeline in sync in case Groq configuration depends on this.
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateElevenLabsFreeTier() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateElevenLabsFreeTier(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateCerebrasFreeTier() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateCerebrasFreeTier(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateCohereFreeTier() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateCohereFreeTier(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateAssemblyAiFreeTier() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateAssemblyAiFreeTier(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateSpeechmaticsFreeTier() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      await tauriAPI.updateSpeechmaticsFreeTier(enabled);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateSTTProvider() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (provider: string | null) => {
      await tauriAPI.updateSTTProvider(provider);
      // Sync the pipeline configuration when STT provider changes
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateSTTModel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (model: string | null) => {
      await tauriAPI.updateSTTModel(model);
      // Sync the pipeline configuration when STT model changes
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateSTTTranscriptionPrompt() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (prompt: string | null) => {
      await tauriAPI.updateSTTTranscriptionPrompt(prompt);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateWhisperServerBaseUrl() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (baseUrl: string | null) => {
      await tauriAPI.updateWhisperServerBaseUrl(baseUrl);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateLocalWhisperModelId() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (modelId: string | null) => {
      await tauriAPI.updateLocalWhisperModelId(modelId);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateLocalWhisperLoadMode() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (mode: "manual" | "on_transcribe" | "on_launch") => {
      await tauriAPI.updateLocalWhisperLoadMode(mode);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
      queryClient.invalidateQueries({ queryKey: ["localWhisperModelLoaded"] });
    },
  });
}

export function useIsLocalWhisperAvailable() {
  return useQuery({
    queryKey: ["localWhisperAvailable"],
    queryFn: () => tauriAPI.isLocalWhisperAvailable(),
    staleTime: Number.POSITIVE_INFINITY,
  });
}

export function useLocalWhisperBackendStatus(enabled: boolean) {
  return useQuery({
    queryKey: ["localWhisperBackendStatus"],
    enabled,
    queryFn: () => tauriAPI.getLocalWhisperBackendStatus(),
    staleTime: 0,
  });
}

export function useWhisperModels(enabled: boolean) {
  return useQuery<WhisperModelInfo[]>({
    queryKey: ["whisperModels"],
    enabled,
    queryFn: () => tauriAPI.getWhisperModels(),
    staleTime: 0,
  });
}

export function useFireworksModels(enabled: boolean) {
  return useQuery({
    queryKey: ["fireworksModels"],
    enabled,
    queryFn: () => llmAPI.getFireworksModels(),
    staleTime: 0,
  });
}

export function useIsLocalWhisperModelLoaded(enabled: boolean) {
  return useQuery({
    queryKey: ["localWhisperModelLoaded"],
    enabled,
    queryFn: () => tauriAPI.isLocalWhisperModelLoaded(),
    staleTime: 0,
  });
}

export function useLoadLocalWhisperModel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      await tauriAPI.loadLocalWhisperModel();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["localWhisperModelLoaded"] });
    },
  });
}

export function useUnloadLocalWhisperModel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      await tauriAPI.unloadLocalWhisperModel();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["localWhisperModelLoaded"] });
    },
  });
}

export function useWhisperModelsDir() {
  return useQuery({
    queryKey: ["whisperModelsDir"],
    queryFn: () => tauriAPI.getWhisperModelsDir(),
    staleTime: Number.POSITIVE_INFINITY,
  });
}

export function useDownloadWhisperModel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (modelId: string) => {
      await tauriAPI.downloadWhisperModel(modelId);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["whisperModels"] });
    },
  });
}

export function useCancelWhisperModelDownload() {
  return useMutation({
    mutationFn: async (modelId: string) => {
      await tauriAPI.cancelWhisperModelDownload(modelId);
    },
  });
}

export function useDeleteWhisperModel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (modelId: string) => {
      await tauriAPI.deleteWhisperModel(modelId);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["whisperModels"] });
    },
  });
}

export function useValidateWhisperModel() {
  return useMutation({
    mutationFn: async (modelId: string) => {
      const ok = await tauriAPI.validateWhisperModel(modelId);
      return ok;
    },
  });
}

export function useUpdateProxySettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (proxySettings: ProxySettings) => {
      await tauriAPI.updateProxySettings(proxySettings);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

// Save proxy settings to the local store without syncing the pipeline.
// Useful for editing Manual fields before enabling Manual mode.
export function useSaveProxySettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (proxySettings: ProxySettings) => {
      await tauriAPI.updateProxySettings(proxySettings);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateLLMProvider() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (provider: string | null) => {
      await tauriAPI.updateLLMProvider(provider);
      // Sync the pipeline configuration when LLM provider changes
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateLLMModel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (model: string | null) => {
      await tauriAPI.updateLLMModel(model);
      // Sync the pipeline configuration when LLM model changes
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuickAskProvider() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (provider: string | null) => {
      await tauriAPI.updateQuickAskProvider(provider);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuickAskModel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (model: string | null) => {
      await tauriAPI.updateQuickAskModel(model);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuickAskSystemPrompt() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (prompt: string | null) => {
      await tauriAPI.updateQuickAskSystemPrompt(prompt);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuickAskOpenAiReasoningEffort() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (effort: OpenAiReasoningEffort | null) => {
      await tauriAPI.updateQuickAskOpenAiReasoningEffort(effort);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuickAskAnthropicThinkingBudget() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (budget: number | null) => {
      await tauriAPI.updateQuickAskAnthropicThinkingBudget(budget);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuickAskGeminiThinkingBudget() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (budget: number | null) => {
      await tauriAPI.updateQuickAskGeminiThinkingBudget(budget);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateQuickAskGeminiThinkingLevel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (level: "minimal" | "low" | "medium" | "high" | null) => {
      await tauriAPI.updateQuickAskGeminiThinkingLevel(level);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateOpenAiReasoningEffort() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (effort: OpenAiReasoningEffort | null) => {
      await tauriAPI.updateOpenAiReasoningEffort(effort);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateAnthropicThinkingBudget() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (budget: number | null) => {
      await tauriAPI.updateAnthropicThinkingBudget(budget);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateGeminiThinkingBudget() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (budget: number | null) => {
      await tauriAPI.updateGeminiThinkingBudget(budget);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useUpdateGeminiThinkingLevel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (level: "minimal" | "low" | "medium" | "high" | null) => {
      await tauriAPI.updateGeminiThinkingLevel(level);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

// STT Timeout mutation (local settings)
export function useUpdateSTTTimeout() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (timeoutSeconds: number | null) => {
      await tauriAPI.updateSTTTimeout(timeoutSeconds);
      await configAPI.syncPipelineConfig();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

// Request Logs queries and mutations
export function useRequestLogs(limit?: number) {
  return useQuery({
    queryKey: ["requestLogs", limit],
    queryFn: () => logsAPI.getRequestLogs(limit),
    refetchInterval: 2000, // Refresh every 2 seconds to show live updates
  });
}

export function useClearRequestLogs() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => logsAPI.clearRequestLogs(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["requestLogs"] });
    },
  });
}

// Retry a previous transcription attempt by request id (loads saved audio in backend).
export function useRetryTranscription() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (requestId: string) => sttAPI.retryTranscription({ requestId }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["history"] });
      queryClient.invalidateQueries({ queryKey: ["historyAll"] });
      queryClient.invalidateQueries({ queryKey: ["historyPage"] });
      queryClient.invalidateQueries({ queryKey: ["requestLogs"] });
    },
  });
}
