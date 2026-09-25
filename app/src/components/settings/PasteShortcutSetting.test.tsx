// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { RewriteProgramPromptProfile } from "../../lib/tauri";
import { DEFAULT_SETTINGS_VALUES } from "../../lib/tauri/settingsDefaults";
import { UiSettings } from "./UiSettings";

const mocks = vi.hoisted(() => ({
	settings: {} as Record<string, unknown>,
	loading: false,
	updateGlobal: vi.fn(async () => {}),
	updateProfiles: vi.fn(async () => {}),
}));
vi.mock("../../lib/tauri", async (original) => {
	const actual = await original<typeof import("../../lib/tauri")>();
	return {
		...actual,
		tauriAPI: {
			...actual.tauriAPI,
			updateOutputPasteShortcut: mocks.updateGlobal,
			updateRewriteProgramPromptProfiles: mocks.updateProfiles,
		},
	};
});
vi.mock("../../lib/queries", async (original) => {
	const actual = await original<typeof import("../../lib/queries")>();
	return {
		...actual,
		useSettings: () => ({ data: mocks.settings, isLoading: mocks.loading }),
		useIsAudioMuteSupported: () => ({ data: true }),
	};
});

let root: Root;
let host: HTMLDivElement;
let client: QueryClient;
const terminal = {
	id: "terminal",
	name: "Terminal",
	output_paste_shortcut: null,
} as RewriteProgramPromptProfile;
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	vi.clearAllMocks();
	mocks.settings = {
		...DEFAULT_SETTINGS_VALUES,
		rewrite_program_prompt_profiles: [terminal],
	};
	mocks.loading = false;
	client = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	});
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
});
afterEach(async () => {
	await act(async () => root.unmount());
	client.clear();
	host.remove();
});
async function render(profile = "default") {
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<MantineProvider env="test">
					<UiSettings editingProfileId={profile} />
				</MantineProvider>
			</QueryClientProvider>,
		),
	);
}
function picker() {
	const input = host.querySelector<HTMLInputElement>(
		'input[aria-label="Paste shortcut"]',
	);
	if (!input) throw new Error("Missing paste shortcut selector");
	return input;
}
async function choose(label: string) {
	await act(async () => picker().click());
	const option = [
		...document.querySelectorAll<HTMLElement>('[role="option"]'),
	].find((el) => el.textContent === label);
	if (!option) throw new Error(`Missing shortcut option: ${label}`);
	await act(async () => option.click());
}

it("offers a global paste chord and keeps the system default for existing settings", async () => {
	delete mocks.settings.output_paste_shortcut;
	await render();
	expect(picker().value).toBe("System default (Ctrl / ⌘ + V)");
	await choose("Ctrl+Shift+V");
	expect(mocks.updateGlobal).toHaveBeenCalledWith("ctrl_shift_v");
	expect(mocks.updateProfiles).not.toHaveBeenCalled();
});
it("inherits the global chord and saves a terminal-only override", async () => {
	mocks.settings.output_paste_shortcut = "shift_insert";
	await render("terminal");
	expect(picker().value).toBe("Shift+Insert");
	expect(host.querySelector('[aria-label="Reset paste shortcut"]')).toBeNull();
	await choose("Ctrl+Shift+V");
	expect(mocks.updateProfiles).toHaveBeenCalledWith([
		{ ...terminal, output_paste_shortcut: "ctrl_shift_v" },
	]);
	expect(mocks.updateGlobal).not.toHaveBeenCalled();
});
it("resets an explicit system-default override back to inheritance", async () => {
	mocks.settings.output_paste_shortcut = "ctrl_shift_v";
	mocks.settings.rewrite_program_prompt_profiles = [
		{ ...terminal, output_paste_shortcut: "system" },
	];
	await render("terminal");
	expect(picker().value).toBe("System default (Ctrl / ⌘ + V)");
	await act(async () =>
		host
			.querySelector<HTMLButtonElement>('[aria-label="Reset paste shortcut"]')
			?.click(),
	);
	expect(mocks.updateProfiles).toHaveBeenCalledWith([terminal]);
});
it("disables paste selection for copy-only output and while settings load", async () => {
	mocks.settings.output_mode = "clipboard";
	await render();
	expect(picker().disabled).toBe(true);
	mocks.settings.output_mode = "paste";
	mocks.loading = true;
	await render();
	expect(picker().disabled).toBe(true);
	expect(mocks.updateGlobal).not.toHaveBeenCalled();
});
