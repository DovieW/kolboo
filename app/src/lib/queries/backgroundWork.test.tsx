// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import { useDataStorageSummary } from "./recordings";
import { useRequestLogIds } from "./logs";

const mocks = vi.hoisted(() => ({
	storage: vi.fn(async () => ({ recordings_count: 3 })),
	ids: vi.fn(async () => ["one"]),
	fullLogs: vi.fn(),
}));
vi.mock("./shared", () => ({
	queryFnDeps: {
		dataAPI: { getStorageSummary: mocks.storage },
		logsAPI: { getRequestLogIds: mocks.ids, getRequestLogs: mocks.fullLogs },
	},
}));
const cleanups: (() => Promise<void>)[] = [];
afterEach(async () => {
	for (const cleanup of cleanups.splice(0)) await cleanup();
	vi.useRealTimers();
	vi.clearAllMocks();
});

it("does not scan storage while a kept-mounted Data tab is hidden, and resumes on return", async () => {
	vi.useFakeTimers();
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	const host = document.createElement("div");
	const root = createRoot(host);
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false, gcTime: Infinity } },
	});
	function View({ active }: { active?: boolean }) {
		const summary = useDataStorageSummary(active);
		useRequestLogIds(25);
		return <span>{summary.data?.recordings_count}</span>;
	}
	const render = async (active?: boolean) => {
		await act(async () =>
			root.render(
				<QueryClientProvider client={client}>
					<View active={active} />
				</QueryClientProvider>,
			),
		);
		await act(async () => vi.advanceTimersByTimeAsync(1));
	};
	cleanups.push(async () => {
		await act(async () => root.unmount());
		client.clear();
	});
	await render(false);
	expect(mocks.storage).not.toHaveBeenCalled();
	expect(mocks.ids).toHaveBeenCalledWith(25);
	expect(mocks.fullLogs).not.toHaveBeenCalled();
	await render();
	expect(host.textContent).toBe("3");
	expect(mocks.storage).toHaveBeenCalledOnce();
	await act(async () => vi.advanceTimersByTimeAsync(10000));
	expect(mocks.storage).toHaveBeenCalledTimes(2);
	await render(false);
	await act(async () => vi.advanceTimersByTimeAsync(30000));
	await act(async () =>
		client.invalidateQueries({ queryKey: ["dataStorageSummary"] }),
	);
	expect(mocks.storage).toHaveBeenCalledTimes(2);
	await render(true);
	expect(mocks.storage).toHaveBeenCalledTimes(3);
	await act(async () =>
		client.invalidateQueries({ queryKey: ["requestLogs"] }),
	);
	expect(mocks.ids.mock.calls.length).toBeGreaterThan(1);
	expect(mocks.fullLogs).not.toHaveBeenCalled();
});
