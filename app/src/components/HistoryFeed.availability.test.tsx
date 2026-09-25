// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { HistoryFeed } from "./HistoryFeed";

const mocks = vi.hoisted(() => ({
	ids: vi.fn<() => Promise<string[]>>(),
	fullLogs: vi.fn(),
	listener: null as null | (() => void),
	stop: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-store", () => ({
	Store: {
		load: async () => ({
			get: async () => null,
			set: async () => {},
			save: async () => {},
		}),
	},
}));
vi.mock("../lib/tauri/events", () => ({
	listenTyped: async (_event: string, handler: () => void) => {
		mocks.listener = handler;
		return mocks.stop;
	},
}));
vi.mock("../lib/queries", async () => {
	const { useRequestLogIds } = await import("../lib/queries/logs");
	const page = {
		items: [
			{
				id: "recording-one",
				text: "Synthetic test transcript",
				timestamp: "2026-01-01T00:00:00Z",
				status: "success",
			},
		],
		totalAll: 1,
		totalFiltered: 1,
		page: 1,
		pageSize: 50,
	};
	return {
		useRequestLogIds,
		useRecordingsStats: () => ({ data: { count: 0, bytes: 0 } }),
		useSettings: () => ({ data: {} }),
		useHistoryPage: () => ({ data: page, isLoading: false }),
		useHistoryAll: () => ({ data: [] }),
		useRetryTranscription: () => ({ isPending: false }),
	};
});
vi.mock("../lib/tauri", async (importOriginal) => ({
	...(await importOriginal<typeof import("../lib/tauri")>()),
	logsAPI: { getRequestLogIds: mocks.ids, getRequestLogs: mocks.fullLogs },
	recordingsAPI: { getRecordingAssetUrl: async () => null },
	tauriAPI: { getHistoryDeleteOptions: vi.fn() },
	llmAPI: { getLlmProviders: async () => [] },
	dataAPI: {},
}));

it("enables the log action from identifiers and refreshes it on History events", async () => {
	vi.useFakeTimers();
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	let loaded!: (ids: string[]) => void;
	mocks.ids.mockImplementationOnce(
		() =>
			new Promise((resolve) => {
				loaded = resolve;
			}),
	);
	mocks.ids.mockResolvedValue([]);
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false, gcTime: Infinity } },
	});
	const jump = vi.fn();
	const settle = async () => {
		await act(async () => vi.advanceTimersByTimeAsync(1));
	};
	const click = async (element: Element | null) => {
		expect(element).not.toBeNull();
		await act(async () => (element as HTMLElement).click());
		await settle();
	};
	const logs = () =>
		[...document.querySelectorAll<HTMLButtonElement>("[role='menuitem']")].find(
			(node) => node.textContent === "View request log",
		);
	try {
		await act(async () =>
			root.render(
				<QueryClientProvider client={client}>
					<MantineProvider env="test">
						<HistoryFeed onJumpToLog={jump} />
					</MantineProvider>
				</QueryClientProvider>,
			),
		);
		await settle();
		expect(mocks.ids).toHaveBeenCalledWith(50);
		await click(host.querySelector("[aria-label='Recording actions']"));
		expect(logs()).toBeUndefined();
		await act(async () => loaded(["recording-one"]));
		await settle();
		expect(logs()).toBeDefined();
		await click(logs() ?? null);
		expect(jump).toHaveBeenCalledWith("recording-one");
		await act(async () => mocks.listener?.());
		await settle();
		expect(mocks.ids).toHaveBeenCalledTimes(2);
		expect(mocks.fullLogs).not.toHaveBeenCalled();
	} finally {
		await act(async () => root.unmount());
		expect(mocks.stop).toHaveBeenCalledOnce();
		client.clear();
		host.remove();
		vi.useRealTimers();
		vi.resetAllMocks();
	}
});
