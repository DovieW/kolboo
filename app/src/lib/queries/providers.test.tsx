// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { useByokLlmModels, useUpdateSTTInputMode } from "./providers";
import { tauriAPI } from "../tauri";

const mocks = vi.hoisted(() => ({
	providers: vi.fn(),
	catalog: vi.fn(async () => ({})),
}));
vi.mock("./shared", async (original) => ({
	...(await original<typeof import("./shared")>()),
	queryFnDeps: { configAPI: { getAvailableProviders: mocks.providers } },
}));

it("saves input mode and invalidates settings without changing a model selection", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	const change = vi
		.spyOn(tauriAPI, "updateSTTInputMode")
		.mockResolvedValue(undefined);
	const host = document.createElement("div");
	const root = createRoot(host);
	const client = new QueryClient({
		defaultOptions: { mutations: { retry: false } },
	});
	const invalidations = vi.spyOn(client, "invalidateQueries");
	let mutation!: ReturnType<typeof useUpdateSTTInputMode>;
	function Reader() {
		mutation = useUpdateSTTInputMode();
		return null;
	}
	try {
		await act(async () =>
			root.render(
				<QueryClientProvider client={client}>
					<Reader />
				</QueryClientProvider>,
			),
		);
		await act(async () => mutation.mutateAsync("after_recording"));
		expect(change).toHaveBeenCalledExactlyOnceWith("after_recording");
		expect(invalidations).toHaveBeenCalledWith({ queryKey: ["settings"] });
	} finally {
		await act(async () => root.unmount());
		client.clear();
		change.mockRestore();
	}
});
vi.mock("../modelsDev", async (original) => ({
	...(await original<typeof import("../modelsDev")>()),
	fetchModelsDevByokLlmCatalog: mocks.catalog,
}));

it("merges configured model names into BYOK discovery without changing IDs", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	vi.useFakeTimers();
	const host = document.createElement("div");
	const root = createRoot(host);
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false, gcTime: Infinity } },
	});
	let result!: ReturnType<typeof useByokLlmModels>;
	function Reader() {
		result = useByokLlmModels();
		return null;
	}
	mocks.providers.mockResolvedValue({
		stt: [],
		llm: [
			{
				value: "custom_text",
				models: ["raw"],
				model_names: { raw: "Friendly text" },
			},
			{ value: "custom_empty", models: [] },
			{ value: "groq" },
		],
	});
	try {
		await act(async () =>
			root.render(
				<QueryClientProvider client={client}>
					<Reader />
				</QueryClientProvider>,
			),
		);
		await act(async () => vi.advanceTimersByTimeAsync(1));
		expect(result.data.custom_text).toEqual([
			{ value: "raw", label: "Friendly text" },
		]);
		expect(result.data.custom_empty).toEqual([]);
		expect(result.data.groq?.length).toBeGreaterThan(0);
		expect(mocks.catalog).toHaveBeenCalledOnce();
	} finally {
		await act(async () => root.unmount());
		client.clear();
		vi.useRealTimers();
	}
});
