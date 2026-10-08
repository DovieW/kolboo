// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS_VALUES } from "../../lib/tauri/settingsDefaults";
import { StartupWindowSetting } from "./StartupWindowSetting";

const state = vi.hoisted(() => ({
	enabled: false,
	readFailure: false,
	writeFailure: false,
	readPending: undefined as Promise<unknown> | undefined,
	writePending: undefined as Promise<unknown> | undefined,
}));
vi.mock("@tauri-apps/plugin-store", () => ({
	Store: { load: async () => ({ get: async () => undefined }) },
}));
vi.mock("../../lib/tauri", async (original) => {
	const actual = await original<typeof import("../../lib/tauri")>();
	return {
		...actual,
		tauriAPI: {
			...actual.tauriAPI,
			getSettings: async () => {
				if (state.readFailure) throw new Error("read failed");
				await state.readPending;
				return {
					...DEFAULT_SETTINGS_VALUES,
					main_window_show_on_launch: state.enabled,
				};
			},
		},
	};
});
let host: HTMLDivElement;
let root: Root;
let client: QueryClient;
let writes: boolean[];
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	vi.useFakeTimers();
	Object.assign(state, {
		enabled: false,
		readFailure: false,
		writeFailure: false,
		readPending: undefined,
		writePending: undefined,
	});
	writes = [];
	mockIPC(async (command, args) => {
		if (command !== "settings_apply_patch")
			throw new Error(`Unexpected command ${command}`);
		const patch = (args as { patch: { main_window_show_on_launch: boolean } })
			.patch;
		writes.push(patch.main_window_show_on_launch);
		await state.writePending;
		if (state.writeFailure) throw new Error("write failed");
		state.enabled = patch.main_window_show_on_launch;
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
					<StartupWindowSetting profileScope={profileScope} />
				</MantineProvider>
			</QueryClientProvider>,
		),
	);
	await settle();
}
function toggle() {
	const input = host.querySelector<HTMLInputElement>(
		'input[aria-label="Show window on launch"]',
	);
	if (!input) throw new Error("Startup window toggle was not rendered");
	return input;
}
async function click() {
	await act(async () => toggle().click());
	await settle();
}

it("defaults to tray-only and persists both directions with a refreshed settings query", async () => {
	await render();
	expect(toggle().checked).toBe(false);
	await click();
	expect(writes).toEqual([true]);
	expect(toggle().checked).toBe(true);
	await click();
	expect(writes).toEqual([true, false]);
	expect(toggle().checked).toBe(false);
});
it("shows an existing preference but prevents per-program startup overrides", async () => {
	state.enabled = true;
	await render(true);
	expect(toggle().checked).toBe(true);
	expect(toggle().disabled).toBe(true);
	await click();
	expect(writes).toEqual([]);
});
it("disables the toggle until loading finishes and if the read fails", async () => {
	let resolve!: () => void;
	state.readPending = new Promise<void>((done) => {
		resolve = done;
	});
	await render();
	expect(toggle().disabled).toBe(true);
	expect(toggle().checked).toBe(false);
	await act(async () => resolve());
	await settle();
	expect(toggle().disabled).toBe(false);
	state.readFailure = true;
	await act(async () => {
		await client.invalidateQueries({ queryKey: ["settings"] });
	});
	await settle();
	expect(toggle().disabled).toBe(true);
});
it("blocks duplicate writes and reports failure without pretending the setting was saved", async () => {
	await render();
	let resolve!: () => void;
	state.writePending = new Promise<void>((done) => {
		resolve = done;
	});
	state.writeFailure = true;
	await click();
	expect(toggle().disabled).toBe(true);
	await click();
	expect(writes).toEqual([true]);
	await act(async () => resolve());
	await settle();
	expect(toggle().checked).toBe(false);
	expect(host.textContent).toContain("Couldn’t save startup preference");
	state.writePending = undefined;
	state.writeFailure = false;
	await click();
	expect(toggle().checked).toBe(true);
	expect(host.textContent).not.toContain("Couldn’t");
});
