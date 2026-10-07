// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { EmailCodeSignIn, type EmailCodeSignInActions } from "./EmailCodeSignIn";

let host: HTMLDivElement;
let root: Root;
let actions: EmailCodeSignInActions;
async function render() {
    await act(async () => root.render(<MantineProvider env="test"><EmailCodeSignIn actions={actions} /></MantineProvider>));
}
function button(text: string) {
    const result = [...host.querySelectorAll("button")].find(button => button.textContent === text);
    expect(result).toBeDefined(); return result as HTMLButtonElement;
}
async function click(text: string) { await act(async () => button(text).click()); }
async function fill(selector: string, value: string) {
    const input = host.querySelector(selector) as HTMLInputElement;
    expect(input).not.toBeNull();
    await act(async () => {
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(input, value);
        input.dispatchEvent(new Event("input", { bubbles: true }));
    });
}
async function submit() {
    await act(async () => host.querySelector("form")?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })));
}
function deferred() {
    let resolve!: (value?: unknown) => void; let reject!: (error: Error) => void;
    const promise = new Promise<unknown>((yes, no) => { resolve = yes; reject = no; });
    return { promise, resolve, reject };
}
beforeEach(() => {
    Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
    vi.useFakeTimers(); vi.setSystemTime(new Date("2026-10-06T12:00:00Z"));
    host = document.createElement("div"); document.body.append(host); root = createRoot(host);
    actions = { requestCode: vi.fn().mockResolvedValue(undefined), verifyCode: vi.fn().mockResolvedValue(undefined),
        passwordSignIn: vi.fn().mockResolvedValue(undefined), resetPassword: vi.fn().mockResolvedValue(undefined),
        browserSignIn: vi.fn().mockResolvedValue(undefined), cancel: vi.fn().mockResolvedValue(undefined) };
});
afterEach(async () => { await act(async () => root.unmount()); host.remove(); vi.useRealTimers(); });
it("requests a code, enforces resend cooldown and verifies without retaining the code", async () => {
    await render(); expect(host.querySelector('input[type="password"]')).toBeNull();
    await fill('input[type="email"]', "person@example.test"); await submit();
    expect(actions.requestCode).toHaveBeenCalledWith("person@example.test");
    expect(host.textContent).toContain("Code sent to person@example.test");
    expect(button("Resend in 60s").disabled).toBe(true);
    await act(async () => vi.advanceTimersByTime(60_000)); await click("Resend code");
    expect(actions.requestCode).toHaveBeenCalledTimes(2);
    await fill('[autocomplete="one-time-code"]', "123456"); await submit();
    expect(actions.verifyCode).toHaveBeenCalledWith("person@example.test", "123456");
    expect((host.querySelector('[autocomplete="one-time-code"]') as HTMLInputElement).value).toBe("");
    await click("Change email");
    expect(actions.cancel).toHaveBeenCalledOnce();
    expect(host.querySelector('input[type="email"]')).not.toBeNull();
});
it("preserves legacy password login and routes password reset to the reset action", async () => {
    await render(); await fill('input[type="email"]', "legacy@example.test"); await click("Use password");
    await fill('input[type="password"]', "synthetic-password"); await click("Forgot password?");
    expect(actions.resetPassword).toHaveBeenCalledWith("legacy@example.test");
    expect(actions.browserSignIn).not.toHaveBeenCalled();
    expect(host.textContent).toContain("Check your email");
    await submit();
    expect(actions.passwordSignIn).toHaveBeenCalledWith("legacy@example.test", "synthetic-password");
    expect((host.querySelector('input[type="password"]') as HTMLInputElement).value).toBe("");
    await click("Use email code"); await click("Use browser"); expect(actions.browserSignIn).toHaveBeenCalledOnce();
});
it("shows send and invalid-code errors without losing the entered email or code", async () => {
    vi.mocked(actions.requestCode).mockRejectedValueOnce(new Error("Invitation required"));
    await render(); await fill('input[type="email"]', "person@example.test"); await submit();
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Invitation required");
    expect((host.querySelector('input[type="email"]') as HTMLInputElement).value).toBe("person@example.test");
    await submit(); vi.mocked(actions.verifyCode).mockRejectedValueOnce(new Error("Code expired"));
    await fill('[autocomplete="one-time-code"]', "000000"); await submit();
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("Code expired");
    expect((host.querySelector('[autocomplete="one-time-code"]') as HTMLInputElement).value).toBe("000000");
});
it("prevents duplicate submits and ignores a cancelled verification response", async () => {
    await render(); await fill('input[type="email"]', "person@example.test"); await submit();
    const pending = deferred(); vi.mocked(actions.verifyCode).mockReturnValueOnce(pending.promise);
    await fill('[autocomplete="one-time-code"]', "123456"); await submit(); await submit();
    expect(actions.verifyCode).toHaveBeenCalledOnce();
    await click("Cancel"); await act(async () => pending.reject(new Error("Late response")));
    expect(host.querySelector('input[type="email"]')).not.toBeNull();
    expect(host.textContent).not.toContain("Late response");
});
it("can cancel a pending send, ignores late success and cancels work on unmount", async () => {
    const pending = deferred(); vi.mocked(actions.requestCode).mockReturnValueOnce(pending.promise);
    await render(); await fill('input[type="email"]', "person@example.test"); await submit(); await click("Cancel");
    await act(async () => pending.resolve());
    expect(host.textContent).not.toContain("Code sent");
    const browser = deferred(); vi.mocked(actions.browserSignIn).mockReturnValueOnce(browser.promise);
    vi.mocked(actions.cancel).mockRejectedValueOnce(new Error("Unmount cancellation failed"));
    await click("Use browser"); await act(async () => root.render(null));
    expect(actions.cancel).toHaveBeenCalledTimes(2);
    await act(async () => browser.reject(new Error("Late browser failure")));
    expect(host.textContent).toBe("");
});
it("reports cancellation failure but discards errors from an older cancellation", async () => {
    const cancellation = deferred(); vi.mocked(actions.cancel).mockReturnValueOnce(cancellation.promise);
    await render(); await fill('input[type="email"]', "person@example.test"); await submit(); await click("Change email");
    await submit(); await act(async () => cancellation.reject(new Error("Old cancellation")));
    expect(host.textContent).not.toContain("Old cancellation");
    vi.mocked(actions.cancel).mockRejectedValueOnce(new Error("Cancel unavailable"));
    await click("Change email"); expect(host.textContent).toContain("Cancel unavailable");
});
