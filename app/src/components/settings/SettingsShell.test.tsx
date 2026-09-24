// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsShell } from "./SettingsShell";

const mocks = vi.hoisted(() => ({
	managed: true,
	hasKey: vi.fn(async () => false),
}));
vi.mock("../../lib/queries", () => ({
	useSettings: () => ({ data: { rewrite_program_prompt_profiles: [] } }),
	useLicenseAuthContext: () => ({ data: {}, isFetched: true }),
}));
vi.mock("../../lib/tauri", () => ({
	hasManagedInferenceAccess: () => mocks.managed,
	tauriAPI: { hasApiKey: mocks.hasKey },
}));
vi.mock("./ApiKeysSettings", () => ({
	ApiKeysSettings: () => <div>Provider controls</div>,
}));
vi.mock("./AudioSettings", () => ({
	AudioSettings: () => <div>Audio controls</div>,
}));
vi.mock("./DataSettings", () => ({
	DataSettings: ({ active }: { active: boolean }) => (
		<div data-data-active={String(active)}>Data controls</div>
	),
}));
vi.mock("./HotkeySettings", () => ({
	HotkeySettings: () => <div>Hotkey controls</div>,
}));
vi.mock("./NetworkSettings", () => ({
	NetworkSettings: () => <div>Network controls</div>,
}));
vi.mock("./PolicySettings", () => ({
	PolicySettings: () => <div>Policy controls</div>,
}));
vi.mock("./PrivacySettings", () => ({
	PrivacySettings: () => <div>Privacy controls</div>,
}));
vi.mock("./ProgramsModal", () => ({ ProfileConfigModal: () => null }));
vi.mock("./PromptSettings", () => ({
	PromptSettings: () => <div>AI controls</div>,
}));
vi.mock("./UiSettings", () => ({
	UiSettings: () => <div>UI controls</div>,
}));

let host: HTMLDivElement;
let root: Root;
let client: QueryClient;
async function render() {
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<MantineProvider env="test">
					<SettingsShell onRunSetupGuide={vi.fn()} />
				</MantineProvider>
			</QueryClientProvider>,
		),
	);
	await act(async () => vi.advanceTimersByTimeAsync(1));
}
beforeEach(() => {
	vi.useFakeTimers();
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	vi.clearAllMocks();
	mocks.managed = true;
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

describe("Settings shell", () => {
	it("keeps cached Data controls mounted but deactivates their background work", async () => {
		await render();
		const tab = (name: string) => {
			const target = [
				...host.querySelectorAll<HTMLButtonElement>("[role='tab']"),
			].find((node) => node.textContent === name);
			if (!target) throw new Error(`Missing ${name} tab`);
			return target;
		};
		await act(async () => tab("Data").click());
		await act(async () => vi.advanceTimersByTimeAsync(1));
		expect(
			host
				.querySelector("[data-data-active]")
				?.getAttribute("data-data-active"),
		).toBe("true");
		await act(async () => tab("AI").click());
		expect(
			host
				.querySelector("[data-data-active]")
				?.getAttribute("data-data-active"),
		).toBe("false");
		await act(async () => tab("Data").click());
		expect(
			host
				.querySelector("[data-data-active]")
				?.getAttribute("data-data-active"),
		).toBe("true");
	});
	it("places profile controls alongside named category tabs without a page heading", async () => {
		await render();
		const tabs = host.querySelector("[role='tablist']");
		expect(tabs?.getAttribute("aria-label")).toBe("Settings categories");
		const profile = host.querySelector("[aria-label='Editing profile']");
		expect(profile).not.toBeNull();
		expect(profile?.closest(".settings-tabs-toolbar")).toBe(
			tabs?.closest(".settings-tabs-toolbar"),
		);
		expect(host.querySelector("h1")).toBeNull();
		expect(host.querySelector('[aria-label="Run setup guide"]')).toBeNull();
		expect(host.querySelector(".main-content-inner")?.textContent).toContain(
			"AI controls",
		);
	});
	it("keeps managed users on AI with no keys, while exposing every category", async () => {
		await render();
		expect(
			host.querySelector("[role='tab'][aria-selected='true']")?.textContent,
		).toBe("AI");
		expect(host.querySelectorAll("[role='tab']")).toHaveLength(9);
		const ui = [
			...host.querySelectorAll<HTMLButtonElement>("[role='tab']"),
		].find((tab) => tab.textContent === "UI");
		await act(async () => ui?.click());
		expect(host.querySelector(".main-content-inner")?.textContent).toContain(
			"UI controls",
		);
	});
	it("activates heavy tabs before mounting their content", async () => {
		await render();
		const providers = [
			...host.querySelectorAll<HTMLButtonElement>("[role='tab']"),
		].find((tab) => tab.textContent === "Providers");
		await act(async () => providers?.click());
		expect(
			host.querySelector("[role='tab'][aria-selected='true']")?.textContent,
		).toBe("Providers");
		expect(
			host.querySelector(
				'[role="status"][aria-label="Loading Providers settings"]',
			),
		).not.toBeNull();
		expect(host.textContent).not.toContain("Provider controls");
		await act(async () => {
			await vi.advanceTimersByTimeAsync(1);
		});
		expect(host.textContent).toContain("Provider controls");
	});
	it("retains the provider setup landing for a community user without keys", async () => {
		mocks.managed = false;
		await render();
		expect(
			host.querySelector("[role='tab'][aria-selected='true']")?.textContent,
		).toBe("Providers");
		await act(async () => {
			await vi.advanceTimersByTimeAsync(1);
		});
		expect(host.querySelector(".main-content-inner")?.textContent).toContain(
			"Provider controls",
		);
	});
});
