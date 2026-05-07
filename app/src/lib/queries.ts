import { notifications } from "@mantine/notifications";
import {
	keepPreviousData,
	type QueryClient,
	useMutation,
	useQuery,
	useQueryClient,
} from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
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
	licenseAPI,
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
	policyAPI,
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
import { listenTyped } from "./tauri/events";
import {
	authReasonCodeToMessage,
	normalizeAuthReasonCode,
} from "./tauri/license";
import {
	applySettingsRuntimeSyncPolicy,
	classifySettingsRuntimeEffects,
	type SettingsQueryInvalidation,
} from "./tauri/settingsSync";

export function toManagedInferenceMessage(error: unknown): string {
	if (!(error && typeof error === "object")) {
		return "Managed inference is temporarily unavailable right now. You can retry, or switch to BYOK providers in Settings.";
	}

	const reasonCode = normalizeAuthReasonCode(
		(error as { reason_code?: unknown }).reason_code,
	);
	const reasonCodeMessage = authReasonCodeToMessage(reasonCode);
	if (reasonCodeMessage) {
		if (reasonCode === "insufficient_tier") {
			return `${reasonCodeMessage} You can continue with BYOK providers in Settings.`;
		}
		return reasonCodeMessage;
	}

	const category = (error as { category?: string }).category;
	if (category === "unauthorized") {
		return "Your session expired. Please sign in again to continue.";
	}
	if (category === "ineligible") {
		return "Your account or org is not eligible for managed inference right now.";
	}
	if (category === "over_quota") {
		return "You've reached your managed usage limit. Please wait for reset or switch to BYOK.";
	}

	return "Managed inference is temporarily unavailable right now. You can retry, or switch to BYOK providers in Settings.";
}

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

export function usePolicyState() {
	const queryClient = useQueryClient();

	useEffect(() => {
		let unlisten: (() => void) | null = null;

		listenTyped("policy-state-changed", () => {
			void invalidatePolicyRelatedQueries(queryClient);
		})
			.then((fn) => {
				unlisten = fn;
			})
			.catch((error) => {
				console.warn("Failed to subscribe to policy-state-changed:", error);
			});

		return () => {
			try {
				unlisten?.();
			} catch {
				// ignore
			}
		};
	}, [queryClient]);

	return useQuery({
		queryKey: ["policyState"],
		queryFn: () => tauriAPI.getPolicyState(),
		staleTime: 0,
		refetchOnWindowFocus: true,
	});
}

export async function applySettingsQueryInvalidations(
	queryClient: Pick<QueryClient, "invalidateQueries">,
	invalidations: readonly SettingsQueryInvalidation[],
): Promise<void> {
	await Promise.all(
		invalidations.map((invalidation) =>
			queryClient.invalidateQueries({ queryKey: invalidation.queryKey }),
		),
	);
}

export async function invalidatePolicyRelatedQueries(
	queryClient: Pick<QueryClient, "invalidateQueries">,
): Promise<void> {
	await applySettingsQueryInvalidations(
		queryClient,
		classifySettingsRuntimeEffects({ policyNormalized: true })
			.queryInvalidations,
	);
}

export async function invalidateLicenseRelatedQueries(
	queryClient: Pick<QueryClient, "invalidateQueries">,
): Promise<void> {
	await applySettingsQueryInvalidations(
		queryClient,
		classifySettingsRuntimeEffects({
			patch: { license_state: true },
		}).queryInvalidations.filter(
			(invalidation) => invalidation.reason === "license",
		),
	);
}

export async function invalidateLogoutRelatedQueries(
	queryClient: Pick<QueryClient, "invalidateQueries">,
): Promise<void> {
	await Promise.all([
		invalidateLicenseRelatedQueries(queryClient),
		invalidatePolicyRelatedQueries(queryClient),
	]);
}

function buildSettingsMutationInvalidations(
  extraInvalidations: readonly SettingsQueryInvalidation[] = [],
): readonly SettingsQueryInvalidation[] {
  return [
    { queryKey: ["settings"], reason: "settings" },
    ...extraInvalidations,
  ];
}

export async function invalidateSettingsQueries(
  queryClient: Pick<QueryClient, "invalidateQueries">,
  extraInvalidations: readonly SettingsQueryInvalidation[] = [],
): Promise<void> {
  await applySettingsQueryInvalidations(
    queryClient,
    buildSettingsMutationInvalidations(extraInvalidations),
  );
}

function useSettingsInvalidatingMutation<TVariables, TData = unknown>(
  mutationFn: (variables: TVariables) => Promise<TData>,
  options?: {
    extraInvalidations?: readonly SettingsQueryInvalidation[];
    onError?: (error: unknown) => void;
  },
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn,
    onSuccess: () => {
      void invalidateSettingsQueries(queryClient, options?.extraInvalidations);
    },
    onError: options?.onError,
  });
}

export function useLicenseQueryBootstrap() {
	const queryClient = useQueryClient();

	useLicenseState();
	useLicenseAuthContext();

	useEffect(() => {
		let unlisten: (() => void) | null = null;

		licenseAPI
			.onTransition(() => {
				void invalidateLicenseRelatedQueries(queryClient);
			})
			.then((fn) => {
				unlisten = fn;
			})
			.catch((error) => {
				console.warn(
					"Failed to subscribe to license transition events:",
					error,
				);
			});

		return () => {
			try {
				unlisten?.();
			} catch {
				// ignore
			}
		};
	}, [queryClient]);
}

export function createPolicySyncMutationFn(deps: {
	syncPolicy: (request?: { policyPack?: unknown }) => Promise<unknown>;
	invoke: (command: string) => Promise<unknown>;
	emitSettingsChanged?: () => Promise<unknown>;
}) {
	return async (request?: { policyPack?: unknown }) => {
		const state = await deps.syncPolicy(request);
		await applySettingsRuntimeSyncPolicy({
			policyNormalized: true,
			backendEventEmitted: true,
			invoke: deps.invoke,
			emitSettingsChanged: deps.emitSettingsChanged ?? (async () => undefined),
		});
		return state;
	};
}

export function usePolicySync() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: createPolicySyncMutationFn({
			syncPolicy: tauriAPI.syncPolicy,
			invoke,
		}),
		onSuccess: () => {
			void invalidatePolicyRelatedQueries(queryClient);
		},
	});
}

export function usePolicyDiagnosticsExport() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: () => policyAPI.exportPolicyDiagnostics(),
		onSuccess: () => {
			void invalidatePolicyRelatedQueries(queryClient);
		},
	});
}

export function createLicenseStateQueryFn(
	api: Pick<typeof tauriAPI, "getLicenseState"> = tauriAPI,
) {
	return () => api.getLicenseState();
}

export function createLicenseAuthContextQueryFn(
	api: Pick<typeof tauriAPI, "getLicenseAuthContext"> = tauriAPI,
) {
	return () => api.getLicenseAuthContext();
}

export function createRefreshLicenseEntitlementMutationFn(
	api: Pick<typeof licenseAPI, "refreshEntitlement"> = licenseAPI,
) {
	return (simulateFailure?: boolean) => api.refreshEntitlement(simulateFailure);
}

export function useLicenseState() {
	return useQuery({
		queryKey: ["licenseState"],
		queryFn: createLicenseStateQueryFn(),
		staleTime: 0,
		refetchOnWindowFocus: true,
	});
}

export function useLicenseAuthContext() {
	return useQuery({
		queryKey: ["licenseAuthContext"],
		queryFn: createLicenseAuthContextQueryFn(),
		staleTime: 0,
		refetchOnWindowFocus: true,
	});
}

export function useStartLicenseLogin() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (request?: {
			provider_hint?: string | null;
			auth_provider?: string | null;
			email?: string | null;
			password?: string | null;
		}) => licenseAPI.startLogin(request),
		onSuccess: () => {
			void invalidateLicenseRelatedQueries(queryClient);
		},
	});
}

export function useLogoutLicense() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: () => licenseAPI.logout(),
		onSuccess: () => {
			void invalidateLogoutRelatedQueries(queryClient);
		},
	});
}

export function useRefreshLicenseEntitlement() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: createRefreshLicenseEntitlementMutationFn(),
		onSuccess: () => {
			void invalidateLicenseRelatedQueries(queryClient);
		},
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
	return useSettingsInvalidatingMutation((card: HotkeyShortcutCard) =>
    tauriAPI.createHotkeyShortcutCard(card),
  );
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
	return useSettingsInvalidatingMutation((micId: string | null) =>
    tauriAPI.updateSelectedMic(micId),
  );
}

export function useUpdateSoundEnabled() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateSoundEnabled(enabled),
  );
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
	return useSettingsInvalidatingMutation((cue: AudioCue) =>
    tauriAPI.updateAudioCue(cue),
  );
}

export function useUpdateAccentColor() {
	return useSettingsInvalidatingMutation(
    (color: string | null) => tauriAPI.updateAccentColor(color),
    {
      onError: (error) => {
        console.error("Update accent color failed:", error);
      },
    },
  );
}

export function useUpdateMainWindowCloseBehavior() {
	return useSettingsInvalidatingMutation((behavior: MainWindowCloseBehavior) =>
    tauriAPI.updateMainWindowCloseBehavior(behavior),
  );
}

export function useUpdateRewriteLlmEnabled() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateRewriteLlmEnabled(enabled),
  );
}

export function useUpdatePlayingAudioHandling() {
	return useSettingsInvalidatingMutation((handling: PlayingAudioHandling) =>
    tauriAPI.updatePlayingAudioHandling(handling),
  );
}

export function useUpdateOverlayMode() {
	return useSettingsInvalidatingMutation(
    (mode: "always" | "never" | "recording_only") =>
      tauriAPI.updateOverlayMode(mode),
  );
}

export function useUpdateOverlayShowDetailedLoading() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateOverlayShowDetailedLoading(enabled),
  );
}

export function useUpdateOverlayMonitorTarget() {
	return useSettingsInvalidatingMutation((target: OverlayMonitorTarget) =>
    tauriAPI.updateOverlayMonitorTarget(target),
  );
}

export function useUpdateOutputMode() {
	return useSettingsInvalidatingMutation((mode: OutputMode) =>
    tauriAPI.updateOutputMode(mode),
  );
}

export function useUpdateOutputHitEnter() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateOutputHitEnter(enabled),
  );
}

export function useUpdateOutputSmartPasteProtection() {
	return useSettingsInvalidatingMutation(
    (enabled: boolean) => tauriAPI.updateOutputSmartPasteProtection(enabled),
    {
      onError: (error) => {
        console.error("Update smart paste protection failed:", error);
        notifications.show({
          title: "Couldn't save setting",
          message:
            "Smart paste protection couldn't be saved. Your previous setting is still active.",
          color: "red",
        });
      },
    },
  );
}

export function useUpdateRequestLogsPrivacyMode() {
	return useSettingsInvalidatingMutation(
    (enabled: boolean) => tauriAPI.updateRequestLogsPrivacyMode(enabled),
    {
      extraInvalidations: [{ queryKey: ["requestLogs"], reason: "settings" }],
    },
  );
}

export function useUpdateQuietAudioGateEnabled() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateQuietAudioGateEnabled(enabled),
  );
}

export function useUpdateQuietAudioMinDurationSecs() {
	return useSettingsInvalidatingMutation((seconds: number) =>
    tauriAPI.updateQuietAudioMinDurationSecs(seconds),
  );
}

export function useUpdateQuietAudioRmsDbfsThreshold() {
	return useSettingsInvalidatingMutation((dbfs: number) =>
    tauriAPI.updateQuietAudioRmsDbfsThreshold(dbfs),
  );
}

export function useUpdateQuietAudioPeakDbfsThreshold() {
	return useSettingsInvalidatingMutation((dbfs: number) =>
    tauriAPI.updateQuietAudioPeakDbfsThreshold(dbfs),
  );
}

export function useUpdateNoiseGateThresholdDbfs() {
	return useSettingsInvalidatingMutation((thresholdDbfs: number | null) =>
    tauriAPI.updateNoiseGateThresholdDbfs(thresholdDbfs),
  );
}

export function useUpdateQuietAudioRequireSpeech() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateQuietAudioRequireSpeech(enabled),
  );
}

export function useUpdateHotMicEnabled() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateHotMicEnabled(enabled),
  );
}

export function useUpdateHotMicPreRollMs() {
	return useSettingsInvalidatingMutation((ms: number) =>
    tauriAPI.updateHotMicPreRollMs(ms),
  );
}

export function useUpdateMicAutoRecoverEnabled() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateMicAutoRecoverEnabled(enabled),
  );
}

export function useUpdateAudioDownmixToMono() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateAudioDownmixToMono(enabled),
  );
}

export function useUpdateAudioResampleTo16khz() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateAudioResampleTo16khz(enabled),
  );
}

export function useUpdateAudioHighpassEnabled() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateAudioHighpassEnabled(enabled),
  );
}

export function useUpdateAudioAgcEnabled() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateAudioAgcEnabled(enabled),
  );
}

export function useUpdateAudioNoiseSuppressionEnabled() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateAudioNoiseSuppressionEnabled(enabled),
  );
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
	return useSettingsInvalidatingMutation((max: number) =>
    tauriAPI.updateMaxSavedRecordings(max),
  );
}

export function useUpdateTranscriptionRetentionDeleteRecordings() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateTranscriptionRetentionDeleteRecordings(enabled),
  );
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
		mutationFn: (sections: CleanupPromptSections | null) =>
			tauriAPI.updateCleanupPromptSections(sections),
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
		mutationFn: (profiles: RewriteProgramPromptProfile[]) =>
			tauriAPI.updateRewriteProgramPromptProfiles(profiles),
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
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateGroqFreeTier(enabled),
  );
}

export function useUpdateCerebrasFreeTier() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateCerebrasFreeTier(enabled),
  );
}

export function useUpdateCohereFreeTier() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateCohereFreeTier(enabled),
  );
}

export function useUpdateAssemblyAiFreeTier() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateAssemblyAiFreeTier(enabled),
  );
}

export function useUpdateSpeechmaticsFreeTier() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateSpeechmaticsFreeTier(enabled),
  );
}

export function useUpdateSTTProvider() {
	return useSettingsInvalidatingMutation((provider: string | null) =>
    tauriAPI.updateSTTProvider(provider),
  );
}

export function useUpdateSTTModel() {
	return useSettingsInvalidatingMutation((model: string | null) =>
    tauriAPI.updateSTTModel(model),
  );
}

export function useUpdateSTTLiveOutput() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateSTTLiveOutput(enabled),
  );
}

export function useUpdateSTTSimulatedStreaming() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateSTTSimulatedStreaming(enabled),
  );
}

export function useUpdateSTTLanguage() {
	return useSettingsInvalidatingMutation((language: string) =>
    tauriAPI.updateSTTLanguage(language),
  );
}

export function useUpdateSTTTranscriptionPrompt() {
	return useSettingsInvalidatingMutation((prompt: string | null) =>
    tauriAPI.updateSTTTranscriptionPrompt(prompt),
  );
}

export function useUpdateWhisperServerBaseUrl() {
	return useSettingsInvalidatingMutation((baseUrl: string | null) =>
    tauriAPI.updateWhisperServerBaseUrl(baseUrl),
  );
}

export function useUpdateOllamaUrl() {
	return useSettingsInvalidatingMutation((baseUrl: string | null) =>
    tauriAPI.updateOllamaUrl(baseUrl),
  );
}

export function useUpdateOcrBaseUrl() {
	return useSettingsInvalidatingMutation((baseUrl: string | null) =>
    tauriAPI.updateOcrBaseUrl(baseUrl),
  );
}

export function useUpdateOcrModel() {
	return useSettingsInvalidatingMutation((model: string | null) =>
    tauriAPI.updateOcrModel(model),
  );
}

export function useUpdateOcrAuthMode() {
	return useSettingsInvalidatingMutation((mode: OcrAuthMode) =>
    tauriAPI.updateOcrAuthMode(mode),
  );
}

export function useUpdateOcrPrompt() {
	return useSettingsInvalidatingMutation((prompt: string) =>
    tauriAPI.updateOcrPrompt(prompt),
  );
}

export function useUpdateOcrMaxTokens() {
	return useSettingsInvalidatingMutation((maxTokens: number) =>
    tauriAPI.updateOcrMaxTokens(maxTokens),
  );
}

export function useUpdateOcrTemperature() {
	return useSettingsInvalidatingMutation((temperature: number) =>
    tauriAPI.updateOcrTemperature(temperature),
  );
}

export function useUpdateOcrTopP() {
	return useSettingsInvalidatingMutation((topP: number) =>
    tauriAPI.updateOcrTopP(topP),
  );
}

export function useUpdateOcrRequestTimeoutMs() {
	return useSettingsInvalidatingMutation((timeoutMs: number) =>
    tauriAPI.updateOcrRequestTimeoutMs(timeoutMs),
  );
}

export function useUpdateOcrContextMaxChars() {
	return useSettingsInvalidatingMutation((maxChars: number) =>
    tauriAPI.updateOcrContextMaxChars(maxChars),
  );
}

export function useUpdateOcrAutoCaptureTiming() {
	return useSettingsInvalidatingMutation((timing: OcrAutoCaptureTiming) =>
    tauriAPI.updateOcrAutoCaptureTiming(timing),
  );
}

export function useUpdateOcrHallucinationProtection() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateOcrHallucinationProtection(enabled),
  );
}

export function useUpdateOcrHallucinationThreshold() {
	return useSettingsInvalidatingMutation((value: number) =>
    tauriAPI.updateOcrHallucinationThreshold(value),
  );
}

export function useUpdateOcrResizeMaxDimension() {
	return useSettingsInvalidatingMutation((value: number) =>
    tauriAPI.updateOcrResizeMaxDimension(value),
  );
}

export function useUpdateOcrResizeFilter() {
	return useSettingsInvalidatingMutation(
    (filter: "nearest" | "triangle" | "catmullrom" | "lanczos3") =>
      tauriAPI.updateOcrResizeFilter(filter),
  );
}

export function useSetOcrApiKey() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (apiKey: string) => tauriAPI.setApiKey("ocr_api_key", apiKey),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["availableProviders"] });
		},
	});
}

export function useClearOcrApiKey() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: () => tauriAPI.clearApiKey("ocr_api_key"),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["availableProviders"] });
		},
	});
}

export function useUpdateLocalWhisperModelId() {
	return useSettingsInvalidatingMutation((modelId: string | null) =>
    tauriAPI.updateLocalWhisperModelId(modelId),
  );
}

export function useUpdateLocalWhisperLoadMode() {
	return useSettingsInvalidatingMutation(
    (mode: "manual" | "on_transcribe" | "on_launch") =>
      tauriAPI.updateLocalWhisperLoadMode(mode),
    {
      extraInvalidations: [
        {
          queryKey: ["localWhisperModelLoaded"],
          reason: "settings",
        },
      ],
    },
  );
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
	return useSettingsInvalidatingMutation((proxySettings: ProxySettings) =>
    tauriAPI.updateProxySettings(proxySettings),
  );
}

// Save proxy settings to the local store without syncing the pipeline.
// Useful for editing Manual fields before enabling Manual mode.
export function useSaveProxySettings() {
	return useSettingsInvalidatingMutation(
    async (proxySettings: ProxySettings) => {
      await tauriAPI.updateProxySettings(proxySettings);
    },
  );
}

export function useUpdateLLMProvider() {
	return useSettingsInvalidatingMutation((provider: string | null) =>
    tauriAPI.updateLLMProvider(provider),
  );
}

export function useUpdateLLMModel() {
	return useSettingsInvalidatingMutation((model: string | null) =>
    tauriAPI.updateLLMModel(model),
  );
}

export function useUpdateQuickAskProvider() {
	return useSettingsInvalidatingMutation((provider: string | null) =>
    tauriAPI.updateQuickAskProvider(provider),
  );
}

export function useUpdateQuickAskModel() {
	return useSettingsInvalidatingMutation((model: string | null) =>
    tauriAPI.updateQuickAskModel(model),
  );
}

export function useUpdateQuickAskSystemPrompt() {
	return useSettingsInvalidatingMutation((prompt: string | null) =>
    tauriAPI.updateQuickAskSystemPrompt(prompt),
  );
}

export function useUpdateQuickAskDismissMode() {
	return useSettingsInvalidatingMutation((mode: QuickAskDismissMode) =>
    tauriAPI.updateQuickAskDismissMode(mode),
  );
}

export function useUpdateQuickAskIncludeSelectedText() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateQuickAskIncludeSelectedText(enabled),
  );
}

export function useUpdateQuickAskConversationHistoryEnabled() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
    tauriAPI.updateQuickAskConversationHistoryEnabled(enabled),
  );
}

export function useUpdateQuickAskConversationHistoryCount() {
	return useSettingsInvalidatingMutation((count: number) =>
    tauriAPI.updateQuickAskConversationHistoryCount(count),
  );
}

export function useUpdateQuickAskOpenAiReasoningEffort() {
	return useSettingsInvalidatingMutation(
    (effort: OpenAiReasoningEffort | null) =>
      tauriAPI.updateQuickAskOpenAiReasoningEffort(effort),
  );
}

export function useUpdateQuickAskAnthropicThinkingBudget() {
	return useSettingsInvalidatingMutation((budget: number | null) =>
    tauriAPI.updateQuickAskAnthropicThinkingBudget(budget),
  );
}

export function useUpdateQuickAskGeminiThinkingBudget() {
	return useSettingsInvalidatingMutation((budget: number | null) =>
    tauriAPI.updateQuickAskGeminiThinkingBudget(budget),
  );
}

export function useUpdateQuickAskGeminiThinkingLevel() {
	return useSettingsInvalidatingMutation(
    (level: "minimal" | "low" | "medium" | "high" | null) =>
      tauriAPI.updateQuickAskGeminiThinkingLevel(level),
  );
}

export function useUpdateOpenAiReasoningEffort() {
	return useSettingsInvalidatingMutation(
    (effort: OpenAiReasoningEffort | null) =>
      tauriAPI.updateOpenAiReasoningEffort(effort),
  );
}

export function useUpdateAnthropicThinkingBudget() {
	return useSettingsInvalidatingMutation((budget: number | null) =>
    tauriAPI.updateAnthropicThinkingBudget(budget),
  );
}

export function useUpdateGeminiThinkingBudget() {
	return useSettingsInvalidatingMutation((budget: number | null) =>
    tauriAPI.updateGeminiThinkingBudget(budget),
  );
}

export function useUpdateGeminiThinkingLevel() {
	return useSettingsInvalidatingMutation(
    (level: "minimal" | "low" | "medium" | "high" | null) =>
      tauriAPI.updateGeminiThinkingLevel(level),
  );
}

// STT Timeout mutation (local settings)
export function useUpdateSTTTimeout() {
	return useSettingsInvalidatingMutation((timeoutSeconds: number | null) =>
    tauriAPI.updateSTTTimeout(timeoutSeconds),
  );
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
