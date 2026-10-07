// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { LicenseState } from "../../lib/tauri";
import { SettingsGuideOverlay } from "./SettingsGuideOverlay";

const state = vi.hoisted(() => ({
	license: undefined as LicenseState | undefined,
}));
vi.mock("../../lib/queries", () => ({
	useSettings: () => ({ data: {} }),
	useLicenseState: () => ({ data: state.license }),
}));
vi.mock("../../lib/tauri", () => ({
	tauriAPI: { getApiKey: vi.fn().mockResolvedValue(null) },
}));
vi.mock("../../lib/frontendLog", () => ({ frontendLog: { info: vi.fn() } }));
vi.mock("../account/AccountAuthentication", () => ({
	AccountAuthentication: () => <form aria-label="Email-code sign-in" />,
}));

let host: HTMLDivElement;
let root: Root;
let client: QueryClient;
const callbacks = { onSkip: vi.fn(), onFinished: vi.fn(), onGoHome: vi.fn() };
async function render() {
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<MantineProvider env="test">
					<SettingsGuideOverlay opened {...callbacks} />
				</MantineProvider>
			</QueryClientProvider>,
		),
	);
}
async function click(label: string) {
	const button = [...host.querySelectorAll("button")].find(
		(item) => item.textContent === label,
	);
	expect(button).toBeDefined();
	await act(async () => button?.click());
}
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	// Happy DOM does not implement the browser font loading event source
	// used by Mantine's autosizing textarea.
	Object.defineProperty(document, "fonts", {
		configurable: true,
		value: new EventTarget(),
	});
	state.license = undefined;
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
});
afterEach(async () => {
	vi.unstubAllEnvs();
	await act(async () => root.unmount());
	client.clear();
	host.remove();
	Reflect.deleteProperty(document, "fonts");
});

it("starts directly at BYOK setup in Community packages, including cached Pro accounts", async () => {
	vi.stubEnv("VITE_CLOUD_SERVICE_ENABLED", "false");
	state.license = { tier: "personal", status: "active" } as LicenseState;
	await render();
	await click("Start");
	expect(host.querySelector("form")).toBeNull();
	expect(host.textContent).toContain("Create a Groq API key");
	expect(host.textContent).not.toContain("Pro active");
});

it("keeps account-free setup available and advances past BYOK setup when access becomes active", async () => {
	await render();
	await click("Start");
	expect(
		host.querySelector('[aria-label="Email-code sign-in"]'),
	).not.toBeNull();
	await click("Continue without an account");
	expect(host.textContent).toContain("Create a Groq API key");
	state.license = {
		tier: "personal",
		status: "active",
		email: "approved@example.test",
	} as LicenseState;
	await render();
	expect(host.textContent).not.toContain("Create a Groq API key");
	expect(host.querySelector("textarea")).not.toBeNull();
});

it.each([
	["personal", "active", "Pro", false],
	["enterprise", "grace", "Managed", false],
	["community", "active", "Community", true],
	["personal", "expired", "Community", true],
] as const)(
	"routes %s/%s using verified access rather than tier alone",
	async (tier, status, badge, needsKey) => {
		state.license = {
			tier,
			status,
			email: "synthetic@example.test",
		} as LicenseState;
		await render();
		await click("Start");
		expect(host.querySelector("form")).toBeNull();
		expect(host.querySelector(".mantine-Badge-root")?.textContent).toBe(badge);
		await click("Continue setup");
		expect(host.textContent?.includes("Create a Groq API key")).toBe(needsKey);
		expect(host.querySelector("textarea") !== null).toBe(!needsKey);
	},
);
