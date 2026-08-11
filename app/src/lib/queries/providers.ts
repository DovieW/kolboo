import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { managedModelsWithBundledFallback } from "../modelOptions";
import {
	managedInferenceAPI,
	type OcrAuthMode,
	type OcrAutoCaptureTiming,
	type OpenAiReasoningEffort,
	type ProxySettings,
	tauriAPI,
	type WhisperModelInfo,
} from "../tauri";
import {
	createAvailableProvidersQueryFn,
	createFireworksModelsQueryFn,
	createIsLocalWhisperAvailableQueryFn,
	createIsLocalWhisperModelLoadedQueryFn,
	createLocalWhisperBackendStatusQueryFn,
	createOllamaModelsQueryFn,
	createSystemProxyInfoQueryFn,
	createWhisperModelsDirQueryFn,
	createWhisperModelsQueryFn,
} from "./queryFns";
import { queryFnDeps, useSettingsInvalidatingMutation } from "./shared";

// Provider hooks keep model discovery, local-whisper lifecycle, and
// provider-specific settings together so provider behavior remains locally
// understandable instead of being scattered through generic settings code.
export function useSystemProxyInfo() {
	return useQuery({
		queryKey: ["systemProxyInfo"],
		queryFn: createSystemProxyInfoQueryFn(queryFnDeps),
		staleTime: 0,
		refetchOnWindowFocus: true,
	});
}

export function useAvailableProviders() {
	return useQuery({
		queryKey: ["availableProviders"],
		queryFn: createAvailableProvidersQueryFn(queryFnDeps),
	});
}

export function useManagedModels(enabled: boolean) {
	const query = useQuery({
		queryKey: ["managedModels"],
		queryFn: () => managedInferenceAPI.getModels(),
		select: (catalog) => catalog.models,
		enabled,
		staleTime: 5 * 60 * 1000,
		retry: false,
	});

	return {
		...query,
		data: managedModelsWithBundledFallback(query.data),
	};
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

export function useUpdateSTTUseManagedInference() {
	return useSettingsInvalidatingMutation((enabled: boolean) =>
		tauriAPI.updateSTTUseManagedInference(enabled),
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

// STT timeout is a provider-facing tuning knob, so it stays with the rest of
// the provider selection/configuration hooks.
export function useUpdateSTTTimeout() {
	return useSettingsInvalidatingMutation((timeoutSeconds: number | null) =>
		tauriAPI.updateSTTTimeout(timeoutSeconds),
	);
}
