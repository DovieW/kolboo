// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { OverlayAudioLevelPayload } from "../lib/tauri/types";
import { BackendAudioWave } from "./BackendAudioWave";
import {
	createLiveWaveformScale,
	LIVE_WAVE_BARS,
	liveWaveformBars,
} from "./liveWaveform";

const listen = vi.hoisted(() => vi.fn());
vi.mock("../lib/tauri/events", () => ({ listenTyped: listen }));
const frame = (
	patch: Partial<OverlayAudioLevelPayload> = {},
): OverlayAudioLevelPayload => ({ seq: 1, rms: 0, peak: 0, ...patch });
const host = document.createElement("div");
let root = createRoot(host);
beforeEach(() => {
	vi.useFakeTimers();
	listen.mockResolvedValue(vi.fn());
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
});
afterEach(async () => {
	await act(async () => root.unmount());
	root = createRoot(host);
	vi.resetAllMocks();
	vi.useRealTimers();
});
it("leaves silence flat, limits noise gain, and retains actual channel shape", () => {
	expect(liveWaveformBars(frame())).toEqual(Array(LIVE_WAVE_BARS).fill(0));
	expect(liveWaveformBars(frame({ rms: 0.0002 }))).toEqual(
		Array(LIVE_WAVE_BARS).fill(0),
	);
	const bars = liveWaveformBars(
		frame({ mins: [-0.0008, -0.002], maxes: [0.0008, 0.006] }),
	);
	expect(bars).toHaveLength(32);
	expect(bars[0]).toBeGreaterThan(0);
	expect(bars[0]).toBeLessThan(bars[31] ?? 0);
	expect(bars.every((bar) => bar >= 0 && bar <= 1)).toBe(true);
});
it("sanitizes malformed frames and never invents moving waves or amplifies NaN", () => {
	for (const patch of [
		{ rms: Number.NaN },
		{ mins: [], maxes: [] },
		{ mins: [1], maxes: [] },
		{ mins: Array(4097).fill(1), maxes: Array(4097).fill(1) },
		{ mins: [Number.NaN], maxes: [Number.POSITIVE_INFINITY] },
	])
		expect(liveWaveformBars(frame(patch))).toEqual(Array(32).fill(0));
	expect(
		liveWaveformBars(frame({ mins: [-9], maxes: [2] })).every(
			(bar) => bar > 0.5 && bar < 0.85,
		),
	).toBe(true);
	expect(liveWaveformBars(frame({ rms: 0.1 }))[0]).toBeGreaterThan(0.5);
	const speaking = liveWaveformBars(frame({ rms: 0.003 }))[0] ?? 0;
	expect(speaking).toBeGreaterThan(0.6);
	expect(speaking).toBeLessThan(0.7);
});
it("adapts visual headroom to a louder mic without pinning every bar", () => {
	const scale = createLiveWaveformScale();
	const quiet = liveWaveformBars(frame({ rms: 0.003 }), scale)[0] ?? 0;
	const loud = liveWaveformBars(frame({ rms: 0.15 }), scale)[0] ?? 0;
	expect(quiet).toBeGreaterThan(0.6);
	expect(loud).toBeGreaterThan(quiet);
	expect(loud).toBeLessThan(0.85);
	expect(liveWaveformBars(frame({ rms: 0.15 }), scale)[0]).toBeCloseTo(loud);
	expect(liveWaveformBars(frame({ rms: 0.003 }), scale)[0]).toBeLessThan(quiet);
	expect(
		liveWaveformBars(frame({ rms: 0.003 }), createLiveWaveformScale())[0],
	).toBeCloseTo(quiet);
});
it("uses typical waveform peaks for gain while retaining a single loud transient", () => {
	const levels = Array(64).fill(0.03);
	levels[0] = 1;
	const bars = liveWaveformBars(
		frame({ rms: 0.025, mins: levels.map((level) => -level), maxes: levels }),
	);
	expect(bars[0]).toBe(1);
	expect(bars[1]).toBeGreaterThan(0.5);
	expect(bars[1]).toBeLessThan(0.85);
});
it("keeps low-output speech visible and falls back when waveform buckets are stale", () => {
	const lowVoice = liveWaveformBars(
		frame({ rms: 0.0015, mins: [-0.002], maxes: [0.002] }),
	);
	expect(lowVoice[0]).toBeGreaterThan(0.5);
	const emptyBuckets = liveWaveformBars(
		frame({ rms: 0.003, peak: 0.006, mins: [0, 0], maxes: [0, 0] }),
	);
	expect(emptyBuckets.every((bar) => bar > 0.6)).toBe(true);
	const staleBuckets = liveWaveformBars(
		frame({ rms: 0.003, mins: [-0.0004], maxes: [0.0004] }),
	);
	expect(staleBuckets[0]).toBeCloseTo(emptyBuckets[0] ?? 0);
	expect(liveWaveformBars(frame({ rms: 0, peak: 0.0002 }))).toEqual(
		Array(LIVE_WAVE_BARS).fill(0),
	);
});
it("renders stationary bars immediately, settles on stalled audio, and does no hidden work", async () => {
	await act(async () => root.render(<BackendAudioWave isActive />));
	const bars = [...host.querySelectorAll("rect")];
	expect(bars).toHaveLength(32);
	expect(bars.every((bar) => bar.getAttribute("y") === "-11")).toBe(true);
	expect(bars.every((bar) => bar.getAttribute("height") === "22")).toBe(true);
	expect(bars.every((bar) => bar.style.transformBox === "fill-box")).toBe(true);
	expect(bars.every((bar) => bar.style.transform === "scaleY(0.07)")).toBe(
		true,
	);
	const positions = [...host.querySelectorAll("g")].map((group) =>
		group.getAttribute("transform"),
	);
	await act(async () => listen.mock.calls[0]?.[1](frame({ rms: 0.1 })));
	expect(bars[0]?.style.transform).not.toBe("scaleY(0.07)");
	expect(
		[...host.querySelectorAll("g")].map((group) =>
			group.getAttribute("transform"),
		),
	).toEqual(positions);
	expect(bars[0]?.style.transition).toBe("transform 180ms ease-out");
	await act(async () => vi.advanceTimersByTime(350));
	expect(bars[0]?.style.transform).toBe("scaleY(0.07)");
	await act(async () => listen.mock.calls[0]?.[1](frame({ rms: 0.003 })));
	expect(
		Number.parseFloat(bars[0]?.style.transform.match(/\((.*)\)/)?.[1] ?? "0"),
	).toBeGreaterThan(0.6);
	await act(async () => listen.mock.calls[0]?.[1](frame({ rms: 0.1 })));
	await act(async () =>
		root.render(
			<BackendAudioWave isActive isVisible={false} className="extra" />,
		),
	);
	expect(host.querySelector("svg")?.classList.contains("extra")).toBe(true);
	expect(bars[0]?.style.transform).toBe("scaleY(0.07)");
	expect(vi.getTimerCount()).toBe(0);
});
it("shows mirrored inward and outward processing pulses without opening the mic", async () => {
	await act(async () =>
		root.render(<BackendAudioWave isActive={false} isProcessing />),
	);
	const bars = [...host.querySelectorAll<SVGRectElement>(".overlay-wave-base")];
	const pulses = [
		...host.querySelectorAll<SVGRectElement>(".overlay-wave-pulse"),
	];
	expect(host.querySelector(".overlay-wave--processing")).not.toBeNull();
	expect(bars.every((bar) => bar.style.transform === "scaleY(0.08)")).toBe(
		true,
	);
	expect(pulses).toHaveLength(LIVE_WAVE_BARS * 2);
	const delay = (bar: number, pass: number) =>
		Number.parseFloat(pulses[bar * 2 + pass]?.style.animationDelay ?? "0");
	expect(delay(0, 0)).toBe(0);
	expect(delay(31, 0)).toBe(0);
	expect(delay(15, 0)).toBeGreaterThan(delay(0, 0));
	expect(delay(15, 0)).toBeCloseTo(delay(16, 0));
	expect(delay(15, 1)).toBeLessThan(delay(0, 1));
	expect(delay(0, 1)).toBeCloseTo(delay(31, 1));
	expect(listen).not.toHaveBeenCalled();
	await act(async () =>
		root.render(<BackendAudioWave isActive={false} isProcessing={false} />),
	);
	expect(host.querySelector(".overlay-wave--processing")).toBeNull();
	expect(bars[0]?.style.transform).toBe("scaleY(0.07)");
	expect(host.querySelectorAll(".overlay-wave-pulse")).toHaveLength(0);
});
