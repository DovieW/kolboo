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
	DataSettings: () => <div>Data controls</div>,
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
	UiSettings: () => <div>Appearance controls</div>,
}));

let host: HTMLDivElement;
let root: Root;
let client: QueryClient;
async function render() {
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<MantineProvider env="test">
					<SettingsShell />
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
	it("keeps category navigation in the shared header and gives the profile selector a name", async () => {
		await render();
		const tabs = host.querySelector("[role='tablist']");
		expect(tabs?.getAttribute("aria-label")).toBe("Settings categories");
		expect(tabs?.closest("header")).not.toBeNull();
		expect(host.querySelector("[aria-label='Editing profile']")).not.toBeNull();
		expect(host.querySelector(".main-content-inner")?.textContent).toContain(
			"AI controls",
		);
		expect(host.querySelector(".tv-page-header")?.textContent).not.toContain(
			"AI controls",
		);
	});
	it("keeps managed users on AI with no keys, while exposing every category", async () => {
		await render();
		expect(
			host.querySelector("[role='tab'][aria-selected='true']")?.textContent,
		).toBe("AI");
		expect(host.querySelectorAll("[role='tab']")).toHaveLength(9);
		const appearance = [
			...host.querySelectorAll<HTMLButtonElement>("[role='tab']"),
		].find((tab) => tab.textContent === "Appearance");
		await act(async () => appearance?.click());
		expect(host.querySelector(".main-content-inner")?.textContent).toContain(
			"Appearance controls",
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
