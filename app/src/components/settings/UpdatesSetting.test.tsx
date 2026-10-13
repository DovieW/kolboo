// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { UpdateStatus } from "../../lib/tauri/types.generated";
import { UpdatesSetting } from "./UpdatesSetting";

let host: HTMLDivElement;
let root: Root;
let client: QueryClient;
let status: UpdateStatus;
let calls: string[];
let failing: string;
let pending: Promise<void> | undefined;
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	vi.useFakeTimers();
	status = {
		enabled: true,
		checked: false,
		phase: "idle",
		version: null,
		error: null,
		hint: "Installing closes Kolboo.",
		can_install: false,
	};
	calls = [];
	failing = "";
	pending = undefined;
	mockIPC(async (command) => {
		calls.push(command);
		if (command === failing) throw new Error("sensitive upstream diagnostic");
		await pending;
		return command === "install_update" ? undefined : { ...status };
	});
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
	clearMocks();
	vi.useRealTimers();
});
async function settle(ms = 1) {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(ms);
	});
}
async function render() {
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<MantineProvider env="test">
					<UpdatesSetting />
				</MantineProvider>
			</QueryClientProvider>,
		),
	);
	await settle();
}
function button(name: string) {
	const found = Array.from(host.querySelectorAll("button")).find(
		(button) => button.textContent === name,
	);
	if (!found) throw new Error(`Missing ${name}`);
	return found;
}
async function click(name: string) {
	await act(async () => button(name).click());
	await settle();
}

it("shows initial loading, then checks through the real IPC wrapper and polls the result", async () => {
	let resolve!: () => void;
	pending = new Promise<void>((done) => {
		resolve = done;
	});
	await render();
	expect(host.textContent).toContain("Loading update status");
	expect(button("Check for updates").disabled).toBe(true);
	await act(async () => resolve());
	pending = undefined;
	await settle();
	expect(host.textContent).toContain("No update is ready");
	status.phase = "checking";
	await click("Check for updates");
	expect(calls).toContain("check_for_updates");
	expect(button("Check for updates").disabled).toBe(true);
	status.phase = "idle";
	status.checked = true;
	await settle(1000);
	expect(host.textContent).toContain("You’re up to date");
	expect(button("Check for updates").disabled).toBe(false);
});

it.each([
	["checking", "Checking for updates", true],
	["downloading", "Downloading and verifying", true],
	["ready", "Update ready", false],
	["installing", "Kolboo will close", true],
	["error", "download failed", false],
] as const)(
	"renders %s with appropriate controls",
	async (phase, label, busy) => {
		status.phase = phase;
		status.version = "1.2.3";
		status.can_install = true;
		await render();
		expect(host.textContent).toContain(label);
		expect(host.textContent).toContain("Version 1.2.3");
		expect(button("Check for updates").disabled).toBe(busy);
		expect(button("Install update").disabled).toBe(busy);
	},
);

it("does not offer native installation for a disabled build or while work is active", async () => {
	status.enabled = false;
	await render();
	expect(host.textContent).toContain("manual downloads");
	expect(button("Check for updates").disabled).toBe(true);
	expect(button("Install update").disabled).toBe(true);
	expect(host.querySelector("a")?.href).toBe(
		"https://github.com/DovieW/kolboo/releases/latest",
	);
	status.enabled = true;
	status.phase = "ready";
	await settle(1000);
	expect(button("Check for updates").disabled).toBe(false);
	expect(button("Install update").disabled).toBe(true);
});

it("installs via IPC, blocks duplicate operations, refreshes status and retries a safe failure", async () => {
	status.phase = "ready";
	status.can_install = true;
	await render();
	failing = "install_update";
	await click("Install update");
	expect(host.textContent).toContain("Finish active work");
	expect(host.textContent).not.toContain("sensitive");
	failing = "";
	let resolve!: () => void;
	pending = new Promise<void>((done) => {
		resolve = done;
	});
	await click("Install update");
	expect(button("Install update").disabled).toBe(true);
	expect(button("Check for updates").disabled).toBe(true);
	await click("Install update");
	expect(calls.filter((c) => c === "install_update")).toHaveLength(2);
	status.phase = "installing";
	await act(async () => resolve());
	pending = undefined;
	await settle();
	expect(host.textContent).toContain("Kolboo will close");
});

it("allows recovery from status-read and check failures without exposing diagnostics", async () => {
	failing = "get_update_status";
	await render();
	expect(host.textContent).toContain("Couldn’t read update status");
	expect(button("Check for updates").disabled).toBe(false);
	failing = "check_for_updates";
	await click("Check for updates");
	expect(host.querySelector('[role="alert"]')?.textContent).toContain(
		"Please try again",
	);
	expect(host.textContent).not.toContain("sensitive");
	failing = "";
	status.error = "safe backend failure";
	await click("Check for updates");
	expect(host.querySelector('[role="alert"]')).not.toBeNull();
	status.error = null;
	await settle(1000);
	expect(host.querySelector('[role="alert"]')).toBeNull();
});
