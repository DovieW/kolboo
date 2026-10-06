// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { AccountAuthentication } from "./AccountAuthentication";
import type { EmailCodeSignInActions } from "./EmailCodeSignIn";
const calls = vi.hoisted(() => ({ request: vi.fn(), verify: vi.fn(), login: vi.fn(), reset: vi.fn(), cancel: vi.fn() }));
vi.mock("../../lib/queries", () => ({
    useRequestLicenseEmailCode: () => ({ mutateAsync: calls.request }),
    useVerifyLicenseEmailCode: () => ({ mutateAsync: calls.verify }),
    useStartLicenseLogin: () => ({ mutateAsync: calls.login }),
    useRequestLicensePasswordReset: () => ({ mutateAsync: calls.reset }),
}));
vi.mock("../../lib/tauri", () => ({ licenseAPI: { cancelLogin: calls.cancel } }));
let actions: EmailCodeSignInActions;
vi.mock("./EmailCodeSignIn", () => ({ EmailCodeSignIn: (props: { actions: EmailCodeSignInActions }) => { actions = props.actions; return <form />; } }));
it("wires every sign-in action to its owned mutation, including cancellation", async () => {
    Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
    const host = document.createElement("div");
    const root = createRoot(host);
    try {
        await act(async () => root.render(<AccountAuthentication />));
        expect(host.querySelector("form")).not.toBeNull();
        await actions.requestCode("person@example.test");
        await actions.verifyCode("person@example.test", "123456");
        await actions.passwordSignIn("person@example.test", "synthetic-password");
        await actions.browserSignIn();
        await actions.resetPassword("person@example.test");
        await actions.cancel();
        expect(calls.request).toHaveBeenCalledWith("person@example.test");
        expect(calls.verify).toHaveBeenCalledWith({ email: "person@example.test", code: "123456" });
        expect(calls.login.mock.calls).toEqual([[{ email: "person@example.test", password: "synthetic-password" }], [undefined]]);
        expect(calls.reset).toHaveBeenCalledWith("person@example.test");
        expect(calls.cancel).toHaveBeenCalledOnce();
    } finally { await act(async () => root.unmount()); }
});
it("does not offer or invoke authentication in a Community package", async () => {
    vi.stubEnv("VITE_CLOUD_SERVICE_ENABLED", "false");
    vi.clearAllMocks();
    Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
    const host = document.createElement("div");
    const root = createRoot(host);
    try {
        await act(async () => root.render(<AccountAuthentication />));
        expect(host.querySelector("form")).toBeNull();
        expect(host.textContent).toContain("Use your own provider keys in Settings");
        for (const action of Object.values(calls)) expect(action).not.toHaveBeenCalled();
    } finally { await act(async () => root.unmount()); vi.unstubAllEnvs(); }
});
