// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ModelChoice } from "../lib/modelSelection";
import type { AppSettings } from "../lib/tauri/types";
import { ActionModelPicker } from "./ActionModelPicker";
import { ModelNavigationContext, type ModelProviderSetup } from "./ModelNavigation";

const fixture = vi.hoisted(() => ({
	settings: undefined as AppSettings | undefined,
	mutate: vi.fn(),
	pending: false,
	error: false,
	catalogError: false,
	purpose: vi.fn(),
}));
const choices: ModelChoice[] = [
	{
		selection: { source: "byok", provider_id: "custom_a", model_id: "raw" },
		displayName: "Friendly",
		providerName: "Custom",
		capabilities: ["Text"],
		ready: true,
	},
	{
		selection: {
			source: "managed",
			provider_id: "groq",
			model_id: "published",
		},
		displayName: "Managed",
		providerName: "Groq",
		capabilities: ["Text"],
		ready: true,
	},
	{
		selection: { source: "byok", provider_id: "missing", model_id: "missing" },
		displayName: "Unconfigured",
		providerName: "Missing",
		capabilities: ["Text"],
		ready: false,
	},
];
vi.mock("../lib/queries/settings", () => ({
	useSettings: () => ({ data: fixture.settings }),
	useUpdateModelSelection: () => ({
		mutate: fixture.mutate,
		isPending: fixture.pending,
		isError: fixture.error,
	}),
}));
vi.mock("../lib/queries/modelChoices", () => ({
	useModelChoices: (purpose: string) => {
		fixture.purpose(purpose);
		return {
			choices,
			loading: false,
			error: fixture.catalogError,
			managedMessage: "Sign in for Managed",
		};
	},
}));
let host: HTMLDivElement, root: Root;
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	vi.clearAllMocks();
	fixture.settings = undefined;
	fixture.pending = false;
	fixture.error = false;
	fixture.catalogError = false;
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
});
afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
});
async function render(
	props: Partial<Parameters<typeof ActionModelPicker>[0]> = {},
	navigation?: {
		setup: (selection: ModelProviderSetup) => void;
		account: () => void;
	},
) {
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<ModelNavigationContext.Provider value={navigation ?? null}>
					<ActionModelPicker action="rewrite" {...props} />
				</ModelNavigationContext.Provider>
			</MantineProvider>,
		),
	);
}
async function click(label: string) {
	const button = [...host.querySelectorAll<HTMLButtonElement>("button")].find(
		(button) =>
			button.getAttribute("aria-label") === label ||
			button.textContent === label,
	);
	if (!button) throw new Error(`Missing ${label}`);
	await act(async () => button.click());
}
async function choose(source: "byok" | "managed" = "byok") {
	await click("Model");
	await click(source === "byok" ? "Your models" : "Kolboo Managed");
	await act(async () =>
		host.querySelector<HTMLElement>('[role="option"]')?.click(),
	);
}

it("browses without persisting then writes a single global source/provider/model tuple", async () => {
	await render();
	await choose("managed");
	expect(fixture.mutate).toHaveBeenCalledTimes(1);
	expect(fixture.mutate.mock.calls[0]?.[0]).toEqual({
		action: "rewrite",
		selection: choices[1]?.selection,
		scope: undefined,
	});
	expect(fixture.purpose).toHaveBeenCalledWith("llm");
	const callback = fixture.mutate.mock.calls[0]?.[1] as {
		onSuccess: () => void;
	};
	callback.onSuccess();
	fixture.settings = {
		llm_provider: "custom_a",
		llm_model: "raw",
	} as AppSettings;
	await render({ profileId: "default" });
	expect(host.textContent).toContain("Friendly");
});

it("saves scoped choices independently and writes an inheritance tombstone without changing legacy IDs", async () => {
	fixture.settings = {
		model_selections: { rewrite: choices[1]?.selection },
		rewrite_program_prompt_profiles: [
			{
				id: "p",
				name: "Profile",
				model_selections: { rewrite: choices[0]?.selection },
				presets: [
					{
						id: "q",
						name: "Preset",
						model_selections: { rewrite: choices[1]?.selection },
					},
				],
			},
		],
	} as AppSettings;
	const selected = vi.fn();
	await render({ profileId: "p", presetId: "q", onSelection: selected });
	expect(host.textContent).toContain("Managed");
	await choose();
	expect(fixture.mutate.mock.calls[0]?.[0]).toEqual({
		action: "rewrite",
		selection: choices[0]?.selection,
		scope: { profileId: "p", presetId: "q" },
	});
	expect(selected).not.toHaveBeenCalled();
	const saved = fixture.mutate.mock.calls[0]?.[1] as
		| { onSuccess: () => void }
		| undefined;
	if (!saved) throw new Error("Missing save callback");
	saved.onSuccess();
	expect(selected).toHaveBeenCalledWith(choices[0]?.selection);
	await click("Model");
	await click("Use inherited selection");
	expect(fixture.mutate.mock.calls[1]?.[0]).toEqual({
		action: "rewrite",
		selection: null,
		scope: { profileId: "p", presetId: "q" },
	});
	await render({ profileId: "p" });
	expect(host.textContent).toContain("Friendly");
	fixture.settings.rewrite_program_prompt_profiles[0]!.presets = [];
	await render({ profileId: "p", presetId: "missing" });
	expect(host.textContent).toContain("Friendly");
});

it("preserves unavailable and legacy Managed selections while rendering setup, account, and error guidance", async () => {
	const navigation = { setup: vi.fn(), account: vi.fn() };
	fixture.settings = {
		stt_provider: "groq",
		stt_model: "published",
		stt_use_managed_inference: true,
	} as AppSettings;
	await render({ action: "transcription" }, navigation);
	expect(host.textContent).toContain("Managed");
	expect(fixture.purpose).toHaveBeenCalledWith("dictation");
	fixture.settings.stt_model = "unpublished";
	await render({ action: "transcription" }, navigation);
	expect(host.textContent).toContain("unpublished");
	expect(host.textContent).toContain("Managed");
	fixture.settings = {
		llm_provider: "managed",
		llm_model: "published",
	} as AppSettings;
	await render({}, navigation);
	expect(host.textContent).toContain("Managed");
	fixture.settings.llm_model = "unpublished";
	await render({}, navigation);
	expect(host.textContent).toContain("unpublished");
	expect(host.querySelector('[aria-label="Model"]')?.textContent).toContain(
		"legacy routing",
	);
	await click("Model");
	await click("Kolboo Managed");
	await click("Sign in / View account access");
	expect(navigation.account).toHaveBeenCalledOnce();
	await click("Your models");
	await click("Set up provider securely");
	expect(navigation.setup).toHaveBeenCalledWith(choices[2]?.selection);
	fixture.settings = {
		llm_provider: "custom_a",
		llm_model: null,
	} as AppSettings;
	fixture.error = true;
	fixture.catalogError = true;
	await render();
	expect(host.textContent).toContain("default model");
	expect(host.textContent).toContain("saved choice is unchanged");
	expect(host.querySelector('[role="alert"]')?.textContent).toContain(
		"could not be applied",
	);
	expect(fixture.mutate).not.toHaveBeenCalled();
});

it("disables changing a choice while saving or under policy", async () => {
	fixture.pending = true;
	await render();
	expect(
		host.querySelector<HTMLButtonElement>('[aria-label="Model"]')?.disabled,
	).toBe(true);
	fixture.pending = false;
	await render({ disabled: true });
	expect(
		host.querySelector<HTMLButtonElement>('[aria-label="Model"]')?.disabled,
	).toBe(true);
	fixture.settings = {
		rewrite_program_prompt_profiles: [],
		policy_state: {
			source: "cloud",
			is_valid: true,
			enforced_fields: [{ path: "llm_provider" }],
		},
	} as unknown as AppSettings;
	await render({ profileId: "p" });
	expect(
		host.querySelector<HTMLButtonElement>('[aria-label="Model"]')?.disabled,
	).toBe(true);
	expect(host.textContent).toContain("organization policy");
	expect(fixture.mutate).not.toHaveBeenCalled();
});
