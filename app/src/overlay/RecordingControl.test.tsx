// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import RecordingControl from "./RecordingControl";

const native = vi.hoisted(() => ({
	listeners: new Map<string, (payload: unknown) => void>(),
	invoke: vi.fn(async (_command: string) => null),
	ocrState: vi.fn(),
	profile: vi.fn(),
	settings: {
		overlay_mode: "recording_only",
		accent_color: "#22c55e",
		rewrite_program_prompt_profiles: [],
	},
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("../lib/tauri/events", () => ({
	listenTyped: async (name: string, handler: (p: unknown) => void) => {
		native.listeners.set(name, handler);
		return () => native.listeners.delete(name);
	},
}));
vi.mock("../lib/queries", () => ({
	useSettings: () => ({ data: native.settings }),
	useTypeText: () => ({ mutateAsync: vi.fn() }),
}));
vi.mock("../lib/tauri", () => ({
	tauriAPI: {
		emitConnectionState: vi.fn(),
		hideOverlayHover: vi.fn(async () => {}),
		setOverlayLayout: vi.fn(async () => {}),
	},
	ocrAPI: { getOverlayState: native.ocrState },
}));
// Native polling isn't part of this UI race: events below drive the real reducer.
vi.mock("./useOverlayPipelineStatePolling", () => ({
	useOverlayPipelineStatePolling: vi.fn(),
}));
vi.mock("./useOverlayActiveProfilePolling", () => ({
	useOverlayActiveProfilePolling: native.profile,
}));

const host = document.createElement("div");
let root = createRoot(host);
const client = new QueryClient();
afterEach(async () => {
	await act(async () => root.unmount());
	root = createRoot(host);
	client.clear();
	vi.useRealTimers();
	vi.clearAllMocks();
	native.profile.mockReset();
	native.ocrState.mockReset();
	native.listeners.clear();
	native.settings.overlay_mode = "recording_only";
});
it("keeps Always mode visible without an idle render loop or stale hides", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	vi.useFakeTimers();
	native.profile.mockImplementation(() => {
		if (native.profile.mock.calls.length > 50)
			throw new Error("Unbounded overlay render loop");
	});
	native.settings.overlay_mode = "always";
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<RecordingControl />
			</QueryClientProvider>,
		),
	);
	expect(native.profile).toHaveBeenLastCalledWith(
		expect.objectContaining({ enabled: false }),
	);
	const initialRenders = native.profile.mock.calls.length;
	await act(async () => vi.advanceTimersByTime(10_000));
	expect(native.profile.mock.calls.length).toBe(initialRenders);
	await act(async () => native.listeners.get("overlay-hide-requested")?.(null));
	expect(host.querySelector(".overlay-widget")?.getAttribute("data-anim")).toBe(
		"visible",
	);
	await act(async () => native.listeners.get("recording-start")?.(null));
	expect(native.profile).toHaveBeenLastCalledWith(
		expect.objectContaining({ enabled: true }),
	);
});
it("cancels an old exit and stays visible across rapid consecutive recording sessions", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	vi.useFakeTimers();
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<RecordingControl />
			</QueryClientProvider>,
		),
	);
	const emit = async (name: string, payload: unknown = null) => {
		await act(async () => native.listeners.get(name)?.(payload));
	};
	const animation = () =>
		host.querySelector(".overlay-widget")?.getAttribute("data-anim");
	expect(native.profile).toHaveBeenLastCalledWith(
		expect.objectContaining({ enabled: false }),
	);
	await act(async () =>
		window.dispatchEvent(
			new MouseEvent("mousemove", { clientX: 1, clientY: 1 }),
		),
	);
	await emit("pipeline-state-changed", "arming");
	expect(host.querySelector(".overlay-wave--arming")).not.toBeNull();
	await emit("overlay-hide-requested");
	expect(animation()).toBe("visible");
	await emit("recording-start");
	expect(animation()).toBe("visible");
	expect(host.querySelector('[aria-label="Recording"]')).toBeNull();
	expect(
		host.querySelector(".overlay-button--expanded .overlay-wave"),
	).not.toBeNull();
	expect(
		host.querySelector(".overlay-button--expanded .overlay-meta:empty"),
	).not.toBeNull();
	expect(native.profile).toHaveBeenLastCalledWith(
		expect.objectContaining({ enabled: true }),
	);
	await emit("pipeline-state-changed", "transcribing");
	expect(host.querySelector(".overlay-wave--processing")).not.toBeNull();
	await emit("pipeline-state-changed", "idle");
	expect(animation()).toBe("exit");
	await act(async () => vi.advanceTimersByTime(100));
	await emit("recording-start");
	await act(async () => vi.advanceTimersByTime(500));
	expect(animation()).toBe("visible");
	expect(
		native.invoke.mock.calls.filter((call) => call[0] === "hide_overlay"),
	).toHaveLength(0);
	await emit("overlay-hide-requested");
	await act(async () => vi.advanceTimersByTime(500));
	expect(animation()).toBe("visible");
	await emit("pipeline-state-changed", "idle");
	await act(async () => vi.advanceTimersByTime(220));
	expect(animation()).toBe("enter");
	await emit("recording-start");
	expect(animation()).toBe("visible");
	await act(async () => vi.advanceTimersByTime(500));
	expect(animation()).toBe("visible");
});

it("shows an error indicator but no recording indicator in the expanded overlay", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<RecordingControl />
			</QueryClientProvider>,
		),
	);
	await act(async () =>
		native.listeners.get("pipeline-state-changed")?.("recording"),
	);
	expect(
		host.querySelector('.overlay-button--expanded [aria-label="Recording"]'),
	).toBeNull();
	await act(async () =>
		native.listeners.get("pipeline-state-changed")?.("error"),
	);
	expect(
		host.querySelector(
			'.overlay-button--expanded[data-error="true"] [aria-label="Error"]',
		),
	).not.toBeNull();
});

it("keeps the OCR action separate from the centered recording waveform", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	native.ocrState.mockResolvedValue({
		pipeline_state: "recording",
		ocr_session_id: "test-session",
		ocr_status: "not_started",
		ocr_manual_available: true,
		ocr_provider: { available: true },
		stt_complete: false,
	});
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<RecordingControl />
			</QueryClientProvider>,
		),
	);
	await act(async () =>
		host.querySelector<HTMLButtonElement>(".overlay-button--expanded")?.click(),
	);
	const button = host.querySelector(".overlay-button--expanded");
	expect(button?.getAttribute("data-has-ocr")).toBe("true");
	expect(button?.querySelector(".overlay-wave")).not.toBeNull();
	expect(button?.querySelector(".overlay-meta--ocr button")).not.toBeNull();
});
