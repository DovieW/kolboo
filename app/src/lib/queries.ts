import { notifications } from "@mantine/notifications";
import {
	keepPreviousData,
	useMutation,
	useQuery,
	useQueryClient,
} from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { updateHotkeyShortcutCardWithValidation } from "./hotkeyMutations";
import type { HotkeyShortcutCard } from "./hotkeys";
import {
	createAudioMuteSupportedQueryFn,
	createAvailableProvidersQueryFn,
	createCostByProviderQueryFn,
	createCostSummaryQueryFn,
	createDataStorageSummaryQueryFn,
	createDefaultSectionsQueryFn,
	createFireworksModelsQueryFn,
	createHasLastAudioQueryFn,
	createHistoryAllQueryFn,
	createHistoryPageQueryFn,
	createIsLocalWhisperAvailableQueryFn,
	createIsLocalWhisperModelLoadedQueryFn,
	createLastRecordingDiagnosticsQueryFn,
	createLocalWhisperBackendStatusQueryFn,
	createModelPricingQueryFn,
	createOllamaModelsQueryFn,
	createRecordingsStatsQueryFn,
	createRequestLogsQueryFn,
	createSettingsGuideStateQueryFn,
	createSettingsQueryFn,
	createSystemProxyInfoQueryFn,
	createWhisperModelsDirQueryFn,
	createWhisperModelsQueryFn,
} from "./queries/queryFns";
import {
	type AppSettings,
	type AudioCue,
	audioSettingsTestAPI,
	type CleanupPromptSections,
	type CostTimeframe,
	configAPI,
	dataAPI,
	type HistoryPageQuery,
	type HotkeyConfig,
	type IterateRewritePromptResponse,
	llmAPI,
	logsAPI,
	type MainWindowCloseBehavior,
	type ModelPricingKind,
	type OcrAuthMode,
	type OcrAutoCaptureTiming,
	type OpenAiReasoningEffort,
	type OutputMode,
	type OverlayMonitorTarget,
	type PlayingAudioHandling,
	type ProxySettings,
	type QuickAskDismissMode,
	type RewriteProgramPromptProfile,
	recordingsAPI,
	type SettingsGuideState,
	sttAPI,
	type TestLlmRewriteResponse,
	type TestRewriteWithPromptResponse,
	tauriAPI,
	type WhisperModelInfo,
} from "./tauri";

const queryFnDeps = {
	tauriAPI,
	sttAPI,
	recordingsAPI,
	dataAPI,
	configAPI,
	llmAPI,
	logsAPI,
} as const;

export function useModelPricing(
	provider: string | null,
	kind: ModelPricingKind,
	model: string | null,
) {
	const { normalized, queryFn } = createModelPricingQueryFn(queryFnDeps, {
		provider,
		kind,
		model,
	});

	return useQuery({
		queryKey: [
			"modelPricing",
			normalized.provider,
			normalized.kind,
			normalized.model,
		],
		enabled: Boolean(provider) && Boolean(model),
		queryFn,
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
	},
) {
	const { normalized, queryFn } = createCostSummaryQueryFn(
		queryFnDeps,
		timeframe,
		filters,
	);

	return useQuery({
		queryKey: [
			"costSummary",
			timeframe,
			normalized.kind ?? "all",
			normalized.excludeFreeTier ? "exclude_free" : "include_free",
			normalized.sttModelKeys,
			normalized.llmModelKeys,
		],
		queryFn,
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
	},
) {
	const { normalized, queryFn } = createCostByProviderQueryFn(
		queryFnDeps,
		timeframe,
		filters,
	);

	return useQuery({
		queryKey: [
			"costByProvider",
			timeframe,
			normalized.kind ?? "all",
			normalized.excludeFreeTier ? "exclude_free" : "include_free",
			normalized.sttModelKeys,
			normalized.llmModelKeys,
		],
		queryFn,
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
		queryFn: createHasLastAudioQueryFn(queryFnDeps),
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
		queryFn: createSettingsQueryFn(queryFnDeps),
		staleTime: Number.POSITIVE_INFINITY,
	});
}

export function useSystemProxyInfo() {
	return useQuery({
		queryKey: ["systemProxyInfo"],
		queryFn: createSystemProxyInfoQueryFn(queryFnDeps),
		staleTime: 0,
		refetchOnWindowFocus: true,
	});
}

export function useSettingsGuideState() {
	return useQuery({
		queryKey: ["settingsGuideState"],
		queryFn: createSettingsGuideStateQueryFn(queryFnDeps),
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

export function useCreateHotkeyShortcutCard() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (card: HotkeyShortcutCard) =>
			tauriAPI.createHotkeyShortcutCard(card),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateHotkeyShortcutCard() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (params: {
			cardId: string;
			hotkey: HotkeyConfig | null;
		}) => {
			const settings =
				queryClient.getQueryData<AppSettings>(["settings"]) ??
				(await tauriAPI.getSettings());
			const cards = settings?.hotkey_shortcuts ?? [];
			await updateHotkeyShortcutCardWithValidation({
				cardId: params.cardId,
				nextHotkey: params.hotkey,
				cards,
				updateCard: async (cardId, hotkey) => {
					await tauriAPI.updateHotkeyShortcutCard(cardId, hotkey);
				},
			});
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useDeleteHotkeyShortcutCard() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (cardId: string) => tauriAPI.deleteHotkeyShortcutCard(cardId),
		onMutate: async (cardId: string) => {
			await queryClient.cancelQueries({ queryKey: ["settings"] });

			const previous = queryClient.getQueryData<AppSettings>(["settings"]);
			if (previous) {
				queryClient.setQueryData<AppSettings>(["settings"], {
					...previous,
					hotkey_shortcuts: previous.hotkey_shortcuts.filter(
						(card) => card.id !== cardId,
					),
				});
			}

			return { previous };
		},
		onError: (_error, _cardId, context) => {
			if (context?.previous) {
				queryClient.setQueryData<AppSettings>(["settings"], context.previous);
			}
		},
		onSettled: () => {
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
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateHotkeyDebugEnabled(enabled),
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

export function useUpdateOutputSmartPasteProtection() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateOutputSmartPasteProtection(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
		onError: (error) => {
			console.error("Update smart paste protection failed:", error);
			notifications.show({
				title: "Couldn't save setting",
				message:
					"Smart paste protection couldn't be saved. Your previous setting is still active.",
				color: "red",
			});
		},
	});
}

export function useUpdateRequestLogsPrivacyMode() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateRequestLogsPrivacyMode(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
			queryClient.invalidateQueries({ queryKey: ["requestLogs"] });
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
		queryFn: createLastRecordingDiagnosticsQueryFn(queryFnDeps),
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
		queryFn: createRecordingsStatsQueryFn(queryFnDeps),
		staleTime: 0,
		refetchOnWindowFocus: true,
		refetchInterval: 10000,
	});
}

export function useDataStorageSummary() {
	return useQuery({
		queryKey: ["dataStorageSummary"],
		queryFn: createDataStorageSummaryQueryFn(queryFnDeps),
		staleTime: 0,
		refetchOnWindowFocus: true,
		refetchInterval: 10000,
	});
}

export function useIsAudioMuteSupported() {
	return useQuery({
		queryKey: ["audioMuteSupported"],
		queryFn: createAudioMuteSupportedQueryFn(queryFnDeps),
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
					context.previousSettings,
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
					context.previousSettings,
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
// Fetch all history entries (unbounded). Intended for optional features like
// analysis where full history is required, but shouldn't load by default.
export function useHistoryAll(options?: { enabled?: boolean }) {
	return useQuery({
		queryKey: ["historyAll"],
		queryFn: createHistoryAllQueryFn(queryFnDeps),
		enabled: options?.enabled ?? true,
	});
}

export function useHistoryPage(params: HistoryPageQuery) {
	const { normalized, queryFn } = createHistoryPageQueryFn(queryFnDeps, params);

	return useQuery({
		queryKey: [
			"historyPage",
			normalized.filterText,
			normalized.showFailed,
			normalized.showEmptyTranscript,
			normalized.selectedSttModelKeys,
			normalized.selectedLlmModelKeys,
			normalized.page,
			normalized.pageSize,
			normalized.includeUsageCounts,
		],
		queryFn,
		placeholderData: keepPreviousData,
		// Keep things feeling responsive while typing filters.
		refetchOnWindowFocus: true,
	});
}

// Config API queries and mutations (now using Tauri commands)
export function useDefaultSections() {
	return useQuery({
		queryKey: ["defaultSections"],
		queryFn: createDefaultSectionsQueryFn(queryFnDeps),
		staleTime: Number.POSITIVE_INFINITY, // Default prompts never change
	});
}

// Provider queries and mutations

export function useAvailableProviders() {
	return useQuery({
		queryKey: ["availableProviders"],
		queryFn: createAvailableProvidersQueryFn(queryFnDeps),
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

export function useUpdateOllamaUrl() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (baseUrl: string | null) => {
			await tauriAPI.updateOllamaUrl(baseUrl);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrBaseUrl() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (baseUrl: string | null) => {
			await tauriAPI.updateOcrBaseUrl(baseUrl);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrModel() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (model: string | null) => {
			await tauriAPI.updateOcrModel(model);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrAuthMode() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (mode: OcrAuthMode) => {
			await tauriAPI.updateOcrAuthMode(mode);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrPrompt() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (prompt: string) => {
			await tauriAPI.updateOcrPrompt(prompt);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrMaxTokens() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (maxTokens: number) => {
			await tauriAPI.updateOcrMaxTokens(maxTokens);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrTemperature() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (temperature: number) => {
			await tauriAPI.updateOcrTemperature(temperature);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrTopP() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (topP: number) => {
			await tauriAPI.updateOcrTopP(topP);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrRequestTimeoutMs() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (timeoutMs: number) => {
			await tauriAPI.updateOcrRequestTimeoutMs(timeoutMs);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrContextMaxChars() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (maxChars: number) => {
			await tauriAPI.updateOcrContextMaxChars(maxChars);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrAutoCaptureTiming() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (timing: OcrAutoCaptureTiming) => {
			await tauriAPI.updateOcrAutoCaptureTiming(timing);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrHallucinationProtection() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (enabled: boolean) => {
			await tauriAPI.updateOcrHallucinationProtection(enabled);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrHallucinationThreshold() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (value: number) => {
			await tauriAPI.updateOcrHallucinationThreshold(value);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrResizeMaxDimension() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (value: number) => {
			await tauriAPI.updateOcrResizeMaxDimension(value);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrResizeFilter() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (
			filter: "nearest" | "triangle" | "catmullrom" | "lanczos3",
		) => {
			await tauriAPI.updateOcrResizeFilter(filter);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useSetOcrApiKey() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (apiKey: string) => {
			await tauriAPI.setApiKey("ocr_api_key", apiKey);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["availableProviders"] });
		},
	});
}

export function useClearOcrApiKey() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async () => {
			await tauriAPI.clearApiKey("ocr_api_key");
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["availableProviders"] });
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
		queryFn: createIsLocalWhisperAvailableQueryFn(queryFnDeps),
		staleTime: Number.POSITIVE_INFINITY,
	});
}

export function useLocalWhisperBackendStatus(enabled: boolean) {
	return useQuery({
		queryKey: ["localWhisperBackendStatus"],
		enabled,
		queryFn: createLocalWhisperBackendStatusQueryFn(queryFnDeps),
		staleTime: 0,
	});
}

export function useWhisperModels(enabled: boolean) {
	return useQuery<WhisperModelInfo[]>({
		queryKey: ["whisperModels"],
		enabled,
		queryFn: createWhisperModelsQueryFn(queryFnDeps),
		staleTime: 0,
	});
}

export function useFireworksModels(enabled: boolean) {
	return useQuery({
		queryKey: ["fireworksModels"],
		enabled,
		queryFn: createFireworksModelsQueryFn(queryFnDeps),
		staleTime: 0,
	});
}

export function useOllamaModels(enabled: boolean) {
	return useQuery({
		queryKey: ["ollamaModels"],
		enabled,
		queryFn: createOllamaModelsQueryFn(queryFnDeps),
		staleTime: 0,
	});
}

export function useIsLocalWhisperModelLoaded(enabled: boolean) {
	return useQuery({
		queryKey: ["localWhisperModelLoaded"],
		enabled,
		queryFn: createIsLocalWhisperModelLoadedQueryFn(queryFnDeps),
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
		queryFn: createWhisperModelsDirQueryFn(queryFnDeps),
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

export function useUpdateQuickAskDismissMode() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (mode: QuickAskDismissMode) => {
			await tauriAPI.updateQuickAskDismissMode(mode);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskIncludeSelectedText() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (enabled: boolean) => {
			await tauriAPI.updateQuickAskIncludeSelectedText(enabled);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskConversationHistoryEnabled() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (enabled: boolean) => {
			await tauriAPI.updateQuickAskConversationHistoryEnabled(enabled);
			await configAPI.syncPipelineConfig();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskConversationHistoryCount() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: async (count: number) => {
			await tauriAPI.updateQuickAskConversationHistoryCount(count);
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
		queryFn: createRequestLogsQueryFn(queryFnDeps, limit),
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
