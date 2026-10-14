// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ProxySettings } from "../../lib/tauri";
import { NetworkSettings } from "./NetworkSettings";

const state = vi.hoisted(() => ({
	settings: undefined as { proxy_settings: ProxySettings } | undefined,
	apply: vi.fn(),
	save: vi.fn(),
}));
vi.mock("../../lib/queries", () => ({
	useSettings: () => ({ data: state.settings, isLoading: !state.settings }),
	useSystemProxyInfo: () => ({ data: {} }),
	useUpdateProxySettings: () => ({ mutate: state.apply }),
	useSaveProxySettings: () => ({ mutate: state.save }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../../lib/tauri", () => ({ tauriAPI: {} }));

let host: HTMLDivElement;
let root: Root;
async function render() {
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<NetworkSettings />
			</MantineProvider>,
		),
	);
}
async function select(label: string) {
	const input = [...host.querySelectorAll("label")].find(
		(item) => item.textContent === label,
	);
	expect(input).toBeDefined();
	await act(async () => input?.click());
}
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	vi.clearAllMocks();
	state.settings = undefined;
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
});
afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
});

it("keeps an empty Manual draft editable through settings refreshes without saving or activating it", async () => {
	await render();
	state.settings = {
		proxy_settings: {
			mode: "system",
			manual: {
				proxy_url: "",
				no_proxy: "localhost",
				username: "",
				password: "",
			},
			trusted_ca_certificates: [],
			danger_accept_invalid_certs: false,
		},
	};
	await render();
	await select("Manual");
	expect(host.textContent).toContain("Required to enable Manual mode");
	expect(state.save).not.toHaveBeenCalled();
	expect(state.apply).not.toHaveBeenCalled();
	state.settings = structuredClone(state.settings);
	await render();
	expect(
		host.querySelector('input[placeholder="http://127.0.0.1:8080"]'),
	).not.toBeNull();
	expect(host.textContent).toContain("Effective mode: System");
	const url = host.querySelector<HTMLInputElement>('input[placeholder="http://127.0.0.1:8080"]');
	expect(url).not.toBeNull();
	await act(async () => {
		const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
		setter?.call(url, "http://127.0.0.1:9090");
		url?.dispatchEvent(new Event("input", { bubbles: true }));
	});
	state.settings = structuredClone(state.settings);
	await render();
	expect(url?.value).toBe("http://127.0.0.1:9090");
	await select("No proxy");
	expect(state.apply).toHaveBeenCalledWith(
		expect.objectContaining({ mode: "no_proxy" }),
		expect.any(Object),
	);
	state.settings.proxy_settings.mode = "no_proxy";
	state.settings.proxy_settings.manual = {
		proxy_url: "http://127.0.0.1:8080",
		no_proxy: "example.test",
		username: "proxy-user",
		password: "synthetic",
	};
	await render();
	await select("Manual");
	expect(state.apply).toHaveBeenLastCalledWith(
		expect.objectContaining({
			mode: "manual",
			manual: state.settings.proxy_settings.manual,
		}),
		expect.any(Object),
	);
});
