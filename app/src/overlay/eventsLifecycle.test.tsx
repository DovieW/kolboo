// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import { useOverlayHideRequested } from "./useOverlayHideRequested";
import { useOverlayHotkeyEvents } from "./useOverlayHotkeyEvents";
import { useOverlayPipelineEvents } from "./useOverlayPipelineEvents";

const native = vi.hoisted(() => ({
	listeners: new Map<string, (payload: unknown) => void>(),
	stops: [] as Array<ReturnType<typeof vi.fn>>,
	listen: vi.fn(),
}));
vi.mock("../lib/tauri/events", () => ({ listenTyped: native.listen }));
const host = document.createElement("div");
let root = createRoot(host);
Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
afterEach(async () => {
	await act(async () => root.unmount());
	root = createRoot(host);
	native.listeners.clear();
	native.stops = [];
	vi.resetAllMocks();
});
it("keeps stable subscriptions across re-renders, routes each state, and releases every listener", async () => {
	native.listen.mockImplementation(async (name, handler) => {
		native.listeners.set(name, handler);
		const stop = vi.fn();
		native.stops.push(stop);
		return stop;
	});
	const state = vi.fn(),
		error = vi.fn(),
		clear = vi.fn(),
		animation = vi.fn(),
		shown = vi.fn(),
		hide = vi.fn();
	function Overlay({ version }: { version: number }) {
		useOverlayPipelineEvents({
			setPipelineState: state,
			clearError: clear,
			onPipelineErrorPayload: (p) => error(version, p),
		});
		useOverlayHideRequested({ requestAnimatedHide: () => hide(version) });
		useOverlayHotkeyEvents({
			setPipelineState: state,
			clearError: clear,
			setAnimState: animation,
			markOverlayShownForHoverGating: shown,
		});
		return null;
	}
	await act(async () => root.render(<Overlay version={1} />));
	await act(async () => root.render(<Overlay version={2} />));
	expect(native.listen).toHaveBeenCalledTimes(8);
	const emit = (name: string, payload: unknown = null) =>
		native.listeners.get(name)?.(payload);
	emit("pipeline-state-changed", "recording");
	expect(state).toHaveBeenLastCalledWith("event", "recording");
	emit("pipeline-state-changed", "invalid");
	expect(state).toHaveBeenCalledOnce();
	for (const name of [
		"pipeline-cancelled",
		"pipeline-reset",
		"pipeline-transcript-ready",
	]) {
		emit(name);
		expect(state).toHaveBeenLastCalledWith("event", "idle");
	}
	expect(clear).toHaveBeenCalledTimes(3);
	emit("pipeline-error", { message: "fixture failure" });
	expect(error).toHaveBeenCalledWith(2, { message: "fixture failure" });
	expect(state).toHaveBeenLastCalledWith("event", "error");
	emit("overlay-hide-requested");
	expect(hide).toHaveBeenCalledWith(2);
	emit("recording-start");
	expect(state).toHaveBeenLastCalledWith("hotkey", "recording");
	expect(animation).toHaveBeenCalledWith("visible");
	expect(shown).toHaveBeenCalledOnce();
	emit("recording-stop");
	expect(state).toHaveBeenLastCalledWith("hotkey", "transcribing");
	await act(async () => root.render(null));
	expect(native.stops.every((stop) => stop.mock.calls.length === 1)).toBe(true);
	const calls = state.mock.calls.length;
	emit("recording-start");
	expect(state).toHaveBeenCalledTimes(calls);
});
