// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import QuickAskApp from "./QuickAskApp";

const mocks = vi.hoisted(() => ({
	events: new Map<string, (payload: unknown) => void>(),
	stop: vi.fn(),
	invoke: vi.fn(async () => ({ profile_id: null as string | null })),
	api: {
		cancelQuickAsk: vi.fn(async (_id: string) => {}),
		setQuickAskEscapeEnabled: vi.fn(async (_enabled: boolean) => {}),
		reloadSettingsFromDisk: vi.fn(async () => {}),
		getSettings: vi.fn(async () => ({
			quick_ask_dismiss_mode: "manual",
			rewrite_program_prompt_profiles: [] as Array<{
				id: string;
				quick_ask_dismiss_mode: "auto" | "manual";
			}>,
		})),
		onSettingsChanged: vi.fn(async (_callback: () => void) => () => {}),
	},
	win: {
		hide: vi.fn(async () => {}),
		show: vi.fn(async () => {}),
		setFocus: vi.fn(async () => {}),
		setAlwaysOnTop: vi.fn(async () => {}),
		setIgnoreCursorEvents: vi.fn(async () => {}),
		outerPosition: vi.fn(async () => ({ x: 0, y: 0 })),
		outerSize: vi.fn(async () => ({ width: 520, height: 440 })),
		scaleFactor: vi.fn(async () => 1),
		setSize: vi.fn(async () => {}),
		setPosition: vi.fn(async () => {}),
	},
}));
vi.mock("./hooks/useModifierKeyForwarder", () => ({
	useModifierKeyForwarder: () => {},
}));
vi.mock("./lib/tauri", () => ({ tauriAPI: mocks.api }));
vi.mock("./lib/tauri/events", () => ({
	listenTyped: vi.fn(
		async (name: string, callback: (payload: unknown) => void) => {
			mocks.events.set(name, callback);
			return () => {
				mocks.events.delete(name);
				mocks.stop(name);
			};
		},
	),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/plugin-opener", () => ({
	openUrl: vi.fn(async () => {}),
}));
vi.mock("@tauri-apps/api/window", () => ({
	getCurrentWindow: () => mocks.win,
	LogicalSize: class {
		constructor(
			public width: number,
			public height: number,
		) {}
	},
	PhysicalPosition: class {
		constructor(
			public x: number,
			public y: number,
		) {}
	},
}));

let host: HTMLDivElement;
let root: ReturnType<typeof createRoot>;
beforeEach(async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	vi.clearAllMocks();
	mocks.invoke.mockResolvedValue({ profile_id: null });
	mocks.api.getSettings.mockResolvedValue({
		quick_ask_dismiss_mode: "manual",
		rewrite_program_prompt_profiles: [],
	});
	mocks.events.clear();
	host = document.createElement("div");
	document.body.appendChild(host);
	root = createRoot(host);
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<QuickAskApp />
			</MantineProvider>,
		),
	);
});

it("contains native setup failures and accepts legacy or malformed start notifications safely", async () => {
	const warning = vi.spyOn(console, "warn").mockImplementation(() => {});
	mocks.invoke.mockRejectedValueOnce(new Error("private details must not log"));
	for (const callback of [
		mocks.api.setQuickAskEscapeEnabled,
		mocks.win.setAlwaysOnTop,
		mocks.win.show,
		mocks.win.setFocus,
	])
		callback.mockRejectedValueOnce(new Error("native unavailable"));
	await event("quick-ask-started", null);
	expect(host.textContent).toContain("Thinking…");
	expect(warning).toHaveBeenCalledWith(
		"Quick Ask: failed to resolve active profile",
	);
	await event("quick-ask-answer", { ok: true, answer: "Legacy answer" });
	expect(host.textContent).toContain("Legacy answer");
	warning.mockRestore();
});

it("does not let delayed profile lookups or old dismissal animations affect the next answer", async () => {
	mocks.api.getSettings.mockResolvedValue({
		quick_ask_dismiss_mode: "manual",
		rewrite_program_prompt_profiles: [
			{ id: "old", quick_ask_dismiss_mode: "auto" },
		],
	});
	await act(async () => mocks.api.onSettingsChanged.mock.calls[0]?.[0]());
	let resolve!: (value: { profile_id: string | null }) => void;
	mocks.invoke.mockImplementationOnce(
		() =>
			new Promise((done) => {
				resolve = done;
			}),
	);
	await event("quick-ask-started", { request_id: "one" });
	vi.useFakeTimers();
	await event("quick-ask-dismiss-requested");
	// A new start before the animation frame must invalidate that old frame.
	await event("quick-ask-started", { request_id: "two" });
	await act(async () => resolve({ profile_id: "old" }));
	await act(async () => vi.advanceTimersByTimeAsync(500));
	await act(async () => window.dispatchEvent(new Event("blur")));
	expect(mocks.api.cancelQuickAsk).not.toHaveBeenCalledWith("two");
	expect(mocks.win.hide).not.toHaveBeenCalled();
	// Also cover a start after the frame but before its delayed hide.
	await event("quick-ask-dismiss-requested");
	await act(async () => vi.advanceTimersByTimeAsync(20));
	await event("quick-ask-started", { request_id: "three" });
	await act(async () => vi.advanceTimersByTimeAsync(500));
	expect(mocks.win.hide).not.toHaveBeenCalled();
	await event("quick-ask-partial", {
		request_id: "three",
		seq: 1,
		text: "Current answer",
	});
	expect(host.textContent).toContain("Current answer");
});

it("ignores delayed native hide completions after another answer starts", async () => {
	for (const trigger of ["recording-start", "quick-ask-dismiss-requested"]) {
		await event("quick-ask-started", { request_id: "old" });
		let resolve!: () => void;
		mocks.win.hide.mockImplementationOnce(
			() =>
				new Promise<void>((done) => {
					resolve = done;
				}),
		);
		vi.useFakeTimers();
		await event(trigger);
		if (trigger === "quick-ask-dismiss-requested")
			await act(async () => vi.advanceTimersByTimeAsync(500));
		await event("quick-ask-started", { request_id: "new" });
		await event("quick-ask-partial", {
			request_id: "new",
			seq: 1,
			text: "Current content",
		});
		await act(async () => resolve());
		expect(host.textContent).toContain("Current content");
		expect(host.textContent).toContain("Answering…");
		vi.useRealTimers();
	}
});

it("an old failed profile lookup cannot erase the next answer's dismissal preference", async () => {
	const warning = vi.spyOn(console, "warn").mockImplementation(() => {});
	mocks.api.getSettings.mockResolvedValue({
		quick_ask_dismiss_mode: "manual",
		rewrite_program_prompt_profiles: [
			{ id: "current", quick_ask_dismiss_mode: "auto" },
		],
	});
	await act(async () => mocks.api.onSettingsChanged.mock.calls[0]?.[0]());
	let reject!: (error: Error) => void;
	mocks.invoke.mockImplementationOnce(
		() =>
			new Promise((_resolve, fail) => {
				reject = fail;
			}),
	);
	await event("quick-ask-started", { request_id: "old" });
	mocks.invoke.mockResolvedValueOnce({ profile_id: "current" });
	await event("quick-ask-started", { request_id: "new" });
	await act(async () => reject(new Error("private details")));
	await event("quick-ask-answer", {
		request_id: "new",
		ok: true,
		answer: "Current answer",
	});
	vi.useFakeTimers();
	await act(async () => window.dispatchEvent(new Event("blur")));
	await act(async () => vi.advanceTimersByTimeAsync(500));
	expect(mocks.win.hide).toHaveBeenCalledOnce();
	expect(warning).toHaveBeenCalledExactlyOnceWith(
		"Quick Ask: failed to resolve active profile",
	);
	warning.mockRestore();
});
afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
	vi.useRealTimers();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});
async function event(name: string, payload: unknown = null) {
	expect(mocks.events.has(name)).toBe(true);
	await act(async () => mocks.events.get(name)?.(payload));
}

it("observes each remounted answer panel and resizes while streamed text grows", async () => {
	const observers: Array<{
		callback: () => void;
		node?: Element;
		disconnected: boolean;
	}> = [];
	vi.stubGlobal(
		"ResizeObserver",
		class {
			entry: (typeof observers)[number];
			constructor(callback: () => void) {
				this.entry = { callback, disconnected: false };
				observers.push(this.entry);
			}
			observe(node: Element) {
				this.entry.node = node;
			}
			unobserve(node: Element) {
				if (this.entry.node === node) this.entry.node = undefined;
			}
			disconnect() {
				this.entry.disconnected = true;
			}
		},
	);
	let height = 100;
	vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
		function (this: HTMLElement) {
			return new DOMRect(
				0,
				0,
				520,
				this.classList.contains("quick-ask-panel") ? height : 0,
			);
		},
	);
	await event("quick-ask-started", { request_id: "one" });
	const panelObservers = () =>
		observers.filter((entry) =>
			entry.node?.classList.contains("quick-ask-panel"),
		);
	expect(panelObservers()).toHaveLength(1);
	const first = panelObservers()[0];
	if (!first) throw new Error("The answer panel must have an observer");
	expect(first.node).toBe(host.querySelector(".quick-ask-panel"));
	height = 240;
	await event("quick-ask-partial", {
		request_id: "one",
		seq: 1,
		text: "Growing answer",
	});
	vi.useFakeTimers();
	const resizeCount = mocks.win.setSize.mock.calls.length;
	await act(async () => {
		first.callback();
		first.callback();
	});
	await act(async () => vi.advanceTimersByTimeAsync(20));
	expect(mocks.win.setSize.mock.calls.length - resizeCount).toBe(1);
	expect(mocks.win.setSize).toHaveBeenLastCalledWith({
		width: 520,
		height: 240,
	});
	await event("quick-ask-started", { request_id: "two" });
	expect(panelObservers()).toHaveLength(2);
	const second = panelObservers()[1];
	if (!second) throw new Error("The new panel must have its own observer");
	expect(first.disconnected).toBe(true);
	expect(second.node).toBe(host.querySelector(".quick-ask-panel"));
	expect(second.node).not.toBe(first.node);
});

it("can start without resize observation and finish once browser observation is available", async () => {
	vi.stubGlobal("ResizeObserver", undefined);
	await event("quick-ask-started", { request_id: "one" });
	await event("quick-ask-started", { request_id: "two" });
	expect(host.textContent).toContain("Thinking…");
	// Mantine's answer ScrollArea requires this browser API independently.
	vi.unstubAllGlobals();
	await event("quick-ask-answer", {
		request_id: "two",
		ok: true,
		answer: "Fallback answer",
	});
	expect(host.textContent).toContain("Fallback answer");
});

it("renders incremental RTL answers while loading, orders snapshots and never refocuses for tokens or completion", async () => {
	await event("quick-ask-started", {
		request_id: "one",
		question: "Synthetic question",
	});
	expect(host.textContent).toContain("Thinking…");
	expect(mocks.win.setFocus).toHaveBeenCalledTimes(1);
	await event("quick-ask-partial", { request_id: "one", seq: 1, text: "שלום" });
	expect(host.textContent).toContain("Answering…");
	expect(host.querySelector('[dir="auto"]')?.textContent).toBe("שלום");
	expect(host.querySelector('[aria-busy="true"]')).not.toBeNull();
	await event("quick-ask-partial", {
		request_id: "one",
		seq: 3,
		text: "שלום **world**",
	});
	await event("quick-ask-partial", { request_id: "one", seq: 2, text: "old" });
	expect(host.querySelector("strong")?.textContent).toBe("world");
	await event("quick-ask-answer", {
		request_id: "one",
		ok: true,
		answer: "שלום **final**",
	});
	expect(host.textContent).not.toContain("Answering…");
	expect(host.querySelector("strong")?.textContent).toBe("final");
	expect(host.querySelector('[aria-busy="true"]')).toBeNull();
	expect(mocks.win.setFocus).toHaveBeenCalledTimes(1);
	expect(mocks.win.show).toHaveBeenCalledTimes(1);
	await event("quick-ask-partial", { request_id: "one", seq: 4, text: "late" });
	expect(host.textContent).not.toContain("late");
});

it("retains partial content as explicitly incomplete and ignores stale completion for another answer", async () => {
	await event("quick-ask-started", { request_id: "one" });
	await event("quick-ask-partial", {
		request_id: "one",
		seq: 1,
		text: "Partial answer",
	});
	await event("quick-ask-answer", {
		request_id: "one",
		ok: false,
		error: "Provider disconnected",
	});
	expect(host.textContent).toContain("Partial answer");
	expect(host.textContent).toContain(
		"Answer incomplete: Provider disconnected",
	);
	await event("quick-ask-started", { request_id: "two" });
	await event("quick-ask-answer", {
		request_id: "one",
		ok: true,
		answer: "stale",
	});
	expect(host.textContent).toContain("Thinking…");
	expect(host.textContent).not.toContain("stale");
	await event("quick-ask-answer", {
		request_id: "two",
		ok: false,
		error: "Could not connect",
	});
	expect(host.textContent).toContain("Could not connect");
	expect(host.textContent).not.toContain("Answer incomplete:");
});

it("cancels on new recordings, dismissal and unmount, including command failures", async () => {
	await event("quick-ask-started", { request_id: "one" });
	await event("recording-start");
	expect(mocks.api.cancelQuickAsk).toHaveBeenCalledWith("one");
	await event("quick-ask-answer", {
		request_id: "one",
		ok: true,
		answer: "late",
	});
	expect(host.textContent).not.toContain("late");
	await event("quick-ask-started", { request_id: "two" });
	await event("pipeline-transcription-started");
	expect(mocks.api.cancelQuickAsk).toHaveBeenCalledWith("two");
	await event("quick-ask-started", { request_id: "three" });
	vi.useFakeTimers();
	mocks.api.cancelQuickAsk.mockRejectedValueOnce(new Error("IPC unavailable"));
	await event("quick-ask-dismiss-requested");
	await act(async () => vi.advanceTimersByTimeAsync(500));
	expect(mocks.api.cancelQuickAsk).toHaveBeenCalledWith("three");
	await event("quick-ask-started", { request_id: "four" });
	await act(async () => root.render(null));
	expect(mocks.api.cancelQuickAsk).toHaveBeenCalledWith("four");
	expect(mocks.events.size).toBe(0);
});
