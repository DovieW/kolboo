import type { CostKind } from "../../costParams";
import type {
	CostTimeframe,
	HistoryPageQuery,
	ModelPricingKind,
} from "../../tauri";

type TauriAPI = typeof import("../../tauri").tauriAPI;
type SttAPI = typeof import("../../tauri").sttAPI;
type RecordingsAPI = typeof import("../../tauri").recordingsAPI;
type DataAPI = typeof import("../../tauri").dataAPI;
type ConfigAPI = typeof import("../../tauri").configAPI;
type LlmAPI = typeof import("../../tauri").llmAPI;
type LogsAPI = typeof import("../../tauri").logsAPI;

export type QueryFnDeps = {
	tauriAPI: Pick<
		TauriAPI,
		| "getModelPricing"
		| "getCostSummary"
		| "getCostByProvider"
		| "getSettings"
		| "listAudioInputDevicesV2"
		| "getDefaultAudioInputDeviceName"
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

export type CostFilters = {
	kind?: CostKind;
	sttModelKeys?: string[];
	llmModelKeys?: string[];
	excludeFreeTier?: boolean;
};

export type NormalizedCostFilters = {
	kind?: CostKind;
	sttModelKeys: string[];
	llmModelKeys: string[];
	excludeFreeTier: boolean;
};

export type NormalizedModelPricingParams = {
	provider: string;
	kind: ModelPricingKind;
	model: string;
};

export type NormalizedHistoryPageQuery = {
	filterText: string;
	showFailed: boolean;
	showEmptyTranscript: boolean;
	selectedSttModelKeys: string[];
	selectedLlmModelKeys: string[];
	page: number;
	pageSize: number;
	includeUsageCounts: boolean;
};

export const normalizeCostFilters = (
	filters?: CostFilters,
): NormalizedCostFilters => ({
	kind: filters?.kind,
	sttModelKeys: (filters?.sttModelKeys ?? []).slice().sort(),
	llmModelKeys: (filters?.llmModelKeys ?? []).slice().sort(),
	excludeFreeTier: filters?.excludeFreeTier ?? true,
});

export const normalizeModelPricingParams = (params: {
	provider: string | null;
	kind: ModelPricingKind;
	model: string | null;
}): NormalizedModelPricingParams => ({
	provider: params.provider ?? "",
	kind: params.kind,
	model: params.model ?? "",
});

export const normalizeHistoryPageQuery = (
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

export function createCostQueryFn<TResult>(
	timeframe: CostTimeframe,
	filters: CostFilters | undefined,
	get: (params: {
		timeframe: CostTimeframe;
		kind?: CostKind;
		sttModelKeys?: string[];
		llmModelKeys?: string[];
		excludeFreeTier?: boolean;
	}) => Promise<TResult>,
) {
	const normalized = normalizeCostFilters(filters);
	return {
		normalized,
		queryFn: () =>
			get({
				timeframe,
				kind: normalized.kind,
				sttModelKeys: normalized.sttModelKeys,
				llmModelKeys: normalized.llmModelKeys,
				excludeFreeTier: normalized.excludeFreeTier,
			}),
	};
}
