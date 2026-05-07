import { describe, expect, it } from "vitest";
import {
	formatDataBytes,
	preserveRetentionDurationOnUnitChange,
	readCloudSyncUiState,
	retentionDaysFromUnitValue,
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
	});
});
