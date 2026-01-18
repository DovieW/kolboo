import type {
	CostTimeframe,
	HistoryPageQuery,
	ModelPricingKind,
} from "../tauri";

type TauriAPI = typeof import("../tauri").tauriAPI;
type SttAPI = typeof import("../tauri").sttAPI;
type RecordingsAPI = typeof import("../tauri").recordingsAPI;
type DataAPI = typeof import("../tauri").dataAPI;
type ConfigAPI = typeof import("../tauri").configAPI;
type LlmAPI = typeof import("../tauri").llmAPI;
type LogsAPI = typeof import("../tauri").logsAPI;

export type QueryFnDeps = {
	tauriAPI: Pick<
		TauriAPI,
		| "getModelPricing"
		| "getCostSummary"
		| "getCostByProvider"
		| "getSettings"
		| "getSystemProxyInfo"
		| "getSettingsGuideState"
		| "isAudioMuteSupported"
		| "getHistory"
		| "getHistoryPage"
		| "isLocalWhisperAvailable"
		| "getLocalWhisperBackendStatus"
		| "getWhisperModels"
		| "isLocalWhisperModelLoaded"
		| "getWhisperModelsDir"
	>;
	sttAPI: Pick<SttAPI, "hasLastAudio" | "getLastRecordingDiagnostics">;
	recordingsAPI: Pick<RecordingsAPI, "getRecordingsStats">;
	dataAPI: Pick<DataAPI, "getStorageSummary">;
	configAPI: Pick<ConfigAPI, "getDefaultSections" | "getAvailableProviders">;
	llmAPI: Pick<LlmAPI, "getFireworksModels" | "getOllamaModels">;
	logsAPI: Pick<LogsAPI, "getRequestLogs">;
};

type CostFilters = {
	kind?: "all" | "stt" | "llm";
	sttModelKeys?: string[];
	llmModelKeys?: string[];
	excludeFreeTier?: boolean;
};

type NormalizedCostFilters = {
	kind?: "all" | "stt" | "llm";
	sttModelKeys: string[];
	llmModelKeys: string[];
	excludeFreeTier: boolean;
};

type NormalizedModelPricingParams = {
	provider: string;
	kind: ModelPricingKind;
	model: string;
};

type NormalizedHistoryPageQuery = {
	filterText: string;
	showFailed: boolean;
	showEmptyTranscript: boolean;
	selectedSttModelKeys: string[];
	selectedLlmModelKeys: string[];
	page: number;
	pageSize: number;
	includeUsageCounts: boolean;
};

const normalizeCostFilters = (
	filters?: CostFilters,
): NormalizedCostFilters => ({
	kind: filters?.kind,
	sttModelKeys: (filters?.sttModelKeys ?? []).slice().sort(),
	llmModelKeys: (filters?.llmModelKeys ?? []).slice().sort(),
	excludeFreeTier: filters?.excludeFreeTier ?? true,
});

const normalizeModelPricingParams = (params: {
	provider: string | null;
	kind: ModelPricingKind;
	model: string | null;
}): NormalizedModelPricingParams => ({
	provider: params.provider ?? "",
	kind: params.kind,
	model: params.model ?? "",
});

const normalizeHistoryPageQuery = (
	params: HistoryPageQuery,
): NormalizedHistoryPageQuery => ({
	filterText: (params.filterText ?? "").toString(),
	showFailed: params.showFailed ?? true,
	showEmptyTranscript: params.showEmptyTranscript ?? false,
	selectedSttModelKeys: (params.selectedSttModelKeys ?? []).slice().sort(),
	selectedLlmModelKeys: (params.selectedLlmModelKeys ?? []).slice().sort(),
	page: params.page ?? 1,
	pageSize: params.pageSize ?? 25,
	includeUsageCounts: params.includeUsageCounts ?? true,
});

export const createModelPricingQueryFn = (
	deps: QueryFnDeps,
	params: {
		provider: string | null;
		kind: ModelPricingKind;
		model: string | null;
	},
) => {
	const normalized = normalizeModelPricingParams(params);
	return {
		normalized,
		queryFn: () =>
			deps.tauriAPI.getModelPricing({
				provider: normalized.provider,
				kind: normalized.kind,
				model: normalized.model,
			}),
	};
};

export const createCostSummaryQueryFn = (
	deps: QueryFnDeps,
	timeframe: CostTimeframe,
	filters?: CostFilters,
) => {
	const normalized = normalizeCostFilters(filters);
	return {
		normalized,
		queryFn: () =>
			deps.tauriAPI.getCostSummary({
				timeframe,
				kind: normalized.kind,
				sttModelKeys: normalized.sttModelKeys,
				llmModelKeys: normalized.llmModelKeys,
				excludeFreeTier: normalized.excludeFreeTier,
			}),
	};
};

export const createCostByProviderQueryFn = (
	deps: QueryFnDeps,
	timeframe: CostTimeframe,
	filters?: CostFilters,
) => {
	const normalized = normalizeCostFilters(filters);
	return {
		normalized,
		queryFn: () =>
			deps.tauriAPI.getCostByProvider({
				timeframe,
				kind: normalized.kind,
				sttModelKeys: normalized.sttModelKeys,
				llmModelKeys: normalized.llmModelKeys,
				excludeFreeTier: normalized.excludeFreeTier,
			}),
	};
};

export const createHasLastAudioQueryFn = (deps: QueryFnDeps) => () =>
	deps.sttAPI.hasLastAudio();

export const createSettingsQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getSettings();

export const createSystemProxyInfoQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getSystemProxyInfo();

export const createSettingsGuideStateQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getSettingsGuideState();

export const createLastRecordingDiagnosticsQueryFn =
	(deps: QueryFnDeps) => () =>
		deps.sttAPI.getLastRecordingDiagnostics();

export const createRecordingsStatsQueryFn = (deps: QueryFnDeps) => () =>
	deps.recordingsAPI.getRecordingsStats();

export const createDataStorageSummaryQueryFn = (deps: QueryFnDeps) => () =>
	deps.dataAPI.getStorageSummary();

export const createAudioMuteSupportedQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.isAudioMuteSupported();

export const createHistoryAllQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getHistory(undefined);

export const createHistoryPageQueryFn = (
	deps: QueryFnDeps,
	params: HistoryPageQuery,
) => {
	const normalized = normalizeHistoryPageQuery(params);
	return {
		normalized,
		queryFn: () => deps.tauriAPI.getHistoryPage(normalized),
	};
};

export const createDefaultSectionsQueryFn = (deps: QueryFnDeps) => () =>
	deps.configAPI.getDefaultSections();

export const createAvailableProvidersQueryFn = (deps: QueryFnDeps) => () =>
	deps.configAPI.getAvailableProviders();

export const createIsLocalWhisperAvailableQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.isLocalWhisperAvailable();

export const createLocalWhisperBackendStatusQueryFn =
	(deps: QueryFnDeps) => () =>
		deps.tauriAPI.getLocalWhisperBackendStatus();

export const createWhisperModelsQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getWhisperModels();

export const createFireworksModelsQueryFn = (deps: QueryFnDeps) => () =>
	deps.llmAPI.getFireworksModels();

export const createOllamaModelsQueryFn = (deps: QueryFnDeps) => () =>
	deps.llmAPI.getOllamaModels();

export const createIsLocalWhisperModelLoadedQueryFn =
	(deps: QueryFnDeps) => () =>
		deps.tauriAPI.isLocalWhisperModelLoaded();

export const createWhisperModelsDirQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getWhisperModelsDir();

export const createRequestLogsQueryFn =
	(deps: QueryFnDeps, limit?: number) => () =>
		deps.logsAPI.getRequestLogs(limit);
