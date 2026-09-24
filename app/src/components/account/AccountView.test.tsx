// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
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
}));
vi.mock("../../lib/queries", () => ({
	useLicenseState: () => ({
		data: state.license,
		isLoading: state.loading,
		error: state.error,
	}),
	useLicenseAuthContext: () => ({ data: undefined, isLoading: state.loading }),
	useLogoutLicense: () => ({ mutate: vi.fn() }),
	useRefreshLicenseEntitlement: () => ({ mutate: vi.fn() }),
	useSignUpLicense: () => ({ mutate: vi.fn() }),
	useStartLicenseLogin: () => ({ mutate: vi.fn() }),
}));
vi.mock("./AccountIdentityCard", () => ({ AccountIdentityCard: () => null }));
vi.mock("./AccountAdvancedPanel", () => ({ AccountAdvancedPanel: () => null }));
vi.mock("./AccountUsageCard", () => ({ AccountUsageCard: () => null }));
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
});
afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
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
