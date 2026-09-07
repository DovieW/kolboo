// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RequestLog } from "../lib/tauri";
import { LogsView } from "./LogsView";

const mocks = vi.hoisted(() => ({
	clear: vi.fn(),
	export: vi.fn(),
	stop: vi.fn(),
	refetch: vi.fn(),
	showNotification: vi.fn(),
	query: {
		data: [] as RequestLog[] | undefined,
		isLoading: false,
		isError: false,
		isFetching: false,
	},
}));
vi.mock("@mantine/notifications", () => ({
	notifications: { show: mocks.showNotification },
}));
vi.mock("@tauri-apps/api/event", () => ({
	listen: vi.fn(async () => vi.fn()),
}));
vi.mock("../lib/queries", () => ({
	useRequestLogs: () => ({ ...mocks.query, refetch: mocks.refetch }),
	useSettings: () => ({ data: { hotkey_debug_enabled: false } }),
}));
vi.mock("../lib/useRecordingPlayer", () => ({
	useRecordingPlayer: () => ({ stop: mocks.stop }),
}));
vi.mock("../lib/logs/orchestration", async () => {
	const { useState } = await import("react");
	return {
		useLogsViewOrchestration: () => {
			const [exportOpened, setExportOpened] = useState(false);
			return {
				exportOpened,
				setExportOpened,
				exportLogs: { mutate: mocks.export, isPending: false },
				clearLogs: { mutate: mocks.clear, isPending: false },
				updateHotkeyDebugEnabled: { mutate: vi.fn(), isPending: false },
			};
		},
		getLogsExportSuccessNotification: () => ({ message: "Exported" }),
		getLogsExportFailureNotification: () => ({ message: "Export failed" }),
	};
});
vi.mock("./logs/LogsSystemEventsPanel", () => ({
	LogsSystemEventsPanel: () => <div>Diagnostic controls</div>,
}));
vi.mock("./logs/LogsRequestList", () => ({
	LogsRequestList: ({
		logs,
		openedLogId,
		onOpenedLogIdChange,
	}: {
		logs: RequestLog[];
		openedLogId: string | null;
		onOpenedLogIdChange: (id: string | null) => void;
	}) => (
		<div
			data-request-list
			data-visible-ids={logs.map((log) => log.id).join(",")}
			data-opened-id={openedLogId ?? ""}
		>
			<button type="button" onClick={() => onOpenedLogIdChange("req-1")}>
				Open request
			</button>
			<button type="button" onClick={() => onOpenedLogIdChange(null)}>
				Close request
			</button>
		</div>
	),
}));

let host: HTMLDivElement;
let root: Root;
async function render() {
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<LogsView />
			</MantineProvider>,
		),
	);
}
async function click(text: string) {
	const button = [...document.querySelectorAll("button")].find(
		(element) => element.textContent === text,
	);
	expect(button, `Button ${text}`).toBeDefined();
	await act(async () => button?.click());
}
async function search(value: string) {
	const input = host.querySelector<HTMLInputElement>(
		"[aria-label='Search request logs']",
	);
	expect(input).not.toBeNull();
	await act(async () => {
		Object.getOwnPropertyDescriptor(
			HTMLInputElement.prototype,
			"value",
		)?.set?.call(input, value);
		input?.dispatchEvent(new Event("input", { bubbles: true }));
	});
}
async function clickLabelledControl(label: string) {
	const labelled = document.querySelector<HTMLElement>(
		`[aria-label='${label}']`,
	);
	const forId = [...document.querySelectorAll("label")]
		.find((element) => element.textContent === label)
		?.getAttribute("for");
	const control = labelled ?? (forId ? document.getElementById(forId) : null);
	expect(control, `Control ${label}`).not.toBeNull();
	await act(async () => control?.click());
}
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	vi.clearAllMocks();
	mocks.query.data = [
		{ id: "req-1", status: "success", entries: [] } as unknown as RequestLog,
	];
	mocks.query.isLoading = false;
	mocks.query.isError = false;
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
});
afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
});

describe("Logs workspace", () => {
	it("keeps the request list out of the sticky header and pauses on request changes", async () => {
		await render();
		expect(host.querySelector("h1")?.textContent).toBe("Logs");
		expect(
			host.querySelector("[data-request-list]")?.closest("header"),
		).toBeNull();
		expect(
			host.querySelector("[aria-label='Search request logs']"),
		).not.toBeNull();
		mocks.stop.mockClear();
		await click("Open request");
		await click("Close request");
		expect(mocks.stop).toHaveBeenCalledTimes(2);
	});
	it.each(["search", "status filter", "pagination"])(
		"pauses playback when %s removes the opened request from the current page",
		async (change) => {
			mocks.query.data = Array.from(
				{ length: 26 },
				(_, index) =>
					({
						id: `req-${index + 1}`,
						status: index === 0 ? "success" : "error",
						entries: [],
					}) as unknown as RequestLog,
			);
			await render();
			await click("Open request");
			mocks.stop.mockClear();

			if (change === "search") {
				await search("req-26");
			} else if (change === "status filter") {
				await click("Filters");
				await clickLabelledControl("Show success");
			} else {
				await clickLabelledControl("Go to next logs page");
			}

			const visibleIds = host
				.querySelector("[data-request-list]")
				?.getAttribute("data-visible-ids")
				?.split(",");
			expect(visibleIds).not.toContain("req-1");
			expect(mocks.stop).toHaveBeenCalledTimes(1);
		},
	);
	it("pauses playback when the query loses its data and the request list unmounts", async () => {
		await render();
		await click("Open request");
		mocks.stop.mockClear();
		mocks.query.data = undefined;
		mocks.query.isLoading = true;
		await render();
		expect(host.querySelector("[data-request-list]")).toBeNull();
		expect(mocks.stop).toHaveBeenCalledTimes(1);
	});
	it("does not pause when search or refreshed data keeps the opened request visible", async () => {
		await render();
		await click("Open request");
		mocks.stop.mockClear();
		await search("req-1");
		mocks.query.data = mocks.query.data?.map((log) => ({ ...log }));
		await render();
		expect(
			host.querySelector("[data-request-list]")?.getAttribute("data-opened-id"),
		).toBe("req-1");
		expect(mocks.stop).not.toHaveBeenCalled();
	});
	it("requires explicit confirmation before clearing all logs", async () => {
		await render();
		await click("Clear logs");
		expect(mocks.clear).not.toHaveBeenCalled();
		expect(document.querySelector("[role='dialog']")?.textContent).toContain(
			"not just the filtered results",
		);
		await click("Cancel");
		expect(mocks.clear).not.toHaveBeenCalled();
		await click("Clear logs");
		await click("Clear all request logs");
		expect(mocks.clear).toHaveBeenCalledTimes(1);
	});
	it("warns before a content-bearing export and offers privacy-safe export instead", async () => {
		await render();
		await click("Export");
		await click("Export full debug JSON");
		expect(mocks.export).not.toHaveBeenCalled();
		expect(document.querySelector("[role='dialog']")?.textContent).toContain(
			"transcripts, prompts and provider responses",
		);
		await click("Use privacy-safe export");
		expect(mocks.export).toHaveBeenCalledWith(
			"privacySafe",
			expect.any(Object),
		);
	});
	it("shows a retryable load failure rather than an empty logs message", async () => {
		mocks.query.data = undefined;
		mocks.query.isError = true;
		await render();
		expect(host.textContent).toContain("Couldn't load request logs");
		expect(host.querySelector("[data-request-list]")).toBeNull();
		await click("Retry");
		expect(mocks.refetch).toHaveBeenCalledTimes(1);
	});
});
