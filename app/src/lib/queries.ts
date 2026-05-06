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
		mutationFn: (enabled: boolean) => tauriAPI.updateRewriteLlmEnabled(enabled),
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
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateQuietAudioGateEnabled(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuietAudioMinDurationSecs() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (seconds: number) =>
			tauriAPI.updateQuietAudioMinDurationSecs(seconds),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuietAudioRmsDbfsThreshold() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (dbfs: number) =>
			tauriAPI.updateQuietAudioRmsDbfsThreshold(dbfs),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuietAudioPeakDbfsThreshold() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (dbfs: number) =>
			tauriAPI.updateQuietAudioPeakDbfsThreshold(dbfs),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateNoiseGateThresholdDbfs() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (thresholdDbfs: number | null) =>
			tauriAPI.updateNoiseGateThresholdDbfs(thresholdDbfs),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuietAudioRequireSpeech() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateQuietAudioRequireSpeech(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateHotMicEnabled() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) => tauriAPI.updateHotMicEnabled(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateHotMicPreRollMs() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (ms: number) => tauriAPI.updateHotMicPreRollMs(ms),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateMicAutoRecoverEnabled() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateMicAutoRecoverEnabled(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateAudioDownmixToMono() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateAudioDownmixToMono(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateAudioResampleTo16khz() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateAudioResampleTo16khz(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateAudioHighpassEnabled() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateAudioHighpassEnabled(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateAudioAgcEnabled() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) => tauriAPI.updateAudioAgcEnabled(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateAudioNoiseSuppressionEnabled() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateAudioNoiseSuppressionEnabled(enabled),
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
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) => tauriAPI.updateGroqFreeTier(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateCerebrasFreeTier() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) => tauriAPI.updateCerebrasFreeTier(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateCohereFreeTier() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) => tauriAPI.updateCohereFreeTier(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateAssemblyAiFreeTier() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateAssemblyAiFreeTier(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateSpeechmaticsFreeTier() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateSpeechmaticsFreeTier(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateSTTProvider() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (provider: string | null) =>
			tauriAPI.updateSTTProvider(provider),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateSTTModel() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (model: string | null) => tauriAPI.updateSTTModel(model),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateSTTLiveOutput() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) => tauriAPI.updateSTTLiveOutput(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateSTTSimulatedStreaming() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateSTTSimulatedStreaming(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateSTTLanguage() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (language: string) => tauriAPI.updateSTTLanguage(language),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateSTTTranscriptionPrompt() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (prompt: string | null) =>
			tauriAPI.updateSTTTranscriptionPrompt(prompt),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateWhisperServerBaseUrl() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (baseUrl: string | null) =>
			tauriAPI.updateWhisperServerBaseUrl(baseUrl),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOllamaUrl() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (baseUrl: string | null) => tauriAPI.updateOllamaUrl(baseUrl),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrBaseUrl() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (baseUrl: string | null) => tauriAPI.updateOcrBaseUrl(baseUrl),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrModel() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (model: string | null) => tauriAPI.updateOcrModel(model),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrAuthMode() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (mode: OcrAuthMode) => tauriAPI.updateOcrAuthMode(mode),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrPrompt() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (prompt: string) => tauriAPI.updateOcrPrompt(prompt),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrMaxTokens() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (maxTokens: number) => tauriAPI.updateOcrMaxTokens(maxTokens),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrTemperature() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (temperature: number) =>
			tauriAPI.updateOcrTemperature(temperature),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrTopP() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (topP: number) => tauriAPI.updateOcrTopP(topP),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrRequestTimeoutMs() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (timeoutMs: number) =>
			tauriAPI.updateOcrRequestTimeoutMs(timeoutMs),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrContextMaxChars() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (maxChars: number) =>
			tauriAPI.updateOcrContextMaxChars(maxChars),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrAutoCaptureTiming() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (timing: OcrAutoCaptureTiming) =>
			tauriAPI.updateOcrAutoCaptureTiming(timing),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrHallucinationProtection() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateOcrHallucinationProtection(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrHallucinationThreshold() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (value: number) =>
			tauriAPI.updateOcrHallucinationThreshold(value),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrResizeMaxDimension() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (value: number) => tauriAPI.updateOcrResizeMaxDimension(value),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOcrResizeFilter() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (filter: "nearest" | "triangle" | "catmullrom" | "lanczos3") =>
			tauriAPI.updateOcrResizeFilter(filter),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
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
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (modelId: string | null) =>
			tauriAPI.updateLocalWhisperModelId(modelId),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateLocalWhisperLoadMode() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (mode: "manual" | "on_transcribe" | "on_launch") =>
			tauriAPI.updateLocalWhisperLoadMode(mode),
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
		mutationFn: (proxySettings: ProxySettings) =>
			tauriAPI.updateProxySettings(proxySettings),
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
		mutationFn: (provider: string | null) =>
			tauriAPI.updateLLMProvider(provider),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateLLMModel() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (model: string | null) => tauriAPI.updateLLMModel(model),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskProvider() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (provider: string | null) =>
			tauriAPI.updateQuickAskProvider(provider),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskModel() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (model: string | null) => tauriAPI.updateQuickAskModel(model),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskSystemPrompt() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (prompt: string | null) =>
			tauriAPI.updateQuickAskSystemPrompt(prompt),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskDismissMode() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (mode: QuickAskDismissMode) =>
			tauriAPI.updateQuickAskDismissMode(mode),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskIncludeSelectedText() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateQuickAskIncludeSelectedText(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskConversationHistoryEnabled() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (enabled: boolean) =>
			tauriAPI.updateQuickAskConversationHistoryEnabled(enabled),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskConversationHistoryCount() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (count: number) =>
			tauriAPI.updateQuickAskConversationHistoryCount(count),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskOpenAiReasoningEffort() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (effort: OpenAiReasoningEffort | null) =>
			tauriAPI.updateQuickAskOpenAiReasoningEffort(effort),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskAnthropicThinkingBudget() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (budget: number | null) =>
			tauriAPI.updateQuickAskAnthropicThinkingBudget(budget),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskGeminiThinkingBudget() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (budget: number | null) =>
			tauriAPI.updateQuickAskGeminiThinkingBudget(budget),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateQuickAskGeminiThinkingLevel() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (level: "minimal" | "low" | "medium" | "high" | null) =>
			tauriAPI.updateQuickAskGeminiThinkingLevel(level),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateOpenAiReasoningEffort() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (effort: OpenAiReasoningEffort | null) =>
			tauriAPI.updateOpenAiReasoningEffort(effort),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateAnthropicThinkingBudget() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (budget: number | null) =>
			tauriAPI.updateAnthropicThinkingBudget(budget),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateGeminiThinkingBudget() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (budget: number | null) =>
			tauriAPI.updateGeminiThinkingBudget(budget),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

export function useUpdateGeminiThinkingLevel() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (level: "minimal" | "low" | "medium" | "high" | null) =>
			tauriAPI.updateGeminiThinkingLevel(level),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
}

// STT Timeout mutation (local settings)
export function useUpdateSTTTimeout() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (timeoutSeconds: number | null) =>
			tauriAPI.updateSTTTimeout(timeoutSeconds),
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
