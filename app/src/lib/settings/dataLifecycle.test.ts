import { describe, expect, it } from "vitest";
import {
	buildDataStorageBreakdown,
	describeRecordingsRetention,
	formatDataBytes,
	getCloudSyncDisplayState,
	getRetentionTimeInputConfig,
	preserveRetentionDurationOnUnitChange,
	readCloudSyncUiState,
	retentionDaysFromUnitValue,
	shouldDisableTranscriptionDeleteRecordings,
	summarizeRecordingsStorage,
} from "./dataLifecycle";

function fakeStore(values: Record<string, unknown>) {
	return async () => ({
		get: async <T>(key: string): Promise<T | undefined> => values[key] as T,
	});
}

describe("Data Lifecycle read model", () => {
	it("normalizes missing cloud sync state to safe UI defaults", async () => {
		await expect(readCloudSyncUiState(fakeStore({}))).resolves.toEqual({
			enabled: false,
			autoPush: true,
			lastPushedAt: null,
			lastPulledAt: null,
			lastError: null,
			remoteRevision: null,
			posthogAnalyticsEnabled: true,
			telemetryDisclosureAcknowledgedAt: null,
			telemetryDisclosureVersion: null,
		});
	});

	it("drops malformed cloud sync timestamps and preserves booleans", async () => {
		await expect(
			readCloudSyncUiState(
				fakeStore({
					cloud_sync_enabled: true,
					cloud_sync_auto_push: false,
					cloud_sync_last_pushed_at: 123,
					cloud_sync_last_pulled_at: "2026-05-07T00:00:00Z",
					cloud_sync_last_error: "offline",
					cloud_sync_remote_revision: "rev-1",
					posthog_analytics_enabled: false,
					telemetry_disclosure_acknowledged_at: "2026-05-13T00:00:00Z",
					telemetry_disclosure_version: "2026-05-phase6b-v1",
				}),
			),
		).resolves.toMatchObject({
			enabled: true,
			autoPush: false,
			lastPushedAt: null,
			lastPulledAt: "2026-05-07T00:00:00Z",
			lastError: "offline",
			remoteRevision: "rev-1",
			posthogAnalyticsEnabled: false,
			telemetryDisclosureAcknowledgedAt: "2026-05-13T00:00:00Z",
			telemetryDisclosureVersion: "2026-05-phase6b-v1",
		});
	});

	it("falls back when cloud sync booleans are malformed", async () => {
		await expect(
			readCloudSyncUiState(
				fakeStore({
					cloud_sync_enabled: "yes",
					cloud_sync_auto_push: 123,
					posthog_analytics_enabled: "nope",
					telemetry_disclosure_acknowledged_at: 123,
					telemetry_disclosure_version: false,
				}),
			),
		).resolves.toMatchObject({
			enabled: false,
			autoPush: true,
			posthogAnalyticsEnabled: true,
			telemetryDisclosureAcknowledgedAt: null,
			telemetryDisclosureVersion: null,
		});
	});

	it("builds cloud sync display labels from state and defaults", () => {
		expect(getCloudSyncDisplayState(undefined)).toMatchObject({
			lastPushedLabel: "never",
			lastPulledLabel: "never",
			footerLabel: "Revision: n/a",
			footerTone: "dimmed",
			telemetryDisclosureResolved: false,
		});

		expect(
			getCloudSyncDisplayState({
				enabled: true,
				autoPush: false,
				lastPushedAt: "2026-05-07T01:00:00Z",
				lastPulledAt: null,
				lastError: "offline",
				remoteRevision: "rev-42",
				posthogAnalyticsEnabled: false,
				telemetryDisclosureAcknowledgedAt: "2026-05-13T00:00:00Z",
				telemetryDisclosureVersion: "2026-05-phase6b-v1",
			}),
		).toMatchObject({
			lastPushedLabel: "2026-05-07T01:00:00Z",
			lastPulledLabel: "never",
			footerLabel: "Last error: offline",
			footerTone: "red",
			telemetryDisclosureResolved: true,
		});
	});

	it("preserves retention duration when toggling days and hours", () => {
		expect(
			preserveRetentionDurationOnUnitChange({
				currentUnit: "days",
				nextUnit: "hours",
				currentValue: 2,
			}),
		).toBe(48);
		expect(
			preserveRetentionDurationOnUnitChange({
				currentUnit: "hours",
				nextUnit: "days",
				currentValue: 49,
			}),
		).toBe(2);
	});

	it("converts retention unit/value pairs into days for backend writes", () => {
		expect(retentionDaysFromUnitValue({ unit: "hours", value: 12 })).toBe(0.5);
		expect(retentionDaysFromUnitValue({ unit: "days", value: -2 })).toBe(0);
		expect(
			retentionDaysFromUnitValue({ unit: "hours", value: Number.NaN }),
		).toBe(0);
	});

	it("returns stable input configs for retention time controls", () => {
		expect(getRetentionTimeInputConfig("days")).toEqual({
			min: 0,
			max: 36500,
			step: 1,
			decimalScale: 0,
		});
		expect(getRetentionTimeInputConfig("hours")).toEqual({
			min: 0,
			max: 876000,
			step: 0.5,
			decimalScale: 2,
		});
	});

	it("describes recordings retention from loading and summary state", () => {
		expect(
			describeRecordingsRetention({ isLoading: true, summary: null }),
		).toContain("Calculating storage");
		expect(
			describeRecordingsRetention({ isLoading: false, summary: null }),
		).toBe("Keep at most this many recordings on disk.");
		expect(
			describeRecordingsRetention({
				isLoading: false,
				summary: { count: 7, gb: 1.234 },
			}),
		).toBe(
			"Keep at most this many recordings on disk. (Currently saved 7 recordings at 1.23 GB)",
		);
	});

	it("computes when transcription retention may delete recordings", () => {
		expect(
			shouldDisableTranscriptionDeleteRecordings({
				isProfileScope: true,
				mode: "time",
				amount: 10,
				value: 7,
			}),
		).toBe(true);
		expect(
			shouldDisableTranscriptionDeleteRecordings({
				isProfileScope: false,
				mode: "time",
				amount: 10,
				value: 0,
			}),
		).toBe(true);
		expect(
			shouldDisableTranscriptionDeleteRecordings({
				isProfileScope: false,
				mode: "amount",
				amount: 5,
				value: 0,
			}),
		).toBe(false);
	});

	it("formats storage sizes and summarizes recordings defensively", () => {
		expect(formatDataBytes(512)).toBe("512 B");
		expect(formatDataBytes(1536)).toBe("1.5 KB");
		expect(formatDataBytes(5 * 1024 ** 2)).toBe("5.0 MB");
		expect(formatDataBytes(3 * 1024 ** 3)).toBe("3.00 GB");
		expect(
			summarizeRecordingsStorage({ count: 2.4, bytes: 1024 ** 3 }),
		).toEqual({
			count: 2,
			gb: 1,
		});
		expect(
			summarizeRecordingsStorage({ count: Number.NaN, bytes: 1 }),
		).toBeNull();
		expect(summarizeRecordingsStorage({ count: 2, bytes: -1024 })).toEqual({
			count: 2,
			gb: 0,
		});
	});

	it("builds danger-zone storage rows with api-key fallback rules", () => {
		expect(
			buildDataStorageBreakdown({
				summary: {
					recordings_count: 4,
					recordings_bytes: 2 * 1024 ** 3,
					history_count: 9,
					history_bytes: 5 * 1024 ** 2,
					request_logs_count: 12,
					stats_files_count: 3,
					stats_bytes: 6 * 1024,
					settings_bytes: 1536,
					api_keys_set_count: 1,
				},
				apiKeysSavedCount: undefined,
				apiKeyStoreKeyCount: 5,
			}),
		).toEqual([
			{ label: "Recordings", value: "4 (2.00 GB)" },
			{ label: "Transcriptions", value: "9 (5.0 MB)" },
			{ label: "Request logs", value: "12" },
			{ label: "Usage/cost stats", value: "3 files (6.0 KB)" },
			{ label: "Settings", value: "1.5 KB" },
			{ label: "API keys saved", value: "1 / 5" },
		]);

		expect(
			buildDataStorageBreakdown({
				summary: {
					recordings_count: 0,
					recordings_bytes: 0,
					history_count: 0,
					history_bytes: 0,
					request_logs_count: 0,
					stats_files_count: 0,
					stats_bytes: 0,
					settings_bytes: 0,
					api_keys_set_count: 1,
				},
				apiKeysSavedCount: 4,
				apiKeyStoreKeyCount: 5,
			}).at(-1),
		).toEqual({ label: "API keys saved", value: "4 / 5" });
	});
});
