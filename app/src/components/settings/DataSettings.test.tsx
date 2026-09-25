// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { DataSettings } from "./DataSettings";

const calls = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: calls.invoke }));
vi.mock("@tauri-apps/plugin-store", () => ({
	Store: { load: async () => ({ get: async () => undefined }) },
}));
vi.mock("../../lib/queries", async () => {
	const { useDataStorageSummary } = await import(
		"../../lib/queries/recordings"
	);
	return {
		useDataStorageSummary,
		useSettings: () => ({ data: { rewrite_program_prompt_profiles: [] } }),
		useLicenseState: () => ({ data: null, isLoading: false }),
	};
});

it("uses one storage snapshot for counts and keys, without scanning a hidden Data tab", async () => {
	vi.useFakeTimers();
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	const summary = {
		recordings_count: 7,
		recordings_bytes: 1024 ** 3,
		history_count: 9,
		history_bytes: 0,
		request_logs_count: 2,
		stats_files_count: 1,
		stats_bytes: 0,
		settings_bytes: 0,
		api_keys_set_count: 3,
	};
	calls.invoke.mockImplementation(async (command: string) => {
		if (command === "get_data_storage_summary") return summary;
		if (command === "github_backup_has_token") return false;
		throw new Error(`Unexpected native call: ${command}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false, gcTime: Infinity } },
	});
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const render = async (active?: boolean) => {
		await act(async () =>
			root.render(
				<QueryClientProvider client={client}>
					<MantineProvider env="test">
						<DataSettings active={active} />
					</MantineProvider>
				</QueryClientProvider>,
			),
		);
		await act(async () => vi.advanceTimersByTimeAsync(1));
	};
	try {
		await render(false);
		expect(calls.invoke).not.toHaveBeenCalledWith("get_data_storage_summary");
		await render();
		expect(host.textContent).toContain("7 recordings at 1.00 GB");
		expect(host.textContent).toContain("API keys saved3 /");
		const storageCalls = () =>
			calls.invoke.mock.calls.filter(
				([command]) => command === "get_data_storage_summary",
			).length;
		expect(storageCalls()).toBe(1);
		await render(false);
		await act(async () => vi.advanceTimersByTimeAsync(30000));
		expect(storageCalls()).toBe(1);
		expect(host.textContent).toContain("7 recordings at 1.00 GB");
		await render(true);
		expect(storageCalls()).toBe(2);
		expect(
			calls.invoke.mock.calls.every(([command]) =>
				["get_data_storage_summary", "github_backup_has_token"].includes(
					command,
				),
			),
		).toBe(true);
	} finally {
		await act(async () => root.unmount());
		client.clear();
		host.remove();
		vi.useRealTimers();
		vi.resetAllMocks();
	}
});
