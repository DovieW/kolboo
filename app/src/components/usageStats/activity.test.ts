import { expect, it } from "vitest";
import { activitySeries, formatAudioTime } from "./activity";

const now = new Date("2026-09-23T12:00:00Z");
const totals = {
	words: 120,
	audio_seconds: 60,
	recordings: 1,
	timed_recordings: 1,
	meetings: 0,
};
it("formats short clips and long meetings without inventing precision", () => {
	expect([0, 12.9, 60, 3599, 3600, 7260].map(formatAudioTime)).toEqual([
		"0s",
		"12s",
		"1m",
		"59m",
		"1h 0m",
		"2h 1m",
	]);
});
it("includes empty dates and handles timeframes crossing a UTC date", () => {
	const days = [
		{ date: "2026-09-22", totals },
		{ date: "2026-09-23", totals },
	];
	expect(activitySeries(days, "24h", now).map((day) => day.words)).toEqual([
		120, 120,
	]);
	for (const [range, count] of [
		["7d", 8],
		["30d", 31],
	] as const) {
		const result = activitySeries(days, range, now);
		expect(result).toHaveLength(count);
		expect(result[0]?.words).toBe(0);
		expect(result.at(-1)?.date).toBe("2026-09-23");
	}
	expect(activitySeries([], "all", now)).toEqual([
		{
			date: "2026-09-23",
			endDate: "2026-09-23",
			words: 0,
			audio_seconds: 0,
			recordings: 0,
		},
	]);
});
it("bounds years of history while preserving totals and ignoring out-of-range rows", () => {
	const result = activitySeries(
		[
			{ date: "2020-01-01", totals },
			{ date: "2026-09-23", totals },
		],
		"all",
		now,
	);
	expect(result.length).toBeLessThanOrEqual(45);
	expect(result.reduce((sum, row) => sum + row.words, 0)).toBe(240);
	expect(result.at(-1)?.endDate).toBe("2026-09-23");
	const recent = activitySeries(
		[
			{ date: "2020-01-01", totals },
			{ date: "2026-09-23", totals },
		],
		"90d",
		now,
	);
	expect(recent.length).toBeLessThanOrEqual(45);
	expect(recent.reduce((sum, row) => sum + row.words, 0)).toBe(120);
});
