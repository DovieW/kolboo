import type { CostTimeframe } from "../../lib/tauri/types";
import type { ActivityDay } from "../../lib/tauri/types.generated";

export function formatAudioTime(seconds: number): string {
	if (seconds < 60) return `${Math.floor(seconds)}s`;
	const minutes = Math.floor(seconds / 60);
	return minutes < 60
		? `${minutes}m`
		: `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}

/** UTC date buckets, including quiet days, bounded for years of retained history. */
export function activitySeries(
	days: ActivityDay[],
	timeframe: CostTimeframe,
	now = new Date(),
) {
	const dayMs = 86_400_000;
	const end = Date.UTC(
		now.getUTCFullYear(),
		now.getUTCMonth(),
		now.getUTCDate(),
	);
	const span = { "24h": 2, "7d": 8, "30d": 31, "90d": 91 };
	const first = days[0];
	const start =
		timeframe === "all"
			? first
				? Date.parse(first.date)
				: end
			: end - (span[timeframe] - 1) * dayMs;
	const count = Math.max(1, Math.round((end - start) / dayMs) + 1);
	const step = Math.ceil(count / 45);
	const buckets = Array.from(
		{ length: Math.ceil(count / step) },
		(_, index) => ({
			date: new Date(start + index * step * dayMs).toISOString().slice(0, 10),
			endDate: new Date(Math.min(end, start + ((index + 1) * step - 1) * dayMs))
				.toISOString()
				.slice(0, 10),
			words: 0,
			audio_seconds: 0,
			recordings: 0,
		}),
	);
	for (const day of days) {
		const bucket =
			buckets[Math.floor((Date.parse(day.date) - start) / dayMs / step)];
		if (bucket) {
			bucket.words += day.totals.words;
			bucket.audio_seconds += day.totals.audio_seconds;
			bucket.recordings += day.totals.recordings;
		}
	}
	return buckets;
}
