import { describe, expect, it, vi } from "vitest";
import { RecordingPlayback } from "./playback";

class FakeAudio extends EventTarget {
	src = "";
	duration = 14400;
	currentTime = 0;
	playbackRate = 1;
	preload = "";
	paused = true;
	load = vi.fn(() => this.dispatchEvent(new Event("loadedmetadata")));
	play = vi.fn(async () => {
		this.paused = false;
		this.dispatchEvent(new Event("playing"));
	});
	pause() {
		this.paused = true;
		this.dispatchEvent(new Event("pause"));
	}
	getAttribute() {
		return this.src || null;
	}
	removeAttribute() {
		this.src = "";
	}
}
function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((r) => {
		resolve = r;
	});
	return { promise, resolve };
}
function setup(getUrl = async (id: string) => `asset:${id}`) {
	const audio = new FakeAudio();
	const player = new RecordingPlayback({
		createAudio: () => audio as unknown as HTMLAudioElement,
		getUrl,
		getWaveform: async () => ({ duration_seconds: 14400, peaks: [-1, 1] }),
	});
	return { audio, player };
}
describe("shared History playback", () => {
	it("shares pending analysis when reopening the same recording", async () => {
		const pending = deferred<{
			duration_seconds: number;
			peaks: number[];
		} | null>();
		const audio = new FakeAudio();
		const getWaveform = vi
			.fn()
			.mockImplementationOnce(() => pending.promise)
			.mockResolvedValue({ duration_seconds: 14400, peaks: [-1, 1] });
		const player = new RecordingPlayback({
			createAudio: () => audio as unknown as HTMLAudioElement,
			getUrl: async () => "media:one",
			getWaveform,
		});
		await player.prepare("one");
		player.stop();
		await player.prepare("one");
		pending.resolve({ duration_seconds: 14400, peaks: [-1, 1] });
		await vi.waitFor(() =>
			expect(player.snapshot().waveform?.peaks).toEqual([-1, 1]),
		);
		expect(player.snapshot().duration).toBe(14400);
		expect(player.snapshot().waveform?.peaks).toEqual([-1, 1]);
		await player.prepare("one");
		expect(getWaveform).toHaveBeenCalledTimes(1);
		expect(audio.play).not.toHaveBeenCalled();
		player.dispose();
	});
	it("loads playable ranges before the waveform, and ignores a stale analysis", async () => {
		const first = deferred<{
			duration_seconds: number;
			peaks: number[];
		} | null>();
		const second = deferred<{
			duration_seconds: number;
			peaks: number[];
		} | null>();
		const audio = new FakeAudio();
		const player = new RecordingPlayback({
			createAudio: () => audio as unknown as HTMLAudioElement,
			getUrl: async (id) => `media:${id}`,
			getWaveform: (id) => (id === "one" ? first.promise : second.promise),
		});
		await player.prepare("one");
		expect(player.snapshot()).toMatchObject({
			ready: true,
			waveform: null,
			duration: 14400,
		});
		await player.toggle("one");
		expect(audio.play).toHaveBeenCalledOnce();
		await player.prepare("two");
		first.resolve({ duration_seconds: 99, peaks: [-1, 1] });
		await first.promise;
		expect(player.snapshot()).toMatchObject({
			id: "two",
			waveform: null,
			duration: 14400,
		});
		second.resolve({ duration_seconds: 14400, peaks: [-0.5, 0.5] });
		await vi.waitFor(() =>
			expect(player.snapshot().waveform?.peaks).toEqual([-0.5, 0.5]),
		);
		expect(audio.play).toHaveBeenCalledOnce();
		player.dispose();
	});
	it("can still play when optional waveform analysis fails or has no peaks", async () => {
		for (const getWaveform of [
			async () => null,
			async () => {
				throw new Error("cache unavailable");
			},
		]) {
			const audio = new FakeAudio();
			audio.duration = Number.NaN;
			const player = new RecordingPlayback({
				createAudio: () => audio as unknown as HTMLAudioElement,
				getUrl: async () => "media:one",
				getWaveform,
			});
			await player.prepare("one");
			await player.toggle("one");
			expect(audio.play).toHaveBeenCalledOnce();
			expect(player.snapshot().error).toBeNull();
			player.dispose();
		}
	});
	it("stays unavailable until the media metadata has loaded", async () => {
		const { player, audio } = setup();
		audio.load.mockImplementationOnce(() => true);
		await player.prepare("one");
		expect(player.snapshot()).toMatchObject({ loading: true, ready: false });
		audio.dispatchEvent(new Event("loadedmetadata"));
		expect(player.snapshot()).toMatchObject({ loading: false, ready: true });
	});
	it("never autoplays, supports seeking/speed, and retains each position", async () => {
		const { player, audio } = setup();
		await player.prepare("one");
		expect(audio.play).not.toHaveBeenCalled();
		player.seek(4000);
		player.setRate(1.5);
		await player.toggle("one");
		expect(audio.playbackRate).toBe(1.5);
		expect(player.snapshot().playing).toBe(true);
		player.stop();
		expect(audio.paused).toBe(true);
		await player.prepare("two");
		player.seek(90000);
		expect(player.snapshot().position).toBe(14400);
		await player.prepare("one");
		audio.dispatchEvent(new Event("loadedmetadata"));
		expect(player.snapshot().position).toBe(4000);
	});
	it("a collapsed pending Play request cannot start after loading", async () => {
		const url = deferred<string>();
		const { player, audio } = setup(() => url.promise);
		const playing = player.toggle("one");
		player.stop();
		url.resolve("asset:one");
		await playing;
		expect(audio.play).not.toHaveBeenCalled();
		expect(audio.src).toBe("");
	});
	it("a stale recording load cannot replace a newer selection", async () => {
		const url = deferred<string>();
		const { player, audio } = setup((id) =>
			id === "one" ? url.promise : Promise.resolve("asset:two"),
		);
		const first = player.prepare("one");
		await player.prepare("two");
		url.resolve("asset:one");
		await first;
		expect(audio.src).toBe("asset:two");
		expect(player.snapshot().id).toBe("two");
		player.dispose();
		audio.dispatchEvent(new Event("playing"));
		expect(player.snapshot().playing).toBe(false);
	});
	it("ignores a late media error from the previous recording", async () => {
		const audios: FakeAudio[] = [];
		const onError = vi.fn();
		const player = new RecordingPlayback({
			createAudio: () => {
				const audio = new FakeAudio();
				audios.push(audio);
				return audio as unknown as HTMLAudioElement;
			},
			getUrl: async (id) => `media:${id}`,
			getWaveform: async () => ({
				duration_seconds: 10,
				peaks: [-1, 1],
			}),
			onError,
		});
		await player.prepare("one");
		await player.prepare("two");
		audios[0]?.dispatchEvent(new Event("error"));
		expect(player.snapshot().id).toBe("two");
		expect(player.snapshot().error).toBeNull();
		expect(onError).not.toHaveBeenCalled();
	});
	it("keeps useful missing audio errors without a timeout or autoplay", async () => {
		const { player, audio } = setup(async () => {
			throw new Error("gone");
		});
		await player.prepare("one");
		expect(player.snapshot().error).toContain("Could not load");
		expect(audio.play).not.toHaveBeenCalled();
	});
	it("leaves playback disabled when the recording no longer exists", async () => {
		const audio = new FakeAudio();
		const player = new RecordingPlayback({
			createAudio: () => audio as unknown as HTMLAudioElement,
			getUrl: async () => null,
			getWaveform: async () => null,
		});
		await player.prepare("deleted");
		expect(player.snapshot()).toMatchObject({
			loading: false,
			ready: false,
			error: "No saved audio is available for this recording.",
		});
		await player.toggle("deleted");
		expect(audio.play).not.toHaveBeenCalled();
		expect(audio.src).toBe("");
		player.dispose();
	});
	it("reports a repeated media failure only once", async () => {
		const audio = new FakeAudio();
		const onError = vi.fn();
		const player = new RecordingPlayback({
			createAudio: () => audio as unknown as HTMLAudioElement,
			getUrl: async () => "media:one",
			getWaveform: async () => ({ duration_seconds: 1, peaks: [-1, 1] }),
			onError,
		});
		await player.prepare("one");
		audio.dispatchEvent(new Event("error"));
		audio.dispatchEvent(new Event("error"));
		expect(onError).toHaveBeenCalledTimes(1);
	});
});
