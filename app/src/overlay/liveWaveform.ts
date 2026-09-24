import type { OverlayAudioLevelPayload } from "../lib/tauri/types";

export const LIVE_WAVE_BARS = 32;
// This is display-only gain. The captured audio is never normalized here.
const VISUAL_FLOOR = 0.0003;
const MIN_CEILING = 0.009;
const HEADROOM = 2.3;
const CEILING_RELEASE = 0.995;

export interface LiveWaveformScale {
	ceiling: number;
}

export function createLiveWaveformScale(): LiveWaveformScale {
	return { ceiling: MIN_CEILING };
}

function amplitude(value: number | undefined) {
	return typeof value === "number" && Number.isFinite(value)
		? Math.min(1, Math.abs(value))
		: 0;
}

function displayAmplitude(level: number, ceiling: number) {
	const floor = Math.max(VISUAL_FLOOR, ceiling * 0.04);
	if (level <= floor) return 0;
	return Math.min(1, Math.log(level / floor) / Math.log(ceiling / floor));
}

/** Stationary bars with fast headroom for loud mics and slow release for quiet speech. */
export function liveWaveformBars(
	frame: OverlayAudioLevelPayload,
	scale: LiveWaveformScale = createLiveWaveformScale(),
): number[] {
	const { mins, maxes } = frame;
	const buckets =
		mins &&
		maxes &&
		mins.length > 0 &&
		mins.length <= 4096 &&
		mins.length === maxes.length;
	const levels = Array.from({ length: LIVE_WAVE_BARS }, (_, bar) => {
		if (buckets) {
			const start = Math.floor((bar * mins.length) / LIVE_WAVE_BARS);
			const end = Math.max(
				start + 1,
				Math.floor(((bar + 1) * mins.length) / LIVE_WAVE_BARS),
			);
			let level = 0;
			for (let bin = start; bin < end; bin++) {
				level = Math.max(level, amplitude(mins[bin]), amplitude(maxes[bin]));
			}
			return level;
		}
		return 0;
	});
	const fallback = Math.max(amplitude(frame.rms), amplitude(frame.peak) * 0.3);
	const bucketPeak = Math.max(...levels);
	// Some devices briefly report empty/stale waveform buckets even while the
	// level meter has live audio. Keep a responsive uniform meter in that case.
	const useBuckets =
		buckets &&
		bucketPeak > VISUAL_FLOOR &&
		bucketPeak >= amplitude(frame.rms) * 0.5;
	// A percentile ignores isolated sample spikes while preserving the shape of
	// each bar. Raise the ceiling on the very first loud frame, then release it
	// gradually so quieter moments still look quieter than recent speech.
	const sorted = useBuckets ? [...levels].sort((a, b) => a - b) : [];
	const representative = useBuckets
		? Math.max(
				sorted[Math.floor(LIVE_WAVE_BARS * 0.9)] as number,
				amplitude(frame.rms),
			)
		: fallback;
	scale.ceiling = Math.max(
		MIN_CEILING,
		scale.ceiling * CEILING_RELEASE,
		representative * HEADROOM,
	);
	return levels.map((level) =>
		displayAmplitude(useBuckets ? level : fallback, scale.ceiling),
	);
}
