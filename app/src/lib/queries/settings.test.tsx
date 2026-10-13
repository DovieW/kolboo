// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { useUpdateModelSelection } from "./settings";

const mocks = vi.hoisted(() => ({ save: vi.fn() }));
vi.mock("../tauri", () => ({
	tauriAPI: { updateModelSelection: mocks.save },
	sttAPI: {},
	recordingsAPI: {},
	dataAPI: {},
	configAPI: {},
	logsAPI: {},
	llmAPI: {},
}));

it("refreshes saved model state after success or a failed runtime activation, without rolling back another window", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	const root = createRoot(document.createElement("div"));
	const client = new QueryClient({
		defaultOptions: { mutations: { retry: false } },
	});
	const invalidate = vi.spyOn(client, "invalidateQueries");
	let mutation!: ReturnType<typeof useUpdateModelSelection>;
	function Reader() {
		mutation = useUpdateModelSelection();
		return null;
	}
	const selection = {
		source: "byok" as const,
		provider_id: "ollama",
		model_id: "raw-id",
	};
	const scope = { profileId: "p", presetId: "q" };
	try {
		await act(async () =>
			root.render(
				<QueryClientProvider client={client}>
					<Reader />
				</QueryClientProvider>,
			),
		);
		mocks.save.mockResolvedValueOnce(undefined);
		await act(async () =>
			mutation.mutateAsync({ action: "prompt_tools", selection, scope }),
		);
		expect(mocks.save).toHaveBeenCalledWith("prompt_tools", selection, scope);
		expect(invalidate).toHaveBeenCalledWith({ queryKey: ["settings"] });
		invalidate.mockClear();
		client.setQueryData(["settings"], {
			model_selections: { rewrite: selection },
		});
		mocks.save.mockRejectedValueOnce(new Error("Runtime activation failed"));
		await act(async () => {
			await expect(
				mutation.mutateAsync({ action: "rewrite", selection: null }),
			).rejects.toThrow("Runtime activation failed");
		});
		expect(invalidate).toHaveBeenCalledWith({ queryKey: ["settings"] });
		expect(client.getQueryData(["settings"])).toEqual({
			model_selections: { rewrite: selection },
		});
	} finally {
		await act(async () => root.unmount());
		client.clear();
	}
});
