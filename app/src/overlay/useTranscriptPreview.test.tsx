// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { useTranscriptPreview } from "./useTranscriptPreview";

const listeners = vi.hoisted(
	() => new Map<string, (payload: unknown) => void>(),
);
vi.mock("../lib/tauri/events", () => ({
	listenTyped: vi.fn(async (name, callback) => {
		listeners.set(name, callback);
		return () => listeners.delete(name);
	}),
}));
Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);

it("preserves RTL text and rejects duplicate starts, superseded previews and late results after completion", async () => {
	const host = document.createElement("div");
	const root = createRoot(host);
	function Preview() {
		return <span dir="auto">{useTranscriptPreview()}</span>;
	}
	await act(async () => root.render(<Preview />));
	const send = async (name: string, payload: unknown = null) => {
		const callback = listeners.get(name);
		expect(callback).toBeDefined();
		await act(async () => callback?.(payload));
	};
	await send("stt-partial-transcript", {
		kind: "partial",
		session_epoch: 1,
		text: "too early",
	});
	expect(host.textContent).toBe("");
	await send("stt-partial-transcript", { kind: "started", session_epoch: 1 });
	await send("stt-partial-transcript", {
		kind: "partial",
		session_epoch: 1,
		text: "א גוטן טאָג",
	});
	expect(host.textContent).toBe("א גוטן טאָג");
	await send("stt-partial-transcript", { kind: "started", session_epoch: 1 });
	expect(host.textContent).toBe("א גוטן טאָג");
	for (const event of [
		"pipeline-cancelled",
		"pipeline-reset",
		"pipeline-error",
		"pipeline-transcript-ready",
	]) {
		await send(event);
		await send("stt-partial-transcript", {
			kind: "partial",
			session_epoch: 1,
			text: "late",
		});
		expect(host.textContent).toBe("");
	}
	await send("stt-partial-transcript", { kind: "started", session_epoch: 2 });
	await send("stt-partial-transcript", {
		kind: "partial",
		session_epoch: 2,
		text: "new",
	});
	await send("stt-partial-transcript", {
		kind: "partial",
		session_epoch: 1,
		text: "old",
	});
	await send("stt-partial-transcript", { kind: "started", session_epoch: 1 });
	expect(host.textContent).toBe("new");
	await act(async () => root.unmount());
	expect(listeners.size).toBe(0);
});
