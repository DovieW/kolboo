import { Store } from "@tauri-apps/plugin-store";
import type {
	DataStorageSummary,
	RecordingsStats,
	TranscriptionRetentionUnit,
} from "../tauri/types";

export type RequestLogsRetentionMode = "amount" | "time";
export type RetentionMode = "amount" | "time";
export type RetentionUnit = "days" | "hours";
export type RetentionTimeUnit = RetentionUnit | TranscriptionRetentionUnit;

export type CloudSyncUiState = {
	enabled: boolean;
	autoPush: boolean;
	lastPushedAt: string | null;
	lastPulledAt: string | null;
	lastError: string | null;
	remoteRevision: string | null;
	posthogAnalyticsEnabled: boolean;
};

type CloudSyncReadableStore = Pick<Store, "get">;

type LoadCloudSyncStore = () => Promise<CloudSyncReadableStore>;

const loadSettingsStore: LoadCloudSyncStore = () => Store.load("settings.json");

const HOURS_PER_DAY = 24;
const MAX_RETENTION_DAYS = 36500;

const DEFAULT_CLOUD_SYNC_UI_STATE: CloudSyncUiState = {
	enabled: false,
	autoPush: true,
	lastPushedAt: null,
	lastPulledAt: null,
	lastError: null,
	remoteRevision: null,
	posthogAnalyticsEnabled: true,
};

function booleanOrDefault(value: unknown, fallback: boolean): boolean {
	return typeof value === "boolean" ? value : fallback;
}

function stringOrNull(value: unknown): string | null {
	return typeof value === "string" ? value : null;
}

/**
 * Read the small UI-only cloud-sync state from settings.json.
 *
 * This lives outside `DataSettings.tsx` so the component remains a UI Adapter:
 * it renders controls and calls mutations, while this Module owns store-shape
 * normalization and fallback policy.
 */
export async function readCloudSyncUiState(
	loadStore: LoadCloudSyncStore = loadSettingsStore,
): Promise<CloudSyncUiState> {
	const store = await loadStore();
	const enabled = booleanOrDefault(
		await store.get("cloud_sync_enabled"),
		DEFAULT_CLOUD_SYNC_UI_STATE.enabled,
	);
	const autoPush = booleanOrDefault(
		await store.get("cloud_sync_auto_push"),
		DEFAULT_CLOUD_SYNC_UI_STATE.autoPush,
	);
	const lastPushedAt = stringOrNull(
		await store.get("cloud_sync_last_pushed_at"),
	);
	const lastPulledAt = stringOrNull(
		await store.get("cloud_sync_last_pulled_at"),
	);
	const lastError = stringOrNull(await store.get("cloud_sync_last_error"));
	const remoteRevision = stringOrNull(
		await store.get("cloud_sync_remote_revision"),
	);
	const posthogAnalyticsEnabled = booleanOrDefault(
		await store.get("posthog_analytics_enabled"),
		DEFAULT_CLOUD_SYNC_UI_STATE.posthogAnalyticsEnabled,
	);

	return {
		enabled,
		autoPush,
		lastPushedAt,
		lastPulledAt,
		lastError,
		remoteRevision,
		posthogAnalyticsEnabled,
	};
}

export type CloudSyncDisplayState = CloudSyncUiState & {
	lastPushedLabel: string;
	lastPulledLabel: string;
	footerLabel: string;
	footerTone: "red" | "dimmed";
};

export function getCloudSyncDisplayState(
	state: CloudSyncUiState | null | undefined,
): CloudSyncDisplayState {
	const safeState = state ?? DEFAULT_CLOUD_SYNC_UI_STATE;

	return {
		...safeState,
		lastPushedLabel: safeState.lastPushedAt ?? "never",
		lastPulledLabel: safeState.lastPulledAt ?? "never",
		footerLabel: safeState.lastError
			? `Last error: ${safeState.lastError}`
			: `Revision: ${safeState.remoteRevision ?? "n/a"}`,
		footerTone: safeState.lastError ? "red" : "dimmed",
	};
}

export function retentionDaysFromUnitValue(args: {
	unit: RetentionUnit;
	value: number;
}): number {
	const value =
		typeof args.value === "number" && Number.isFinite(args.value)
			? Math.max(0, args.value)
			: 0;
	return args.unit === "hours" ? value / HOURS_PER_DAY : value;
}

export function preserveRetentionDurationOnUnitChange(args: {
	currentUnit: RetentionUnit;
	nextUnit: RetentionUnit;
	currentValue: number;
}): number {
	const current = Number.isFinite(args.currentValue) ? args.currentValue : 0;
	if (current === 0) return 0;
	if (args.currentUnit === "days" && args.nextUnit === "hours") {
		return current * HOURS_PER_DAY;
	}
	if (args.currentUnit === "hours" && args.nextUnit === "days") {
		return Math.round(current / HOURS_PER_DAY);
	}
	return current;
}

export type RetentionTimeInputConfig = {
	min: number;
	max: number;
	step: number;
	decimalScale: number;
};

export function getRetentionTimeInputConfig(
	unit: RetentionTimeUnit,
): RetentionTimeInputConfig {
	if (unit === "hours") {
		return {
			min: 0,
			max: MAX_RETENTION_DAYS * HOURS_PER_DAY,
			step: 0.5,
			decimalScale: 2,
		};
	}

	return {
		min: 0,
		max: MAX_RETENTION_DAYS,
		step: 1,
		decimalScale: 0,
	};
}

export function shouldDisableTranscriptionDeleteRecordings(args: {
	isProfileScope: boolean;
	mode: RetentionMode;
	amount: number;
	value: number;
}): boolean {
	if (args.isProfileScope) return true;
	return args.mode === "time" ? args.value === 0 : args.amount <= 0;
}

export type RecordingsStorageSummary = {
	count: number;
	gb: number;
};

export function summarizeRecordingsStorage(
	stats: RecordingsStats | null | undefined,
): RecordingsStorageSummary | null {
	if (!stats) return null;
	if (typeof stats.count !== "number" || !Number.isFinite(stats.count)) {
		return null;
	}
	if (typeof stats.bytes !== "number" || !Number.isFinite(stats.bytes)) {
		return null;
	}
	const count = Math.max(0, Math.round(stats.count));
	const bytes = Math.max(0, stats.bytes);

	return {
		count,
		gb: bytes / 1024 ** 3,
	};
}

export function describeRecordingsRetention(args: {
	isLoading: boolean;
	summary: RecordingsStorageSummary | null;
}): string {
	const base = "Keep at most this many recordings on disk.";
	if (args.isLoading) return `${base} (Calculating storage…)`;
	if (!args.summary) return base;
	return `${base} (Currently saved ${args.summary.count} recordings at ${args.summary.gb.toFixed(2)} GB)`;
}

export type DataStorageBreakdownItem = {
	label: string;
	value: string;
};

export function buildDataStorageBreakdown(args: {
	summary: DataStorageSummary;
	apiKeysSavedCount: number | null | undefined;
	apiKeyStoreKeyCount: number;
}): DataStorageBreakdownItem[] {
	const apiKeysSavedCount =
		typeof args.apiKeysSavedCount === "number" &&
		Number.isFinite(args.apiKeysSavedCount)
			? Math.max(0, Math.round(args.apiKeysSavedCount))
			: Math.max(0, Math.round(args.summary.api_keys_set_count));

	return [
		{
			label: "Recordings",
			value: `${args.summary.recordings_count} (${formatDataBytes(args.summary.recordings_bytes)})`,
		},
		{
			label: "Transcriptions",
			value: `${args.summary.history_count} (${formatDataBytes(args.summary.history_bytes)})`,
		},
		{
			label: "Request logs",
			value: `${args.summary.request_logs_count}`,
		},
		{
			label: "Usage/cost stats",
			value: `${args.summary.stats_files_count} files (${formatDataBytes(args.summary.stats_bytes)})`,
		},
		{
			label: "Settings",
			value: formatDataBytes(args.summary.settings_bytes),
		},
		{
			label: "API keys saved",
			value: `${apiKeysSavedCount} / ${args.apiKeyStoreKeyCount}`,
		},
	];
}

export function formatDataBytes(bytes: number): string {
	const b =
		typeof bytes === "number" && Number.isFinite(bytes)
			? Math.max(0, bytes)
			: 0;
	if (b < 1024) return `${Math.round(b)} B`;
	const kb = b / 1024;
	if (kb < 1024) return `${kb.toFixed(1)} KB`;
	const mb = kb / 1024;
	if (mb < 1024) return `${mb.toFixed(1)} MB`;
	const gb = mb / 1024;
	return `${gb.toFixed(2)} GB`;
}
