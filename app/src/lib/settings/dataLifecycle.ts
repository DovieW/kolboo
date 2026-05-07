import { Store } from "@tauri-apps/plugin-store";
import type { RecordingsStats } from "../tauri/types";

export type RequestLogsRetentionMode = "amount" | "time";
export type RetentionMode = "amount" | "time";
export type RetentionUnit = "days" | "hours";

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
	const enabled = (await store.get<boolean>("cloud_sync_enabled")) ?? false;
	const autoPush = (await store.get<boolean>("cloud_sync_auto_push")) ?? true;
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
	const posthogAnalyticsEnabled =
		(await store.get<boolean>("posthog_analytics_enabled")) ?? true;

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

export function retentionDaysFromUnitValue(args: {
	unit: RetentionUnit;
	value: number;
}): number {
	return args.unit === "hours" ? args.value / 24 : Math.max(0, args.value);
}

export function preserveRetentionDurationOnUnitChange(args: {
	currentUnit: RetentionUnit;
	nextUnit: RetentionUnit;
	currentValue: number;
}): number {
	const current = Number.isFinite(args.currentValue) ? args.currentValue : 0;
	if (current === 0) return 0;
	if (args.currentUnit === "days" && args.nextUnit === "hours") {
		return current * 24;
	}
	if (args.currentUnit === "hours" && args.nextUnit === "days") {
		return Math.round(current / 24);
	}
	return current;
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

	return {
		count: Math.max(0, Math.round(stats.count)),
		gb: stats.bytes / 1024 ** 3,
	};
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
