// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { RequestLog } from "../lib/tauri";
import LogJsonModal from "./LogJsonModal";

// Render the code given to the third-party highlighter; never mock redaction.
vi.mock("@mantine/code-highlight", () => ({
	CodeHighlight: ({ code }: { code: string }) => <pre data-code>{code}</pre>,
}));
let host: HTMLDivElement;
let root: Root;
const close = vi.fn();
const fixture = (fields: Partial<RequestLog> = {}) =>
	({ id: "fixture", status: "success", ...fields }) as RequestLog;
async function render(log: RequestLog, opened = true) {
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<LogJsonModal opened={opened} onClose={close} log={log} />
			</MantineProvider>,
		),
	);
}
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	close.mockClear();
});
afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
});

it("always omits sensitive fields in the complete log and every payload tab without modifying the source", async () => {
	const payload = {
		model: "model-fixture",
		usage: 17,
		nested: [
			null,
			3,
			{
				prompt: "private-fixture",
				content: "private-fixture",
				text: "private-fixture",
			},
		],
		messages: [{ content: "private-fixture" }],
		context: "private-fixture",
	};
	const keys = [
		"stt_request_json",
		"stt_response_json",
		"llm_request_json",
		"llm_response_json",
		"ocr_request_json",
		"ocr_response_json",
		"quick_ask_request_json",
		"quick_ask_response_json",
		"quick_replace_request_json",
		"quick_replace_response_json",
		"router_request_json",
		"router_response_json",
	];
	const log = fixture({
		raw_transcript: "private-fixture",
		final_text: "private-fixture",
		...Object.fromEntries(keys.map((k) => [k, payload])),
	});
	await render(log);
	expect(host.textContent).not.toContain("private-fixture");
	expect(host.textContent).not.toContain("Privacy mode");
	expect(host.textContent).toContain("model-fixture");
	expect(host.textContent).toContain("sensitive content omitted");
	for (const tab of [
		...host.querySelectorAll<HTMLButtonElement>('[role="tab"]'),
	]) {
		await act(async () => tab.click());
		expect(host.textContent).not.toContain("private-fixture");
	}
	expect(log.raw_transcript).toBe("private-fixture");
	expect(payload.context).toBe("private-fixture");
});

it("sanitizes serialized JSON before formatting and omits unstructured provider bodies", async () => {
	await render(
		fixture({
			stt_request_json: '{"prompt":"private-fixture","model":"kept-model"}',
			stt_response_json: "raw private-fixture",
			llm_request_json: '"private-fixture"',
			llm_response_json: "null",
		}),
	);
	expect(host.textContent).not.toContain("private-fixture");
	expect(host.textContent).toContain("kept-model");
	expect(host.textContent).toContain("unstructured payload omitted");
});

it("keeps missing and null payloads readable and resets the tab for a different request", async () => {
	await render(fixture());
	expect(host.querySelectorAll('[role="tab"]')).toHaveLength(1);
	await render(
		fixture({ stt_request_json: null, stt_response_json: undefined }),
	);
	await act(async () =>
		[...host.querySelectorAll<HTMLButtonElement>('[role="tab"]')]
			.find((t) => t.textContent === "STT Response")
			?.click(),
	);
	expect(host.textContent).toContain("No payload captured");
	await render(fixture({ id: "next", stt_response_json: null }));
	expect(
		host.querySelector('[role="tab"][aria-selected="true"]')?.textContent,
	).toBe("Log");
	await render(fixture(), false);
	expect(host.querySelector('[role="dialog"]')).toBeNull();
});
