// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { PromptSettings } from "./PromptSettings";

const fixtures = vi.hoisted(() => ({
	settings: undefined as unknown,
	defaults: { system: "Synthetic default prompt" },
	providers: { stt: [], llm: [] },
	models: {},
	empty: [],
}));
vi.mock("../../lib/queries", async (original) => ({
	...(await original<typeof import("../../lib/queries")>()),
	useSettings: () => ({ data: fixtures.settings, isLoading: false }),
	useDefaultSections: () => ({ data: fixtures.defaults, isLoading: false }),
	useAvailableProviders: () => ({ data: fixtures.providers, isLoading: false }),
	useLicenseAuthContext: () => ({ data: undefined }),
	useHasLastAudioForSttTest: () => ({ data: false }),
	useByokLlmModels: () => ({ data: fixtures.models }),
	useManagedModels: () => ({ data: fixtures.empty }),
	useFireworksModels: () => ({ data: fixtures.empty }),
	useOllamaModels: () => ({ data: fixtures.empty }),
}));
vi.mock("../../lib/queries/settings", async (original) => ({
	...(await original<typeof import("../../lib/queries/settings")>()),
	useSettings: () => ({ data: fixtures.settings, isLoading: false }),
}));
vi.mock("../../lib/queries/modelChoices", () => ({
	useModelChoices: () => ({ choices: [], loading: false, error: false }),
}));

it("wires profile controls to the shared picker without exposing old source switches", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	Object.defineProperty(document, "fonts", {
		configurable: true,
		value: { addEventListener: vi.fn(), removeEventListener: vi.fn() },
	});
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	});
	fixtures.settings = {
		stt_provider: "groq",
		stt_model: "whisper-large-v3-turbo",
		llm_provider: "ollama",
		llm_model: "saved-model",
		rewrite_llm_enabled: false,
		cleanup_prompt_sections: { system: { content: null } },
		rewrite_program_prompt_profiles: [
			{
				id: "p",
				name: "Profile",
				program_paths: [],
				cleanup_prompt_sections: {},
				presets: [
					{
						id: "q",
						name: "Preset",
						routing_hints: [],
						rewrite_llm_enabled: true,
					},
				],
			},
		],
	};
	try {
		await act(async () =>
			root.render(
				<QueryClientProvider client={client}>
					<MantineProvider env="test">
						<PromptSettings editingProfileId="p" />
					</MantineProvider>
				</QueryClientProvider>,
			),
		);
		expect(host.textContent).not.toContain("Show all providers and models");
		expect(host.textContent).toContain("Transcription model");
		expect(host.textContent).toContain("Rewrite model");
		expect(host.textContent).toContain("Quick Ask model");
		const presets = [
			...host.querySelectorAll<HTMLButtonElement>("button"),
		].find(
			(button) =>
				button.textContent?.includes("Presets") &&
				button.getAttribute("aria-expanded") !== null,
		);
		expect(presets).toBeDefined();
		await act(async () => presets?.click());
		const edit = [...host.querySelectorAll<HTMLButtonElement>("button")].find(
			(button) => button.textContent === "Edit Presets",
		);
		expect(edit).toBeDefined();
		await act(async () => edit?.click());
		expect(document.body.textContent).toContain("Prompt Lab");
		const presetSelector = document.querySelector<HTMLInputElement>(
			'input[placeholder="Default"]',
		);
		expect(presetSelector).toBeDefined();
		await act(async () => presetSelector?.click());
		const option = [
			...document.querySelectorAll<HTMLElement>('[role="option"]'),
		].find((option) => option.textContent === "Preset");
		expect(option).toBeDefined();
		await act(async () => option?.click());
		const lab = [
			...document.querySelectorAll<HTMLButtonElement>("button"),
		].find((button) => button.textContent === "Prompt Lab");
		expect(lab).toBeDefined();
		await act(async () => lab?.click());
		expect(
			document.querySelector('[aria-label="Prompt tools model"]'),
		).toBeDefined();
	} finally {
		await act(async () => root.unmount());
		host.remove();
		client.clear();
	}
});
