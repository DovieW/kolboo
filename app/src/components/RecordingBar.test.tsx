import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { RecordingBar } from "./RecordingBar";

vi.mock("../lib/tauri/commands", () => ({
	recordingControlsAPI: {
		getSeconds: vi.fn(),
		canPause: vi.fn(),
		computerAudioAvailable: vi.fn(),
		getState: vi.fn(),
		listRecovery: vi.fn(),
		recover: vi.fn(),
		discardRecovery: vi.fn(),
		getPaused: vi.fn(),
		setPaused: vi.fn(),
		start: vi.fn(),
		stop: vi.fn(),
		cancel: vi.fn(),
	},
}));

function render(state?: string, paused = false) {
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	if (state) client.setQueryData(["home-recording-state"], state);
	client.setQueryData(["home-recording-paused"], paused);
	client.setQueryData(["recording-can-pause"], state === "recording");
	return renderToStaticMarkup(
		<QueryClientProvider client={client}>
			<MantineProvider>
				<RecordingBar />
			</MantineProvider>
		</QueryClientProvider>,
	);
}

describe("Home recording controls", () => {
	it("offers resume without hiding stop when capture is paused", () => {
		const html = render("recording", true);
		expect(html).toContain("Resume");
		expect(html).toContain("Stop &amp; transcribe");
	});
	it("offers recording from the idle backend state", () => {
		const html = render("idle");
		expect(html).toContain("Record a transcription");
		expect(html).not.toContain("Stop &amp; transcribe");
	});
	it("shows Stop for a recording started by another window or shortcut", () => {
		const html = render("recording");
		expect(html).toContain("Recording audio");
		expect(html).toContain("Stop &amp; transcribe");
		expect(html).toContain("Cancel");
	});
	it("keeps cancellation available during transcription", () => {
		const html = render("transcribing");
		expect(html).toContain("Cancel");
		expect(html).toContain("Recording pipeline busy");
	});
	it("does not advertise computer capture or recovery as available", () => {
		const html = render("idle");
		expect(html).toContain(
			"Computer audio is unavailable on this installation",
		);
		expect(html).toContain("saved locally for recovery");
		expect(html).toMatch(
			/type="checkbox"[^>]*disabled|disabled[^>]*type="checkbox"/,
		);
	});
});
