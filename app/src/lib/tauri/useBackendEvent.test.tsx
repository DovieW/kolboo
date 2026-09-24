// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import { useBackendEvent } from "./useBackendEvent";

const listen = vi.hoisted(() => vi.fn());
vi.mock("./events", () => ({ listenTyped: listen }));
const host = document.createElement("div");
let root = createRoot(host);
Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
afterEach(async () => {
	await act(async () => root.unmount());
	root = createRoot(host);
	vi.resetAllMocks();
});
function Listener({
	handler,
	enabled,
}: {
	handler: () => void;
	enabled?: boolean;
}) {
	useBackendEvent("pipeline-cancelled", handler, enabled);
	return null;
}
it("keeps one subscription and calls the current callback, then unregisters on cleanup", async () => {
	const stop = vi.fn();
	listen.mockResolvedValue(stop);
	const first = vi.fn();
	const second = vi.fn();
	await act(async () => root.render(<Listener handler={first} />));
	await act(async () => root.render(<Listener handler={second} />));
	expect(listen).toHaveBeenCalledTimes(1);
	const deliver = listen.mock.calls[0]?.[1];
	deliver(null);
	expect(first).not.toHaveBeenCalled();
	expect(second).toHaveBeenCalledOnce();
	await act(async () => root.render(null));
	deliver(null);
	expect(second).toHaveBeenCalledOnce();
	expect(stop).toHaveBeenCalledOnce();
});
it("removes subscriptions that resolve after unmount without delivering stale events", async () => {
	let resolve!: (stop: () => void) => void;
	listen.mockImplementation(
		() =>
			new Promise((done) => {
				resolve = done;
			}),
	);
	const handler = vi.fn();
	const stop = vi.fn();
	await act(async () => root.render(<Listener handler={handler} />));
	await act(async () => root.render(null));
	await act(async () => resolve(stop));
	listen.mock.calls[0]?.[1](null);
	expect(stop).toHaveBeenCalledOnce();
	expect(handler).not.toHaveBeenCalled();
});
it("only subscribes while enabled and contains registration failures", async () => {
	listen.mockRejectedValue(new Error("native events unavailable"));
	await act(async () =>
		root.render(<Listener handler={vi.fn()} enabled={false} />),
	);
	expect(listen).not.toHaveBeenCalled();
	await act(async () => root.render(<Listener handler={vi.fn()} enabled />));
	expect(listen).toHaveBeenCalledOnce();
	await act(async () =>
		root.render(<Listener handler={vi.fn()} enabled={false} />),
	);
});
