import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { RecordingBar } from "./RecordingBar";

vi.mock("./MeetingModelDialog", () => ({ MeetingModelDialog: () => null }));

vi.mock("../lib/tauri/commands", () => ({
	recordingControlsAPI: {
		getPreferences: vi.fn(),
		setPreferences: vi.fn(),
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

function render(
	state?: string,
	paused = false,
	saved: string[] = [],
	historyOnly = false,
) {
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	if (state) client.setQueryData(["home-recording-state"], state);
	client.setQueryData(["home-recording-paused"], paused);
	client.setQueryData(
		["recording-can-pause"],
		historyOnly && state === "recording",
	);
	client.setQueryData(["recording-recovery"], saved);
	client.setQueryData(["recording-preferences"], {
		mode: "dictation",
		meeting_model: null,
	});
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
		const html = render("recording", true, [], true);
		expect(html).toContain("Resume");
		expect(html).toContain("Stop &amp; transcribe");
		expect(html).toContain('aria-label="Stop &amp; save for later"');
	});
	it("offers recording from the idle backend state", () => {
		const html = render("idle");
		expect(html).toContain('aria-label="Record"');
		expect(html).not.toContain(">Record<");
		expect(html).toContain('aria-label="Recording options"');
		expect(html).not.toContain("Stop &amp; transcribe");
	});
	it("shows Stop for a recording started by another window or shortcut", () => {
		const html = render("recording");
		expect(html).toContain("Recording duration");
		expect(html).toContain("Stop &amp; transcribe");
		expect(html).toContain("Cancel");
		expect(html).not.toContain("save for later");
		expect(html).not.toContain("Discard");
	});
	it("keeps cancellation available during transcription", () => {
		const html = render("transcribing");
		expect(html).toContain("Cancel");
		expect(html).toContain("Processing");
	});
	it("keeps options and recovery lists out of the compact closed bar", () => {
		const html = render("idle", false, ["one", "two", "three"]);
		expect(html).toContain(
			'aria-label="Recording options: 3 saved recordings"',
		);
		expect(html).not.toContain("Saved recording 1");
		expect(html).not.toContain("Computer audio");
		expect(html).not.toContain("30-second sections");
		expect(html.match(/<button\b/g)).toHaveLength(2);
	});
});
