// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act, type ComponentProps } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { AccountActionsCard } from "./AccountActionsCard";

let host: HTMLDivElement;
let root: Root;
let props: ComponentProps<typeof AccountActionsCard>;
async function render(changes: Partial<typeof props> = {}) {
	props = { ...props, ...changes };
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<AccountActionsCard {...props} />
			</MantineProvider>,
		),
	);
}
async function click(text: string) {
	const button = [...host.querySelectorAll("button")].find(
		(button) => button.textContent === text,
	);
	expect(button).toBeDefined();
	await act(async () => button?.click());
}
async function fill(selector: string, value: string) {
	const input = host.querySelector(selector) as HTMLInputElement;
	await act(async () => {
		Object.getOwnPropertyDescriptor(
			HTMLInputElement.prototype,
			"value",
		)?.set?.call(input, value);
		input.dispatchEvent(new Event("input", { bubbles: true }));
	});
}
async function submit() {
	await act(async () =>
		host
			.querySelector("form")
			?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })),
	);
}
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	props = {
		signedIn: false,
		reauthRequired: false,
		loginPending: false,
		signupPending: false,
		refreshPending: false,
		logoutPending: false,
		managePending: false,
		manageAvailable: false,
		onPasswordSignIn: vi.fn(),
		onPasswordSignUp: vi.fn(),
		onBrowserSignIn: vi.fn(),
		onRefresh: vi.fn(),
		onManage: vi.fn(),
		onSignOut: vi.fn(),
	};
});
afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
});
it("defaults to sign in with a short form and preserves browser recovery", async () => {
	await render();
	expect(host.textContent).toContain("Sign in to Kolboo");
	expect(host.textContent).not.toContain("ACCOUNT ACCESS");
	expect(host.textContent).toContain("without an account");
	expect(
		host.querySelector('[autocomplete="current-password"]'),
	).not.toBeNull();
	await fill('input[type="email"]', "person@example.test");
	await fill('input[type="password"]', "test-pass");
	await submit();
	expect(props.onPasswordSignIn).toHaveBeenCalledWith(
		"person@example.test",
		"test-pass",
	);
	expect(props.onPasswordSignUp).not.toHaveBeenCalled();
	await click("Continue in browser");
	expect(props.onBrowserSignIn).toHaveBeenCalledOnce();
	await click("Forgot password?");
	expect(props.onBrowserSignIn).toHaveBeenCalledTimes(2);
});
it("switches account creation explicitly and prevents duplicate submits while busy", async () => {
	await render();
	await click("Create an account");
	expect(host.querySelector('[autocomplete="new-password"]')).not.toBeNull();
	await fill('input[type="email"]', "new@example.test");
	await fill('input[type="password"]', "new-pass");
	await submit();
	expect(props.onPasswordSignUp).toHaveBeenCalledWith(
		"new@example.test",
		"new-pass",
	);
	await render({ signupPending: true });
	await submit();
	expect(props.onPasswordSignUp).toHaveBeenCalledOnce();
	expect(
		(host.querySelector('input[type="email"]') as HTMLInputElement).disabled,
	).toBe(true);
	await render({ signupPending: false });
	await click("Already have an account? Sign in");
	expect(
		host.querySelector('[autocomplete="current-password"]'),
	).not.toBeNull();
});
it("shows only available signed-in actions and supports reauthentication", async () => {
	await render({ signedIn: true });
	expect(host.querySelector("form")).toBeNull();
	expect(host.textContent).not.toContain("Manage account");
	await click("Refresh access");
	await click("Sign out");
	expect(props.onRefresh).toHaveBeenCalledOnce();
	expect(props.onSignOut).toHaveBeenCalledOnce();
	await render({ manageAvailable: true, reauthRequired: true });
	await click("Manage account");
	expect(props.onManage).toHaveBeenCalledOnce();
	expect(host.querySelector("form")).not.toBeNull();
	expect(host.textContent).not.toContain("Create an account");
	await submit();
	expect(props.onPasswordSignIn).toHaveBeenCalledOnce();
});
