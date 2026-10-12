// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act, type ComponentProps } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { RewritePromptLabModal } from "./RewritePromptLabModal";

const mocks = vi.hoisted(() => ({ settings: undefined as unknown }));
vi.mock("../../lib/queries", () => ({
	useSettings: () => ({ data: mocks.settings }),
}));
vi.mock("../../lib/queries/settings", () => ({
	useSettings: () => ({ data: mocks.settings }),
	useUpdateModelSelection: () => ({ mutate: vi.fn(), isPending: false }),
}));
vi.mock("../../lib/queries/modelChoices", () => ({
	useModelChoices: () => ({ choices: [], loading: false, error: false }),
}));

it("resolves the preset action choice and carries its scope through improve/test without altering raw IDs", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	Object.defineProperty(document, "fonts", {
		configurable: true,
		value: { addEventListener: vi.fn(), removeEventListener: vi.fn() },
	});
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const selected = {
		source: "byok",
		provider_id: "ollama",
		model_id: "preset-id",
	};
	mocks.settings = {
		llm_provider: "openai",
		llm_model: "global",
		rewrite_program_prompt_profiles: [
			{
				id: "p",
				name: "Profile",
				presets: [
					{
						id: "q",
						name: "Preset",
						model_selections: { prompt_tools: selected },
					},
				],
			},
		],
	};
	const improve = vi.fn(async () => ({
		improvedPrompt: "Synthetic improved prompt",
		providerUsed: "ollama",
		modelUsed: "preset-id",
	}));
	const test = vi.fn(async () => ({
		output: "Synthetic output",
		providerUsed: "ollama",
		modelUsed: "preset-id",
	}));
	const props: ComponentProps<typeof RewritePromptLabModal> = {
		opened: true,
		onClose: vi.fn(),
		profileId: "p",
		presetId: "q",
		profileLabel: "Profile · Preset",
		currentPrompt: "Synthetic prompt",
		initialTranscript: "Synthetic input",
		initialProblemOutput: "Synthetic bad output",
		onIteratePrompt: improve,
		onTestPrompt: test,
		onSetPrompt: vi.fn(),
	};
	async function render(overrides: Partial<typeof props> = {}) {
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<RewritePromptLabModal {...props} {...overrides} />
				</MantineProvider>,
			),
		);
	}
	const all = () => [...document.querySelectorAll<HTMLButtonElement>("button")];
	async function click(label: string) {
		const button = all().find((button) => button.textContent === label);
		if (!button) throw Error(`Missing ${label}`);
		await act(async () => button.click());
	}
	async function input(label: string, value: string) {
		const field = [
			...document.querySelectorAll<HTMLTextAreaElement>("textarea"),
		].find((field) => field.labels?.[0]?.textContent?.includes(label));
		if (!field) throw Error(`Missing ${label}`);
		await act(async () => {
			Object.getOwnPropertyDescriptor(
				HTMLTextAreaElement.prototype,
				"value",
			)?.set?.call(field, value);
			field.dispatchEvent(new Event("input", { bubbles: true }));
		});
	}
	try {
		await render();
		expect(
			document.querySelector('[aria-label="Prompt tools model"]')?.textContent,
		).toContain("preset-id");
		await input("Desired output (what you want)", "Synthetic desired output");
		await click("Improve prompt");
		expect(improve).toHaveBeenCalledWith(
			expect.objectContaining({
				profileId: "p",
				presetId: "q",
				llmProvider: "ollama",
				llmModel: "preset-id",
			}),
		);
		await click("Test prompt");
		expect(test).toHaveBeenCalledWith(
			expect.objectContaining({
				profileId: "p",
				presetId: "q",
				prompt: "Synthetic improved prompt",
			}),
		);
		mocks.settings = {
			llm_provider: "ollama",
			llm_model: "global-id",
			rewrite_program_prompt_profiles: [],
		};
		await render({ profileId: "default", presetId: undefined });
		expect(
			document.querySelector('[aria-label="Prompt tools model"]')?.textContent,
		).toContain("global-id");
		mocks.settings = undefined;
		await render({ profileId: "missing", presetId: undefined });
		expect(
			document.querySelector('[aria-label="Prompt tools model"]')?.textContent,
		).toContain("Choose a model");
	} finally {
		await act(async () => root.unmount());
		host.remove();
	}
});
