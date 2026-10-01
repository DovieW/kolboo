// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { autostartAPI } from "../../lib/tauri/autostart";
import { StartAtLoginSetting } from "./StartAtLoginSetting";

let host: HTMLDivElement;
let root: Root;
let client: QueryClient;
let enabled: boolean;
let readFailure: boolean;
let writeFailure: boolean;
let pendingRead: Promise<boolean> | undefined;
let commands: string[];

beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	vi.useFakeTimers();
	enabled = false;
	readFailure = false;
	writeFailure = false;
	pendingRead = undefined;
	commands = [];
	mockIPC(async (command) => {
		commands.push(command);
		if (command === "plugin:autostart|is_enabled") {
			if (readFailure) throw new Error("Sensitive OS path must not be shown");
			return pendingRead ?? enabled;
		}
		if (writeFailure) throw new Error("Sensitive OS path must not be shown");
		if (command === "plugin:autostart|enable") enabled = true;
		else if (command === "plugin:autostart|disable") enabled = false;
		else throw new Error(`Unexpected command ${command}`);
		return undefined;
	});
	client = new QueryClient({
		defaultOptions: {
			queries: { retry: false, gcTime: Infinity },
			mutations: { retry: false, gcTime: Infinity },
		},
	});
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
});

afterEach(async () => {
	await act(async () => root.unmount());
	client.clear();
	host.remove();
	clearMocks();
	vi.useRealTimers();
});

async function settle() {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(1);
	});
}

async function render(profileScope = false) {
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<MantineProvider env="test">
					<StartAtLoginSetting profileScope={profileScope} />
				</MantineProvider>
			</QueryClientProvider>,
		),
	);
	await settle();
}

function toggle() {
	const input = host.querySelector<HTMLInputElement>(
		'input[aria-label="Start at login"]',
	);
	if (!input) throw new Error("Missing startup switch");
	return input;
}

async function clickToggle() {
	await act(async () => toggle().click());
	await settle();
}

it("reads existing OS state and enables/disables with a verified readback", async () => {
	await render();
	expect(toggle().checked).toBe(false);
	expect(toggle().disabled).toBe(false);
	expect(commands).toEqual(["plugin:autostart|is_enabled"]);
	await clickToggle();
	expect(enabled).toBe(true);
	expect(toggle().checked).toBe(true);
	expect(commands).toContain("plugin:autostart|enable");
	expect(
		commands.filter((value) => value.endsWith("is_enabled")).length,
	).toBeGreaterThan(1);
	await clickToggle();
	expect(enabled).toBe(false);
	expect(toggle().checked).toBe(false);
	expect(commands).toContain("plugin:autostart|disable");
});

it("loads an already-enabled registration but blocks per-program overrides", async () => {
	enabled = true;
	await render(true);
	expect(toggle().checked).toBe(true);
	expect(toggle().disabled).toBe(true);
	await clickToggle();
	expect(commands).toEqual(["plugin:autostart|is_enabled"]);
});

it("keeps the control disabled until the OS check completes", async () => {
	let resolve!: (value: boolean) => void;
	pendingRead = new Promise((done) => {
		resolve = done;
	});
	await render();
	expect(toggle().disabled).toBe(true);
	await act(async () => resolve(true));
	await settle();
	expect(toggle().disabled).toBe(false);
	expect(toggle().checked).toBe(true);
});

it("offers a safe retry when the OS read fails", async () => {
	readFailure = true;
	await render();
	expect(toggle().disabled).toBe(true);
	expect(host.textContent).toContain("Couldn’t read startup setting");
	expect(host.textContent).not.toContain("Sensitive");
	readFailure = false;
	enabled = true;
	const retry = host.querySelector<HTMLButtonElement>(
		'[aria-label="Retry startup setting"]',
	);
	if (!retry) throw new Error("Missing startup retry");
	await act(async () => retry.click());
	await settle();
	expect(toggle().checked).toBe(true);
	expect(toggle().disabled).toBe(false);
	expect(host.querySelector('[aria-label="Retry startup setting"]')).toBeNull();
});

it("does not show a failed write as saved and allows a later retry", async () => {
	await render();
	writeFailure = true;
	await clickToggle();
	expect(toggle().checked).toBe(false);
	expect(host.textContent).toContain("Couldn’t update startup setting");
	expect(host.textContent).not.toContain("Sensitive");
	writeFailure = false;
	await clickToggle();
	expect(toggle().checked).toBe(true);
	expect(host.textContent).not.toContain("Couldn’t");
});

it("blocks duplicate writes while a mutation readback is pending", async () => {
	await render();
	let resolve!: (value: boolean) => void;
	pendingRead = new Promise((done) => {
		resolve = done;
	});
	await clickToggle();
	expect(toggle().disabled).toBe(true);
	await clickToggle();
	expect(commands.filter((value) => value.endsWith("enable"))).toHaveLength(1);
	pendingRead = undefined;
	await act(async () => resolve(true));
	await settle();
	expect(toggle().disabled).toBe(false);
	expect(toggle().checked).toBe(true);
});

it("propagates disable/readback failures instead of claiming success", async () => {
	enabled = true;
	writeFailure = true;
	await expect(autostartAPI.setEnabled(false)).rejects.toThrow("Sensitive");
	expect(enabled).toBe(true);
	writeFailure = false;
	readFailure = true;
	await expect(autostartAPI.setEnabled(false)).rejects.toThrow("Sensitive");
	expect(enabled).toBe(false);
});
