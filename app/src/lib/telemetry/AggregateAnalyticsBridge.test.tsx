// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const {
	listeners,
	listenTypedMock,
	trackAggregateEventMock,
	getHistoryPageMock,
} = vi.hoisted(() => {
	const listeners = new Map<string, (payload: unknown) => void>();
	return {
		listeners,
		listenTypedMock: vi.fn(
			async (name: string, callback: (payload: unknown) => void) => {
				listeners.set(name, callback);
				return () => {
					listeners.delete(name);
				};
			},
		),
		trackAggregateEventMock: vi.fn(
			async (_event: string, _properties?: unknown) => undefined,
		),
		getHistoryPageMock: vi.fn(async () => ({
			items: [] as Array<Record<string, unknown>>,
		})),
	};
});

vi.mock("../tauri", () => ({
	tauriAPI: { getHistoryPage: getHistoryPageMock },
}));

vi.mock("../tauri/events", () => ({ listenTyped: listenTypedMock }));
vi.mock("./posthog", () => ({ trackAggregateEvent: trackAggregateEventMock }));

import { AggregateAnalyticsBridge } from "./AggregateAnalyticsBridge";

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	listeners.clear();
	listenTypedMock.mockReset();
	listenTypedMock.mockImplementation(async (name, callback) => {
		listeners.set(name, callback);
		return () => {
			listeners.delete(name);
		};
	});
	trackAggregateEventMock.mockClear();
	getHistoryPageMock.mockReset();
	getHistoryPageMock.mockResolvedValue({ items: [] });
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
});

afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
});

describe("AggregateAnalyticsBridge", () => {
	it("tracks only after disclosure, once per mount, and counts native events without payloads", async () => {
		await act(async () =>
			root.render(<AggregateAnalyticsBridge enabled={false} page="home" />),
		);
		expect(trackAggregateEventMock).not.toHaveBeenCalled();
		expect(listeners.size).toBe(0);

		await act(async () =>
			root.render(<AggregateAnalyticsBridge enabled page="home" />),
		);
		expect(trackAggregateEventMock).toHaveBeenCalledWith("desktop_opened");
		expect(trackAggregateEventMock).toHaveBeenCalledWith("page_viewed", {
			page: "home",
		});
		expect(listeners.size).toBe(5);

		await act(async () => {
			listeners.get("pipeline-recording-started")?.(undefined);
			listeners.get("pipeline-transcription-started")?.(undefined);
			listeners.get("pipeline-transcript-ready")?.("   ");
			listeners.get("pipeline-transcript-ready")?.("private transcript");
			listeners.get("pipeline-error")?.({ message: "private failure" });
		});
		expect(trackAggregateEventMock.mock.calls.map(([event]) => event)).toEqual([
			"desktop_opened",
			"page_viewed",
			"recording_started",
			"pipeline_transcription_started",
			"pipeline_transcript_ready",
			"pipeline_error",
		]);
		expect(
			trackAggregateEventMock.mock.calls
				.slice(2)
				.every((call) => call.length === 1),
		).toBe(true);

		await act(async () =>
			root.render(<AggregateAnalyticsBridge enabled page="settings" />),
		);
		expect(trackAggregateEventMock).toHaveBeenLastCalledWith("page_viewed", {
			page: "settings",
		});

		await act(async () =>
			root.render(<AggregateAnalyticsBridge enabled={false} page="settings" />),
		);
		expect(listeners.size).toBe(0);
		await act(async () =>
			root.render(<AggregateAnalyticsBridge enabled page="settings" />),
		);
		expect(trackAggregateEventMock).toHaveBeenCalledTimes(8);
	});

	it("counts new or completed history entries but not pre-existing recordings or edits", async () => {
		getHistoryPageMock.mockResolvedValueOnce({
			items: [{ id: "old", status: "success" }],
		});
		await act(async () =>
			root.render(<AggregateAnalyticsBridge enabled page="home" />),
		);
		getHistoryPageMock.mockResolvedValueOnce({
			items: [
				{
					id: "new",
					status: "success",
					recording_mode: "meeting",
					duration_seconds: 83.4,
					stt_provider: "openai",
					stt_model: "gpt-4o-transcribe-diarize",
				},
				{ id: "old", status: "success" },
			],
		});
		await act(async () => listeners.get("history-changed")?.(undefined));
		expect(trackAggregateEventMock).toHaveBeenCalledWith("recording_finished", {
			status: "success",
			mode: "meeting",
			duration_seconds: 83.4,
			stt_provider: "openai",
			stt_model: "gpt-4o-transcribe-diarize",
		});
		const count = trackAggregateEventMock.mock.calls.length;
		await act(async () => listeners.get("history-changed")?.(undefined));
		expect(trackAggregateEventMock).toHaveBeenCalledTimes(count);
	});

	it("rechecks history when a change arrives during a fetch", async () => {
		let resolveFirst: (value: {
			items: Array<Record<string, unknown>>;
		}) => void = () => {};
		getHistoryPageMock.mockImplementationOnce(
			() =>
				new Promise((resolve) => {
					resolveFirst = resolve;
				}),
		);
		getHistoryPageMock.mockResolvedValueOnce({
			items: [{ id: "new", status: "error" }],
		});
		await act(async () =>
			root.render(<AggregateAnalyticsBridge enabled page="home" />),
		);
		await act(async () => listeners.get("history-changed")?.(undefined));
		await act(async () => resolveFirst({ items: [] }));
		expect(getHistoryPageMock).toHaveBeenCalledTimes(2);
		expect(trackAggregateEventMock).toHaveBeenCalledWith(
			"recording_finished",
			expect.objectContaining({ status: "error" }),
		);
	});

	it("drops a late history result after observation is disabled", async () => {
		let resolveFirst: (value: {
			items: Array<Record<string, unknown>>;
		}) => void = () => {};
		getHistoryPageMock.mockImplementationOnce(
			() =>
				new Promise((resolve) => {
					resolveFirst = resolve;
				}),
		);
		await act(async () =>
			root.render(<AggregateAnalyticsBridge enabled page="home" />),
		);
		await act(async () => listeners.get("history-changed")?.(undefined));
		await act(async () =>
			root.render(<AggregateAnalyticsBridge enabled={false} page="home" />),
		);
		await act(async () =>
			resolveFirst({ items: [{ id: "private", status: "success" }] }),
		);
		expect(trackAggregateEventMock.mock.calls.map(([event]) => event)).toEqual([
			"desktop_opened",
			"page_viewed",
		]);
	});

	it("cleans up a late subscription and tolerates listener failure", async () => {
		let resolveListen: (stop: () => void) => void = () => {};
		const stop = vi.fn();
		listenTypedMock.mockImplementationOnce(
			() =>
				new Promise((resolve) => {
					resolveListen = resolve;
				}),
		);
		await act(async () =>
			root.render(<AggregateAnalyticsBridge enabled page="home" />),
		);
		await act(async () =>
			root.render(<AggregateAnalyticsBridge enabled={false} page="home" />),
		);
		await act(async () => resolveListen(stop));
		expect(stop).toHaveBeenCalledOnce();
		listenTypedMock.mockImplementationOnce(async () => {
			throw new Error("native listener unavailable");
		});
		await act(async () =>
			root.render(<AggregateAnalyticsBridge enabled page="home" />),
		);
		expect(trackAggregateEventMock).toHaveBeenCalledWith("page_viewed", {
			page: "home",
		});
	});
});
