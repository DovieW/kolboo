// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { notifications } from "@mantine/notifications";
import { openUrl } from "@tauri-apps/plugin-opener";
import { tauriAPI } from "../../lib/tauri";
import { AccountView } from "./AccountView";

const state = vi.hoisted(() => ({
	license: undefined as
		| undefined
		| {
				status: string;
				tier: string;
				email?: string;
				org?: { org_name: string };
		  },
	loading: false,
	error: null as Error | null,
	refresh: vi.fn(),
	logout: vi.fn(),
}));
vi.mock("@mantine/notifications", () => ({ notifications: { show: vi.fn() } }));
vi.mock("@tauri-apps/plugin-opener", () => ({
	openUrl: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../../lib/tauri", () => ({
	tauriAPI: { getLicenseManagementUrl: vi.fn() },
}));
vi.mock("../../lib/queries", () => ({
	useLicenseState: () => ({
		data: state.license,
		isLoading: state.loading,
		error: state.error,
	}),
	useLicenseAuthContext: () => ({ data: undefined, isLoading: state.loading }),
	useLogoutLicense: () => ({ mutate: state.logout }),
	useRefreshLicenseEntitlement: () => ({ mutate: state.refresh }),
	useSignUpLicense: () => ({ mutate: vi.fn() }),
	useStartLicenseLogin: () => ({ mutate: vi.fn() }),
}));
vi.mock("./AccountIdentityCard", () => ({ AccountIdentityCard: () => null }));
vi.mock("./AccountAdvancedPanel", () => ({
	AccountAdvancedPanel: (props: { onSimulateAuthFailure: () => void }) => (
		<button type="button" onClick={props.onSimulateAuthFailure}>
			Simulate auth failure
		</button>
	),
}));
vi.mock("./AccountUsageCard", () => ({ AccountUsageCard: () => null }));
vi.mock("./AccountAuthentication", () => ({
	AccountAuthentication: () => <form aria-label="Authentication" />,
}));
let host: HTMLDivElement;
let root: Root;
async function render() {
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<AccountView />
			</MantineProvider>,
		),
	);
}
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	state.license = undefined;
	state.loading = false;
	state.error = null;
	vi.clearAllMocks();
});
it("runs refresh, sign-out and management actions and reports failures without losing the account", async () => {
	state.license = {
		status: "active",
		tier: "personal",
		portal_available: true,
	} as typeof state.license;
	state.refresh.mockImplementation((_value, callbacks) =>
		callbacks.onError(new Error("Offline")),
	);
	state.logout.mockImplementation((_value, callbacks) =>
		callbacks.onError(new Error("Wallet unavailable")),
	);
	const click = async (text: string) => {
		const target = [...host.querySelectorAll("button")].find(
			(button) => button.textContent === text,
		);
		expect(target).toBeDefined();
		await act(async () => target?.click());
	};
	await render();
	await click("Refresh access");
	expect(state.refresh).toHaveBeenCalledWith(false, expect.any(Object));
	expect(notifications.show).toHaveBeenCalledWith(
		expect.objectContaining({ title: "Refresh failed", message: "Offline" }),
	);
	await click("Sign out");
	expect(notifications.show).toHaveBeenCalledWith(
		expect.objectContaining({
			title: "Sign-out failed",
			message: "Wallet unavailable",
		}),
	);
	await click("Account details");
	await click("Simulate auth failure");
	expect(state.refresh).toHaveBeenLastCalledWith(true, expect.any(Object));
	vi.mocked(tauriAPI.getLicenseManagementUrl).mockResolvedValueOnce(
		"https://account.example.test",
	);
	await click("Manage account");
	expect(openUrl).toHaveBeenCalledWith("https://account.example.test");
	vi.mocked(tauriAPI.getLicenseManagementUrl).mockRejectedValueOnce(
		new Error("Not available"),
	);
	await click("Manage account");
	expect(notifications.show).toHaveBeenCalledWith(
		expect.objectContaining({
			title: "Unable to open account management",
			message: "Not available",
		}),
	);
	expect(host.textContent).toContain("Your account");
});
afterEach(async () => {
	vi.unstubAllEnvs();
	await act(async () => root.unmount());
	host.remove();
});
it("keeps Community packages account-free even with a cached Pro session, without clearing it", async () => {
	vi.stubEnv("VITE_CLOUD_SERVICE_ENABLED", "false");
	await render();
	expect(host.textContent).toContain("Accounts will be available in a later update");
	expect(host.querySelector("form")).toBeNull();
	expect(host.querySelector("button")).toBeNull();
	state.license = { status: "active", tier: "personal" };
	await render();
	expect(host.textContent).not.toContain("Pro active");
	expect(host.textContent).not.toContain("Refresh access");
	expect(host.textContent).not.toContain("Manage account");
	expect(state.logout).not.toHaveBeenCalled();
	await act(async () => host.querySelector<HTMLButtonElement>("button")?.click());
	expect(state.logout).toHaveBeenCalledWith(undefined, expect.any(Object));
	expect(state.refresh).not.toHaveBeenCalled();
});
it("shows one compact sign-in form instead of duplicate signed-out summary cards", async () => {
	state.license = { status: "signed_out", tier: "community" };
	await render();
	expect(host.querySelector(".account-signin-layout")).not.toBeNull();
	expect(host.querySelectorAll("form")).toHaveLength(1);
	expect(host.textContent).not.toContain("ACCOUNT ACCESS");
	expect(host.textContent).not.toContain("Not signed in");
	state.error = new Error("Offline");
	await render();
	expect(host.textContent).toContain("Offline");
	expect(host.querySelector("form")).not.toBeNull();
});
it("keeps signed-in details and exits the narrow login layout", async () => {
	state.license = { status: "active", tier: "personal" };
	await render();
	expect(host.querySelector(".account-signin-layout")).toBeNull();
	expect(host.querySelector("form")).toBeNull();
	expect(host.textContent).toContain("Your account");
	state.license = {
		status: "active",
		tier: "enterprise",
		email: "demo@example.test",
		org: { org_name: "Example" },
	};
	await render();
	expect(host.textContent).toContain("demo@example.test");
	expect(host.textContent).toContain("Example");
	state.loading = true;
	await render();
	expect(host.querySelector("form")).toBeNull();
});
