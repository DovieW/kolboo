import { describe, expect, it } from "vitest";
import {
	normalizeMaxSavedRecordings,
	normalizeRequestLogsRetentionAmount,
	normalizeRequestLogsRetentionDays,
	normalizeRequestLogsRetentionMode,
	normalizeRetentionMode,
	normalizeStatsRetentionMaxBytes,
	normalizeTranscriptionRetentionAmount,
	normalizeTranscriptionRetentionDeleteRecordings,
	normalizeTranscriptionRetentionUnit,
	normalizeTranscriptionRetentionValue,
} from "./retention";

describe("retention settings normalizer", () => {
	it("normalizes saved recording counts with default, rounding, and defensive caps", () => {
		expect(normalizeMaxSavedRecordings(undefined)).toBe(1000);
		expect(normalizeMaxSavedRecordings(12.6)).toBe(13);
		expect(normalizeMaxSavedRecordings(0)).toBe(1);
		expect(normalizeMaxSavedRecordings(1_000_001)).toBe(100000);
	});

	it("normalizes retention units and preserves the current day-vs-hour semantics", () => {
		expect(normalizeTranscriptionRetentionUnit("days")).toBe("days");
		expect(normalizeTranscriptionRetentionUnit("hours")).toBe("hours");
		expect(normalizeTranscriptionRetentionUnit("weeks")).toBe("days");

		expect(normalizeTranscriptionRetentionValue(2.2, "days")).toBe(2);
		expect(normalizeTranscriptionRetentionValue(-5, "days")).toBe(0);
		expect(normalizeTranscriptionRetentionValue(0.5, "hours")).toBe(0.5);
		expect(normalizeTranscriptionRetentionValue(Number.NaN, "hours")).toBe(0);
		expect(normalizeTranscriptionRetentionValue(900_000, "hours")).toBe(
			36500 * 24,
		);
	});

	it("normalizes request-log retention defaults, clamps, and caller fallbacks", () => {
		expect(normalizeRequestLogsRetentionMode("amount")).toBe("amount");
		expect(normalizeRequestLogsRetentionMode("time")).toBe("time");
		expect(normalizeRequestLogsRetentionMode("banana")).toBe("amount");

		expect(normalizeRequestLogsRetentionAmount(undefined)).toBe(50);
		expect(normalizeRequestLogsRetentionAmount(5000)).toBe(1000);
		expect(normalizeRequestLogsRetentionAmount(0)).toBe(1);

		expect(normalizeRequestLogsRetentionDays(undefined)).toBe(7);
		expect(normalizeRequestLogsRetentionDays(-10)).toBe(0);
		expect(normalizeRequestLogsRetentionDays(40.8)).toBe(41);

		expect(normalizeRetentionMode("banana")).toBe("amount");
		expect(normalizeRetentionMode("banana", "time")).toBe("time");
	});

	it("normalizes transcription amounts, stats caps, and delete-recordings flags", () => {
		expect(normalizeTranscriptionRetentionAmount(undefined)).toBe(1000);
		expect(normalizeTranscriptionRetentionAmount(0)).toBe(1);
		expect(normalizeTranscriptionRetentionAmount(1000000)).toBe(100000);

		expect(normalizeStatsRetentionMaxBytes(undefined)).toBe(50_000_000);
		expect(normalizeStatsRetentionMaxBytes(100)).toBe(1_000_000);
		expect(normalizeStatsRetentionMaxBytes(6_000_000_000)).toBe(5_000_000_000);

		expect(normalizeTranscriptionRetentionDeleteRecordings(true)).toBe(true);
		expect(normalizeTranscriptionRetentionDeleteRecordings(false)).toBe(false);
		expect(normalizeTranscriptionRetentionDeleteRecordings("nope")).toBe(false);
	});
});
