// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { PrivacySettings } from "./PrivacySettings";

it("provides privacy guidance without the retired payload switch and retains data/key/network navigation", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const navigate = vi.fn();
	try {
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<PrivacySettings onNavigateToTab={navigate} />
				</MantineProvider>,
			),
		);
		expect(host.textContent).not.toContain("Privacy mode for payloads");
		expect(host.querySelector('[role="switch"]')).toBeNull();
		expect(host.textContent).toContain("History");
		expect(host.textContent).toContain("Recordings");
		for (const [label, tab] of [
			["Manage stored data", "data"],
			["Manage provider keys", "api-keys"],
			["Network settings", "network"],
		]) {
			const button = [
				...host.querySelectorAll<HTMLButtonElement>("button"),
			].find((b) => b.textContent === label);
			expect(button).toBeDefined();
			await act(async () => button?.click());
			expect(navigate).toHaveBeenLastCalledWith(tab);
		}
	} finally {
		await act(async () => root.unmount());
		host.remove();
	}
});
