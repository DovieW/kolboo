import { describe, expect, it, vi } from "vitest";
import type {
	AppSettings,
	CostByProvider,
	CostSummary,
	DataStorageSummary,
	HistoryPageResult,
	LocalWhisperBackendStatus,
	ModelPricing,
	SettingsGuideState,
	SystemProxyInfo,
	WhisperModelInfo,
} from "../tauri";
import {
	createAudioMuteSupportedQueryFn,
	createCostByProviderQueryFn,
	createCostSummaryQueryFn,
	createHistoryAllQueryFn,
	createHistoryPageQueryFn,
	createModelPricingQueryFn,
	createRequestLogsQueryFn,
	createSettingsQueryFn,
	type QueryFnDeps,
} from "./queryFns";

const createDeps = (): QueryFnDeps => {
	const emptySettings = {} as AppSettings;
	const emptyCostSummary: CostSummary = {
		timeframe: "7d",
		total_usd_micros: 0,
		events_total: 0,
		events_with_cost: 0,
		earliest_included_at: null,
		latest_included_at: null,
	};
	const emptyCostByProvider: CostByProvider = {
		timeframe: "7d",
		providers: [],
	};
	const emptyHistoryPage: HistoryPageResult = {
		items: [],
		totalAll: 0,
		totalFiltered: 0,
		page: 1,
		pageSize: 25,
		sttModelUsage: [],
		llmModelUsage: [],
	};
	const emptySystemProxy: SystemProxyInfo = {
		env_http_proxy: null,
		env_https_proxy: null,
		env_no_proxy: null,
		windows_internet_settings: null,
	};
	const emptyWhisperBackend: LocalWhisperBackendStatus = {
		build_has_local_whisper: false,
		build_has_cuda: false,
		compute: "cpu",
		reason: null,
		missing_dlls: [],
	};
	const emptyStorageSummary: DataStorageSummary = {
		recordings_count: 0,
		recordings_bytes: 0,
		history_count: 0,
		history_bytes: 0,
		request_logs_count: 0,
		stats_files_count: 0,
		stats_bytes: 0,
		settings_bytes: 0,
		api_keys_set_count: 0,
	};
	const emptyWhisperModels: WhisperModelInfo[] = [];

	return {
		tauriAPI: {
			getModelPricing: vi.fn(async () => null as ModelPricing | null),
			getCostSummary: vi.fn(async () => emptyCostSummary),
			getCostByProvider: vi.fn(async () => emptyCostByProvider),
			getSettings: vi.fn(async () => emptySettings),
			getSystemProxyInfo: vi.fn(async () => emptySystemProxy),
			getSettingsGuideState: vi.fn(async () => "pending" as SettingsGuideState),
			isAudioMuteSupported: vi.fn(async () => true),
			getHistory: vi.fn(async () => []),
			getHistoryPage: vi.fn(async () => emptyHistoryPage),
			isLocalWhisperAvailable: vi.fn(async () => false),
			getLocalWhisperBackendStatus: vi.fn(async () => emptyWhisperBackend),
			getWhisperModels: vi.fn(async () => emptyWhisperModels),
			isLocalWhisperModelLoaded: vi.fn(async () => false),
			getWhisperModelsDir: vi.fn(async () => ""),
		},
		sttAPI: {
			hasLastAudio: vi.fn(async () => false),
			getLastRecordingDiagnostics: vi.fn(async () => ({
				stats: { duration_secs: 0, rms: 0, peak: 0 },
				speech_detected: null,
			})),
		},
		recordingsAPI: {
			getRecordingsStats: vi.fn(async () => ({ count: 0, bytes: 0 })),
		},
		dataAPI: {
			getStorageSummary: vi.fn(async () => emptyStorageSummary),
		},
		configAPI: {
			getDefaultSections: vi.fn(async () => ({ system: "" })),
			getAvailableProviders: vi.fn(async () => ({ stt: [], llm: [] })),
		},
		llmAPI: {
			getFireworksModels: vi.fn(async () => []),
			getOllamaModels: vi.fn(async () => []),
		},
		logsAPI: {
			getRequestLogs: vi.fn(async () => []),
		},
	};
};

describe("queryFns", () => {
	it("normalizes cost summary filters before querying", async () => {
		const deps = createDeps();
		const { normalized, queryFn } = createCostSummaryQueryFn(deps, "7d", {
			kind: "stt",
			sttModelKeys: ["b", "a"],
			llmModelKeys: ["z", "y"],
		});

		expect(normalized.sttModelKeys).toEqual(["a", "b"]);
		expect(normalized.llmModelKeys).toEqual(["y", "z"]);
		expect(normalized.excludeFreeTier).toBe(true);

		await queryFn();

		expect(deps.tauriAPI.getCostSummary).toHaveBeenCalledWith({
			timeframe: "7d",
			kind: "stt",
			sttModelKeys: ["a", "b"],
			llmModelKeys: ["y", "z"],
			excludeFreeTier: true,
		});
	});

	it("normalizes cost-by-provider defaults", async () => {
		const deps = createDeps();
		const { normalized, queryFn } = createCostByProviderQueryFn(deps, "24h");

		expect(normalized.sttModelKeys).toEqual([]);
		expect(normalized.llmModelKeys).toEqual([]);
		expect(normalized.excludeFreeTier).toBe(true);

		await queryFn();

		expect(deps.tauriAPI.getCostByProvider).toHaveBeenCalledWith({
			timeframe: "24h",
			kind: undefined,
			sttModelKeys: [],
			llmModelKeys: [],
			excludeFreeTier: true,
		});
	});

	it("normalizes history page query defaults", async () => {
		const deps = createDeps();
		const { normalized, queryFn } = createHistoryPageQueryFn(deps, {
			filterText: "  hello ",
			selectedSttModelKeys: ["b", "a"],
			page: 2,
		});

		expect(normalized.filterText).toBe("  hello ");
		expect(normalized.selectedSttModelKeys).toEqual(["a", "b"]);
		expect(normalized.selectedLlmModelKeys).toEqual([]);
		expect(normalized.showFailed).toBe(true);
		expect(normalized.showEmptyTranscript).toBe(false);
		expect(normalized.page).toBe(2);
		expect(normalized.pageSize).toBe(25);
		expect(normalized.includeUsageCounts).toBe(true);

		await queryFn();

		expect(deps.tauriAPI.getHistoryPage).toHaveBeenCalledWith({
			filterText: "  hello ",
			showFailed: true,
			showEmptyTranscript: false,
			selectedSttModelKeys: ["a", "b"],
			selectedLlmModelKeys: [],
			page: 2,
			pageSize: 25,
			includeUsageCounts: true,
		});
	});

	it("history all query passes undefined limit", async () => {
		const deps = createDeps();
		const queryFn = createHistoryAllQueryFn(deps);

		await queryFn();

		expect(deps.tauriAPI.getHistory).toHaveBeenCalledWith(undefined);
	});

	it("normalizes model pricing params", async () => {
		const deps = createDeps();
		const { normalized, queryFn } = createModelPricingQueryFn(deps, {
			provider: null,
			kind: "llm",
			model: null,
		});

		expect(normalized.provider).toBe("");
		expect(normalized.model).toBe("");

		await queryFn();

		expect(deps.tauriAPI.getModelPricing).toHaveBeenCalledWith({
			provider: "",
			kind: "llm",
			model: "",
		});
	});

	it("settings query calls getSettings", async () => {
		const deps = createDeps();
		const queryFn = createSettingsQueryFn(deps);

		await queryFn();

		expect(deps.tauriAPI.getSettings).toHaveBeenCalled();
	});

	it("audio mute supported query calls isAudioMuteSupported", async () => {
		const deps = createDeps();
		const queryFn = createAudioMuteSupportedQueryFn(deps);

		await queryFn();

		expect(deps.tauriAPI.isAudioMuteSupported).toHaveBeenCalled();
	});

	it("request logs query forwards limit", async () => {
		const deps = createDeps();
		const queryFn = createRequestLogsQueryFn(deps, 25);

		await queryFn();

		expect(deps.logsAPI.getRequestLogs).toHaveBeenCalledWith(25);
	});
});
