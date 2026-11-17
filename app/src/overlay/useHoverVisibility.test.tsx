// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import { useHoverVisibility } from "./useHoverVisibility";

const native = vi.hoisted(() => ({ listen: vi.fn(), invoke: vi.fn() }));
vi.mock("../lib/tauri/events", () => ({ listenTyped: native.listen }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
const host = document.createElement("div");
let root = createRoot(host);
Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
function Surface() {
	return <span>{String(useHoverVisibility())}</span>;
}
afterEach(async () => {
	await act(async () => root.unmount());
	root = createRoot(host);
	vi.resetAllMocks();
});

it("acknowledges only a live subscription and follows native show/hide without heartbeats", async () => {
	let handler!: (visible: boolean) => void;
	let resolve!: (stop: () => void) => void;
	const stop = vi.fn();
	native.listen.mockImplementation((_name, next) => {
		handler = next;
		return new Promise((done) => {
			resolve = done;
		});
	});
	await act(async () => root.render(<Surface />));
	expect(host.textContent).toBe("false");
	expect(native.invoke).not.toHaveBeenCalled();
	await act(async () => resolve(stop));
	expect(native.listen).toHaveBeenCalledWith(
		"overlay-hover-visibility-changed",
		expect.any(Function),
	);
	expect(native.invoke).toHaveBeenCalledExactlyOnceWith(
		"overlay_hover_frontend_ready",
	);
	await act(async () => handler(true));
	expect(host.textContent).toBe("true");
	await act(async () => handler(false));
	expect(host.textContent).toBe("false");
	await act(async () => root.render(null));
	expect(stop).toHaveBeenCalledOnce();
	handler(true);
	expect(host.textContent).toBe("");
});

it("does not report readiness after unmount or a failed subscription", async () => {
	let resolve!: (stop: () => void) => void;
	const stop = vi.fn();
	native.listen.mockReturnValue(
		new Promise((done) => {
			resolve = done;
		}),
	);
	await act(async () => root.render(<Surface />));
	await act(async () => root.render(null));
	await act(async () => resolve(stop));
	expect(stop).toHaveBeenCalledOnce();
	expect(native.invoke).not.toHaveBeenCalled();
	native.listen.mockRejectedValue(new Error("synthetic subscription failure"));
	await act(async () => root.render(<Surface />));
	expect(native.invoke).not.toHaveBeenCalled();
});

it("keeps cleanup owned when readiness fails", async () => {
	const stop = vi.fn();
	native.listen.mockResolvedValue(stop);
	native.invoke.mockRejectedValue(new Error("synthetic IPC failure"));
	await act(async () => root.render(<Surface />));
	await act(async () => root.render(null));
	expect(stop).toHaveBeenCalledOnce();
});
