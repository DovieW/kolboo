import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AudioInputDevicesQueryData } from "../audioDevices";
import { updateHotkeyShortcutCardWithValidation } from "../hotkeyMutations";
import type { HotkeyShortcutCard } from "../hotkeys";
import {
	type AppSettings,
	type AudioCue,
	type CleanupPromptSections,
	type HotkeyConfig,
	type MainWindowCloseBehavior,
	type OutputMode,
	type OverlayMonitorTarget,
	type PlayingAudioHandling,
	type QuickAskDismissMode,
	type RewriteProgramPromptProfile,
	type SettingsGuideState,
	tauriAPI,
} from "../tauri";
import {
	createAudioInputDevicesQueryFn,
	createSettingsGuideStateQueryFn,
	createSettingsQueryFn,
} from "./queryFns";
import { queryFnDeps, useSettingsInvalidatingMutation } from "./shared";

// General app settings hooks live here so UI components can depend on one
// module for settings reads/writes without re-importing the monolith.
export function useSettings() {
	return useQuery({
		queryKey: ["settings"],
		queryFn: createSettingsQueryFn(queryFnDeps),
		staleTime: Number.POSITIVE_INFINITY,
	});
}

export function useAudioInputDevices() {
	return useQuery<AudioInputDevicesQueryData>({
		queryKey: ["audioInputDevices"],
		queryFn: createAudioInputDevicesQueryFn(queryFnDeps),
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
