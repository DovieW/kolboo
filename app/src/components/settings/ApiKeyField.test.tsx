// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ApiKeyField } from "./ApiKeyField";

const api = vi.hoisted(() => ({
	hasApiKey: vi.fn(),
	getApiKey: vi.fn(),
	setApiKey: vi.fn(),
	clearApiKey: vi.fn(),
}));
vi.mock("../../lib/tauri", () => ({ tauriAPI: api }));
let host: HTMLDivElement;
let root: Root;
let client: QueryClient;
beforeEach(() => {
	vi.useFakeTimers();
	vi.clearAllMocks();
	api.hasApiKey.mockResolvedValue(true);
	api.setApiKey.mockResolvedValue(undefined);
	api.clearApiKey.mockResolvedValue(undefined);
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
});
afterEach(async () => {
	await act(async () => root.unmount());
	client.clear();
	host.remove();
	vi.useRealTimers();
});
async function flush() {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(0);
	});
}
async function render(
	storeKey = "groq_api_key",
	label = "Groq",
	disabled = false,
) {
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<QueryClientProvider client={client}>
					<ApiKeyField storeKey={storeKey} label={label} disabled={disabled} />
				</QueryClientProvider>
			</MantineProvider>,
		),
	);
	await flush();
}
function input() {
	return host.querySelector("input") as HTMLInputElement;
}
function button(text: string) {
	const found = [...document.querySelectorAll("button")].find(
		(element) => element.textContent === text,
	);
	if (!found) throw new Error(`Missing button: ${text}`);
	return found;
}
async function type(value: string) {
	await act(async () => {
		const field = input();
		Object.getOwnPropertyDescriptor(
			HTMLInputElement.prototype,
			"value",
		)?.set?.call(field, value);
		field.dispatchEvent(new Event("input", { bubbles: true }));
	});
}
async function blur(relatedTarget: EventTarget | null = null) {
	await act(async () =>
		input().dispatchEvent(
			new FocusEvent("focusout", { bubbles: true, relatedTarget }),
		),
	);
	await flush();
}
it("checks presence without reading the secret and never clears an empty replacement draft", async () => {
	await render();
	expect(host.textContent).toContain("Saved securely");
	expect(input().value).toBe("");
	expect(api.getApiKey).not.toHaveBeenCalled();
	await blur();
	expect(api.clearApiKey).not.toHaveBeenCalled();
	expect(api.setApiKey).not.toHaveBeenCalled();
});
it("saves on Enter once, clears the draft only after success, and caches no credential", async () => {
	let finish: () => void = () => {};
	api.setApiKey.mockImplementation(
		() =>
			new Promise<void>((resolve) => {
				finish = resolve;
			}),
	);
	await render();
	await type("  synthetic-fixture  ");
	await act(async () =>
		input().dispatchEvent(
			new KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
		),
	);
	await blur();
	expect(api.setApiKey).toHaveBeenCalledTimes(1);
	expect(api.setApiKey).toHaveBeenCalledWith(
		"groq_api_key",
		"synthetic-fixture",
	);
	expect(input().value).toBe("  synthetic-fixture  ");
	await act(async () => finish());
	await flush();
	expect(input().value).toBe("");
	expect(host.textContent).toContain("Saved securely");
	expect(
		JSON.stringify(
			client
				.getQueryCache()
				.getAll()
				.map((query) => query.state.data),
		),
	).not.toContain("synthetic-fixture");
	expect(client.getMutationCache().getAll()).toHaveLength(0);
});
it("uses blur saving for OCR through the same secure-storage contract", async () => {
	await render("ocr_api_key", "OCR");
	await type("synthetic-ocr-fixture");
	await blur();
	expect(api.setApiKey).toHaveBeenCalledWith(
		"ocr_api_key",
		"synthetic-ocr-fixture",
	);
	expect(input().value).toBe("");
});
it("preserves a failed draft, hides raw errors and retries explicitly", async () => {
	api.setApiKey.mockRejectedValueOnce(
		new Error("raw error containing synthetic-fixture"),
	);
	await render();
	await type("synthetic-fixture");
	await blur();
	expect(input().value).toBe("synthetic-fixture");
	expect(host.textContent).toContain("Your draft is still here");
	expect(host.textContent).not.toContain("raw error");
	expect(api.clearApiKey).not.toHaveBeenCalled();
	await act(async () => button("Retry").click());
	await flush();
	expect(api.setApiKey).toHaveBeenCalledTimes(2);
	expect(input().value).toBe("");
});
it("requires confirmation to remove, and does not pretend a failed removal succeeded", async () => {
	api.clearApiKey.mockRejectedValueOnce(new Error("credential store failure"));
	await render();
	await act(async () => button("Remove").click());
	expect(api.clearApiKey).not.toHaveBeenCalled();
	await act(async () => button("Remove key").click());
	await flush();
	expect(host.textContent).toContain("Couldn’t remove the key");
	expect(client.getQueryData(["apiKey", "groq_api_key"])).toBe(true);
	await act(async () => button("Cancel").click());
	await act(async () => button("Retry").click());
	expect(api.clearApiKey).toHaveBeenCalledTimes(1);
	await act(async () => button("Remove key").click());
	await flush();
	expect(host.textContent).toContain("No key saved");
	expect(api.clearApiKey).toHaveBeenCalledTimes(2);
});
it("does not save a draft merely because its visibility button receives focus", async () => {
	await render();
	await type("synthetic-fixture");
	await blur(host.querySelector("button"));
	expect(api.setApiKey).not.toHaveBeenCalled();
	expect(input().value).toBe("synthetic-fixture");
	await act(async () =>
		host
			.querySelector("button")
			?.dispatchEvent(new FocusEvent("focusout", { bubbles: true })),
	);
	await flush();
	expect(api.setApiKey).toHaveBeenCalledTimes(1);
});
it("shows a recoverable status-check failure rather than an editable unknown state", async () => {
	api.hasApiKey.mockRejectedValueOnce(new Error("private status failure"));
	await render();
	expect(input().disabled).toBe(true);
	expect(host.textContent).toContain("Can’t check the saved key");
	expect(host.textContent).not.toContain("private status failure");
	await act(async () => button("Retry").click());
	await flush();
	expect(input().disabled).toBe(false);
	expect(host.textContent).toContain("Saved securely");
});

it("does not allow global key mutations in a disabled profile scope", async () => {
	await render("groq_api_key", "Groq", true);
	expect(input().disabled).toBe(true);
	expect(button("Remove").disabled).toBe(true);
	await type("synthetic-fixture");
	await blur();
	expect(api.setApiKey).not.toHaveBeenCalled();
	expect(api.clearApiKey).not.toHaveBeenCalled();
});
