// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { TelemetryDisclosureContent } from "./TelemetryDisclosureModal";

const host = document.createElement("div");
let root = createRoot(host);

beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
});
afterEach(async () => {
	await act(async () => root.unmount());
	root = createRoot(host);
	host.replaceChildren();
});

function button(label: string): HTMLButtonElement | undefined {
	return [...host.querySelectorAll("button")].find(
		(item) => item.textContent?.trim() === label,
	);
}

it("keeps collection details collapsed while offering two explicit choices", async () => {
	const onDisableAnalytics = vi.fn();
	const onAllowAnalytics = vi.fn();
	const render = (loading: boolean) =>
		root.render(
			<MantineProvider>
				<TelemetryDisclosureContent
					analyticsPolicyEnforced={false}
					analyticsPolicyReason={null}
					loading={loading}
					onDisableAnalytics={onDisableAnalytics}
					onAllowAnalytics={onAllowAnalytics}
				/>
			</MantineProvider>,
		);
	await act(async () => render(false));
	expect(button("What’s shared?")?.getAttribute("aria-expanded")).toBe("false");
	expect(
		host.querySelector('[role="region"]')?.getAttribute("aria-hidden"),
	).toBe("true");
	await act(async () => button("What’s shared?")?.click());
	expect(button("What’s shared?")?.getAttribute("aria-expanded")).toBe("true");
	expect(
		host.querySelector('[role="region"]')?.getAttribute("aria-hidden"),
	).toBe("false");
	expect(host.textContent).toContain(
		"Recording events include rounded duration",
	);
	expect(host.textContent).toContain("network request’s IP address");
	await act(async () => button("Basic usage only")?.click());
	await act(async () => button("Allow linked analytics")?.click());
	expect(onDisableAnalytics).toHaveBeenCalledOnce();
	expect(onAllowAnalytics).toHaveBeenCalledOnce();
	await act(async () => render(true));
	expect(button("Basic usage only")?.disabled).toBe(true);
	expect(button("Allow linked analytics")?.disabled).toBe(true);
});

it("offers only Continue when policy disables analytics and hides policy details", async () => {
	const onDisableAnalytics = vi.fn();
	const onAllowAnalytics = vi.fn();
	await act(async () =>
		root.render(
			<MantineProvider>
				<TelemetryDisclosureContent
					analyticsPolicyEnforced
					analyticsPolicyReason="Workspace policy"
					loading={false}
					onDisableAnalytics={onDisableAnalytics}
					onAllowAnalytics={onAllowAnalytics}
				/>
			</MantineProvider>,
		),
	);
	expect(button("Allow linked analytics")).toBeUndefined();
	expect(
		host.querySelector('[role="region"]')?.getAttribute("aria-hidden"),
	).toBe("true");
	await act(async () => button("What’s shared?")?.click());
	expect(host.textContent).toContain("Policy reason: Workspace policy");
	await act(async () => button("Continue")?.click());
	expect(onDisableAnalytics).toHaveBeenCalledOnce();
	expect(onAllowAnalytics).not.toHaveBeenCalled();
});
