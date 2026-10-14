// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { AccountAdvancedPanel } from "./AccountAdvancedPanel";

const state = vi.hoisted(() => ({
	sync: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../../lib/queries/enterprisePersona", () => ({
	useEnterprisePersonaState: () => ({ data: null }),
	formatEnterprisePersonaLabel: () => "None",
}));
vi.mock("../../lib/queries", () => ({
	usePolicyState: () => ({ data: undefined }),
	usePolicySync: () => ({ mutateAsync: state.sync, isPending: false }),
	usePolicyDiagnosticsExport: () => ({
		mutateAsync: vi.fn(),
		isPending: false,
	}),
}));
vi.mock("@mantine/notifications", () => ({ notifications: { show: vi.fn() } }));

let host: HTMLDivElement;
let root: Root;
const simulate = vi.fn();
async function render(signedIn = false) {
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<AccountAdvancedPanel
					authContext={null}
					authContextMessage="No auth issue detected."
					signedIn={signedIn}
					refreshPending={false}
					onSimulateAuthFailure={simulate}
				/>
			</MantineProvider>,
		),
	);
}
async function click(text: string) {
	const button = [...host.querySelectorAll("button")].find((b) =>
		b.textContent?.includes(text),
	);
	expect(button).toBeDefined();
	await act(async () => button?.click());
}
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	vi.clearAllMocks();
});
afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
});

it("keeps read-only policy diagnostics in Advanced, reachable without account login", async () => {
	await render();
	expect(
		host.querySelector('[role="region"]')?.getAttribute("aria-hidden"),
	).toBe("true");
	await click("Advanced");
	expect(
		host.querySelector('[role="region"]')?.getAttribute("aria-hidden"),
	).toBe("false");
	expect(host.textContent).toContain("Organization policy");
	expect(host.textContent).toContain("Read-only rules");
	expect(host.textContent).toContain("not edit the rules");
	expect(host.querySelectorAll("input, select, textarea")).toHaveLength(0);
	expect(
		[...host.querySelectorAll("button")].some(
			(b) => b.textContent === "Export diagnostics",
		),
	).toBe(true);
	await click("Sync now");
	expect(state.sync).toHaveBeenCalledWith(undefined);
	await click("Simulate auth failure");
	expect(simulate).not.toHaveBeenCalled();
	await render(true);
	await click("Simulate auth failure");
	expect(simulate).toHaveBeenCalledOnce();
});
