// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ModelChoice } from "../lib/modelSelection";
import { ModelPicker } from "./ModelPicker";

const choices: ModelChoice[] = [
	{
		selection: { source: "byok", provider_id: "custom_a", model_id: "raw-a" },
		displayName: "<b>Friendly</b>",
		providerName: "My endpoint",
		ready: true,
		capabilities: ["Text"],
		pricing: "$1 / million input tokens",
		limitations: "Text only",
	},
	{
		selection: { source: "byok", provider_id: "custom_b", model_id: "raw-b" },
		displayName: "Another model",
		providerName: "Second endpoint",
		ready: false,
		reason: "API key required",
		capabilities: ["Text"],
	},
	{
		selection: { source: "managed", provider_id: "openai", model_id: "raw-a" },
		displayName: "Managed model",
		providerName: "OpenAI",
		ready: true,
		capabilities: ["Text"],
	},
];
let host: HTMLDivElement, root: Root;
const change = vi.fn(),
	retry = vi.fn(),
	setup = vi.fn(),
	inherit = vi.fn();
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	vi.clearAllMocks();
});
afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
});
async function render(props: Partial<Parameters<typeof ModelPicker>[0]> = {}) {
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<ModelPicker
					label="Model"
					value={null}
					choices={choices}
					onChange={change}
					{...props}
				/>
			</MantineProvider>,
		),
	);
}
async function click(label: string) {
	const button = [...host.querySelectorAll<HTMLButtonElement>("button")].find(
		(button) =>
			button.textContent === label ||
			button.getAttribute("aria-label") === label,
	);
	if (!button) throw new Error(`Missing ${label}`);
	await act(async () => button.click());
}
async function search(value: string) {
	const field = host.querySelector<HTMLInputElement>(
		'[aria-label="Search models"]',
	);
	if (!field) throw new Error("Missing search");
	await act(async () => {
		Object.getOwnPropertyDescriptor(
			HTMLInputElement.prototype,
			"value",
		)?.set?.call(field, value);
		field.dispatchEvent(new Event("input", { bubbles: true }));
	});
	return field;
}

it("browses both tabs and details without changing settings, then atomically chooses a model", async () => {
	await render({
		onRetry: retry,
		onSetup: setup,
		onInherit: inherit,
		managedMessage: "Sign in or refresh access",
	});
	await click("Model");
	expect(
		[...host.querySelectorAll('[role="tab"]')].map((el) => el.textContent),
	).toEqual(["Kolboo Managed", "Your models"]);
	expect(host.textContent).toContain("Model ID: raw-a");
	expect(host.textContent).toContain("$1 / million input tokens");
	expect(host.textContent).toContain("Text only");
	expect(host.textContent).toContain("Pricing unavailable — not assumed free.");
	expect(host.querySelector("b")).toBeNull();
	await click("Set up provider securely");
	expect(setup).toHaveBeenCalledExactlyOnceWith(choices[1]?.selection);
	await click("Retry loading models");
	expect(retry).toHaveBeenCalledOnce();
	await click("Kolboo Managed");
	expect(host.textContent).toContain("Sign in or refresh access");
	expect(change).not.toHaveBeenCalled();
	await search(" OpenAI ");
	expect(host.querySelectorAll('[role="option"]')).toHaveLength(1);
	await act(async () =>
		host.querySelector<HTMLElement>('[role="option"]')?.click(),
	);
	expect(change).toHaveBeenCalledExactlyOnceWith({
		source: "managed",
		provider_id: "openai",
		model_id: "raw-a",
	});
	await click("Model");
	await click("Use inherited selection");
	expect(inherit).toHaveBeenCalledOnce();
	expect(change).toHaveBeenCalledOnce();
});

it("supports keyboard search/selection and preserves raw identifiers when clearing", async () => {
	await render({ value: choices[0]?.selection ?? null });
	expect(host.textContent).toContain("<b>Friendly</b>");
	await click("Model");
	const field = await search("raw-a");
	await act(async () => {
		field.dispatchEvent(
			new KeyboardEvent("keydown", {
				key: "ArrowDown",
				code: "ArrowDown",
				bubbles: true,
			}),
		);
	});
	await act(async () => {
		field.dispatchEvent(
			new KeyboardEvent("keydown", {
				key: "Enter",
				code: "Enter",
				bubbles: true,
			}),
		);
	});
	expect(change).toHaveBeenLastCalledWith({
		source: "byok",
		provider_id: "custom_a",
		model_id: "raw-a",
	});
	await click("Clear model");
	expect(change).toHaveBeenLastCalledWith(null);
});

it("keeps missing selections visible and explains loading and unavailable models without substituting", async () => {
	await render({
		value: {
			source: "managed",
			provider_id: "retired",
			model_id: "retired-id",
		},
		choices: [],
		loading: true,
	});
	expect(host.textContent).toContain("retired-id · retired · Managed");
	expect(host.textContent).toContain("Saved selection unavailable");
	await click("Model");
	expect(host.textContent).toContain("Loading models…");
	await render({ choices: [], loading: false });
	expect(host.textContent).toContain("Choose a model");
	expect(host.textContent).toContain("No matching models");
	expect(change).not.toHaveBeenCalled();
});

it("prevents unconfigured selections and locks all mutation controls during processing", async () => {
	await render({ onSetup: setup });
	await click("Model");
	await act(async () =>
		host.querySelectorAll<HTMLElement>('[role="option"]')[1]?.click(),
	);
	expect(change).not.toHaveBeenCalled();
	await render({
		value: choices[1]?.selection ?? null,
		disabled: true,
		onSetup: setup,
		onRetry: retry,
		onInherit: inherit,
	});
	expect(host.textContent).toContain("Saved selection unavailable");
	for (const button of host.querySelectorAll<HTMLButtonElement>("button"))
		expect(button.disabled).toBe(true);
	await click("Set up provider securely");
	await click("Retry loading models");
	await click("Use inherited selection");
	expect(setup).not.toHaveBeenCalled();
	expect(retry).not.toHaveBeenCalled();
	expect(inherit).not.toHaveBeenCalled();
});
