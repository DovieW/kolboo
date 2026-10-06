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
    await act(async () => root.render(<MantineProvider env="test"><AccountActionsCard {...props} /></MantineProvider>));
}
async function click(text: string) {
    const button = [...host.querySelectorAll("button")].find(button => button.textContent === text);
    expect(button).toBeDefined();
    await act(async () => button?.click());
}
beforeEach(() => {
    Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
    host = document.createElement("div"); document.body.append(host); root = createRoot(host);
    props = { signedIn: false, reauthRequired: false, refreshPending: false, logoutPending: false,
        managePending: false, manageAvailable: false, authForm: <form aria-label="Authentication" />,
        onRefresh: vi.fn(), onManage: vi.fn(), onSignOut: vi.fn() };
});
afterEach(async () => { await act(async () => root.unmount()); host.remove(); });
it("shows only authentication while signed out", async () => {
    await render();
    expect(host.textContent).toContain("Sign in to Kolboo");
    expect(host.querySelector("form")).not.toBeNull();
    expect(host.querySelector("button")).toBeNull();
});
it("shows available account actions and preserves reauthentication", async () => {
    await render({ signedIn: true });
    expect(host.textContent).toContain("Your account");
    expect(host.querySelector("form")).toBeNull();
    expect(host.textContent).not.toContain("Manage account");
    await click("Refresh access"); await click("Sign out");
    expect(props.onRefresh).toHaveBeenCalledOnce(); expect(props.onSignOut).toHaveBeenCalledOnce();
    await render({ manageAvailable: true, reauthRequired: true });
    expect(host.querySelector("form")).not.toBeNull();
    expect(host.textContent).toContain("Sign in to Kolboo");
    await click("Manage account"); expect(props.onManage).toHaveBeenCalledOnce();
    await render({ refreshPending: true, logoutPending: true, managePending: true });
    for (const button of host.querySelectorAll("button")) expect(button.disabled).toBe(true);
});
