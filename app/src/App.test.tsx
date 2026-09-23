// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

vi.mock("./components/account", () => ({
	AccountView: () => <div>Account page</div>,
}));
vi.mock("./components/FileTranscription", () => ({
	FileTranscription: ({ active }: { active: boolean }) => (
		<section data-file-active={String(active)}>
			<input aria-label="File import draft" defaultValue="" />
		</section>
	),
}));
vi.mock("./components/HistoryFeed", () => ({
	HistoryFeed: () => <div>History list</div>,
}));
vi.mock("./components/LogsView", () => ({
	LogsView: () => <div>Logs page</div>,
}));
vi.mock("./components/MicStatusCard", () => ({ MicStatusCard: () => null }));
vi.mock("./components/RecordingBar", () => ({ RecordingBar: () => null }));
vi.mock("./components/settings", () => ({
	SettingsShell: () => <div>Settings page</div>,
}));
vi.mock("./components/settings/SettingsGuideOverlay", () => ({
	SettingsGuideOverlay: () => null,
}));
vi.mock("./components/settings/TelemetryDisclosureModal", () => ({
	TelemetryDisclosureModal: () => null,
}));
vi.mock("./components/usageStats/CostTab", () => ({
	CostTab: () => <h3>Total spend</h3>,
}));
vi.mock("./hooks/useModifierKeyForwarder", () => ({
	useModifierKeyForwarder: () => {},
}));
vi.mock("./lib/accentColor", () => ({ applyAccentColor: vi.fn() }));
vi.mock("./lib/bootStorage", () => ({
	readBootAccentColor: () => "green",
	readBootGuideState: () => "completed",
	setBootGuideState: vi.fn(),
}));
vi.mock("./lib/frontendLog", () => ({
	frontendLog: { info: vi.fn(), warn: vi.fn() },
}));
vi.mock("./lib/modelOptions", () => ({
	listAllLlmModelKeys: () => [],
	listAllSttModelKeys: () => [],
}));
vi.mock("./lib/queries", () => ({
	useSettings: () => ({ data: undefined }),
	useSettingsGuideState: () => ({ data: "completed" }),
	useSetSettingsGuideState: () => ({ mutate: vi.fn() }),
}));
vi.mock("./lib/tauri", () => ({
	getPolicyPathEnforcement: () => ({ enforced: false }),
	tauriAPI: {
		onStatsChanged: vi.fn(async () => vi.fn()),
		onTranscriptCopiedToClipboard: vi.fn(async () => vi.fn()),
	},
}));
vi.mock("./lib/tauri/events", () => ({
	listenTyped: vi.fn(async () => vi.fn()),
}));
vi.mock("./lib/updates", () => ({
	checkSignedUpdateVersion: vi.fn(),
	compareSemver: vi.fn(() => 0),
	installSignedUpdate: vi.fn(),
	signedUpdaterEnabled: false,
}));

let host: HTMLDivElement;
let root: Root;
let client: QueryClient;
beforeEach(async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<MantineProvider env="test">
					<App />
				</MantineProvider>
			</QueryClientProvider>,
		),
	);
});
afterEach(async () => {
	await act(async () => root.unmount());
	client.clear();
	host.remove();
});
async function navigate(label: string) {
	const button = host.querySelector<HTMLButtonElement>(
		`nav button[aria-label="${label}"]`,
	);
	expect(button).not.toBeNull();
	await act(async () => button?.click());
}

describe("Desktop navigation", () => {
	it("names every destination for icon-rail users and marks the current page", async () => {
		expect(
			[...host.querySelectorAll("nav button")].map((button) =>
				button.getAttribute("aria-label"),
			),
		).toEqual([
			"Home",
			"Transcribe file",
			"Settings",
			"Usage",
			"Account",
			"Logs",
		]);
		expect(
			host
				.querySelector("nav [aria-current='page']")
				?.getAttribute("aria-label"),
		).toBe("Home");
		expect(host.querySelector("h1")).toBeNull();
		expect(host.textContent).toContain("History list");
		expect(
			[...host.querySelectorAll("main button")].some(
				(button) => button.textContent === "Transcribe file",
			),
		).toBe(false);
		expect(host.textContent).not.toContain("Welcome to Kolboo");
		await navigate("Settings");
		expect(host.querySelector("main")?.textContent).toContain("Settings page");
		expect(
			host
				.querySelector("nav [aria-current='page']")
				?.getAttribute("aria-label"),
		).toBe("Settings");
	});
	it("keeps a file-import draft mounted while navigating and only activates it on its page", async () => {
		await navigate("Transcribe file");
		const draft = host.querySelector<HTMLInputElement>(
			"[aria-label='File import draft']",
		);
		expect(draft).not.toBeNull();
		if (!draft) throw new Error("Missing draft");
		draft.value = "Selected recording";
		expect(
			host.querySelector("[data-file-active='true']")?.closest("[hidden]"),
		).toBeNull();
		await navigate("Home");
		expect(
			host.querySelector("[data-file-active='false']")?.closest("[hidden]"),
		).not.toBeNull();
		await navigate("Transcribe file");
		expect(host.querySelector("[aria-label='File import draft']")).toBe(draft);
		expect(draft.value).toBe("Selected recording");
	});
	it("keeps Usage filters beside Total spend without a page heading", async () => {
		await navigate("Usage");
		expect(host.querySelector("h1")).toBeNull();
		expect(host.querySelector("h3")?.textContent).toBe("Total spend");
		expect(
			host.querySelector(".usage-controls [aria-label='Filters']"),
		).not.toBeNull();
		expect(
			host.querySelector(".usage-controls input")?.getAttribute("value"),
		).toBe("Last 30 days");
	});
});
