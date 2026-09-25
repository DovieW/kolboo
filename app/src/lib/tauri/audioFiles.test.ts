import { describe, expect, it } from "vitest";
import { audioFileName, isSupportedAudioFile } from "./audioFiles";

describe("local audio-file selection", () => {
	it("shows only the basename on Windows and Unix", () => {
		expect(audioFileName("C:\\Private\\meeting.wav")).toBe("meeting.wav");
		expect(audioFileName("/private/meeting.wav")).toBe("meeting.wav");
	});
	it("accepts known formats case-insensitively and rejects unrelated files", () => {
		for (const name of [
			"audio.WAV",
			"audio.mp3",
			"audio.flac",
			"audio.aac",
		])
			expect(isSupportedAudioFile(name)).toBe(true);
		for (const name of [
			"audio.mp3.txt",
			"audio",
			"audio.webm",
			"video.mp4",
			"audio.aiff",
			"audio.aif",
			"audio.ogg",
			"audio.oga",
			"audio.m4a",
		])
			expect(isSupportedAudioFile(name)).toBe(false);
	});
});
