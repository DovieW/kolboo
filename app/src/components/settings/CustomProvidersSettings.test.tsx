// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { configAPI } from "../../lib/tauri/commands";
import {
	CustomProvidersSettings,
	isLocalProviderHost,
} from "./CustomProvidersSettings";

Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);

vi.mock("../../lib/tauri/commands", () => ({
	configAPI: {
		getCustomProviders: vi.fn(),
		saveCustomProvider: vi.fn(),
		deleteCustomProvider: vi.fn(),
	},
}));
vi.mock("./ApiKeyField", () => ({
	ApiKeyField: ({ storeKey }: { storeKey: string }) => <span>{storeKey}</span>,
}));

it("shows secure key editor and confirms removal without silently changing selections", async () => {
	Object.defineProperty(document, "fonts", {
		configurable: true,
		value: new EventTarget(),
	});
	vi.mocked(configAPI.getCustomProviders).mockResolvedValue([
		{
			id: "custom_test",
			name: "My endpoint",
			base_url: "http://localhost:8000/v1",
			llm_models: ["model"],
			stt_models: [],
			llm_model_names: { model: "My model" },
		},
	]);
	vi.mocked(configAPI.deleteCustomProvider).mockResolvedValue();
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	const button = (label: string) => {
		const found = [...document.querySelectorAll("button")].find(
			(b) => b.textContent === label || b.getAttribute("aria-label") === label,
		);
		if (!found) throw new Error(`Missing button: ${label}`);
		return found;
	};
	try {
		await act(async () => {
			root.render(
				<MantineProvider>
					<QueryClientProvider client={client}>
						<CustomProvidersSettings />
					</QueryClientProvider>
				</MantineProvider>,
			);
		});
		await act(async () => {
			await new Promise((resolve) => setTimeout(resolve, 0));
		});
		expect(host.textContent).toContain("custom_test_api_key");
		await act(async () => button("Edit My endpoint").click());
		expect(
			(
				document.querySelector(
					'[aria-label="Text display name 1"]',
				) as HTMLInputElement
			).value,
		).toBe("My model");
		await act(async () => button("Save provider").click());
		expect(configAPI.saveCustomProvider).toHaveBeenCalledWith({
			id: "custom_test",
			name: "My endpoint",
			base_url: "http://localhost:8000/v1",
			llm_models: ["model"],
			stt_models: [],
			llm_model_names: { model: "My model" },
			stt_model_names: {},
		});
		await act(async () => button("Remove My endpoint").click());
		expect(configAPI.deleteCustomProvider).not.toHaveBeenCalled();
		expect(document.body.textContent).toContain(
			"will not be replaced automatically",
		);
		await act(async () => button("Remove provider").click());
		expect(configAPI.deleteCustomProvider).toHaveBeenCalledWith("custom_test");
	} finally {
		await act(async () => root.unmount());
		host.remove();
		client.clear();
	}
});

it("restricts insecure HTTP hosts to local addresses", () => {
	for (const host of [
		"localhost",
		"127.0.0.1",
		"192.168.1.2",
		"10.1.1.1",
		"172.16.0.1",
		"169.254.0.2",
		"[::1]",
		"[fd00::1]",
	])
		expect(isLocalProviderHost(host)).toBe(true);
	for (const host of [
		"example.com",
		"localhost.example.com",
		"8.8.8.8",
		"172.32.0.1",
	])
		expect(isLocalProviderHost(host)).toBe(false);
});

it.each([
	{ name: "", message: "Add a name and at least one model." },
	{ llm_models: [], message: "Add a name and at least one model." },
	{
		llm_models: ["duplicate", "duplicate"],
		message: "Each model ID must be unique",
	},
	{ name: "x".repeat(101), message: "Use a provider name up to 100" },
	{
		base_url: `https://example.test/${"x".repeat(2048)}`,
		message: "Use a provider name up to 100",
	},
])(
	"rejects invalid model or provider metadata before persistence: $message",
	async ({ message, ...invalid }) => {
		vi.mocked(configAPI.getCustomProviders).mockResolvedValue([
			{
				id: "custom_invalid",
				name: "Invalid",
				base_url: "https://example.test",
				llm_models: ["raw"],
				stt_models: [],
				...invalid,
			},
		]);
		vi.mocked(configAPI.saveCustomProvider).mockClear();
		const host = document.createElement("div");
		document.body.append(host);
		const root = createRoot(host);
		const client = new QueryClient({
			defaultOptions: { queries: { retry: false } },
		});
		vi.useFakeTimers();
		try {
			await act(async () =>
				root.render(
					<MantineProvider env="test">
						<QueryClientProvider client={client}>
							<CustomProvidersSettings />
						</QueryClientProvider>
					</MantineProvider>,
				),
			);
			await act(async () => vi.advanceTimersByTimeAsync(1));
			await act(async () =>
				document
					.querySelector<HTMLButtonElement>(
						`[aria-label="Edit ${invalid.name ?? "Invalid"}"]`,
					)
					?.click(),
			);
			await act(async () =>
				[...document.querySelectorAll("button")]
					.find((button) => button.textContent === "Save provider")
					?.click(),
			);
			expect(document.body.textContent).toContain(message);
			expect(configAPI.saveCustomProvider).not.toHaveBeenCalled();
		} finally {
			await act(async () => root.unmount());
			host.remove();
			client.clear();
			vi.useRealTimers();
		}
	},
);
