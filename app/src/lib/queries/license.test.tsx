// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { licenseAPI } from "../tauri";
import { useRequestLicenseEmailCode, useVerifyLicenseEmailCode } from "./license";

vi.mock("../tauri", () => ({ licenseAPI: {
    requestEmailCode: vi.fn(), verifyEmailCode: vi.fn(),
}, tauriAPI: {}, configAPI: {}, dataAPI: {}, llmAPI: {}, logsAPI: {}, recordingsAPI: {}, sttAPI: {} }));

it("requests and verifies email codes through the adapter, invalidating account and managed access only on success", async () => {
    Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
    const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
    const invalidations = vi.spyOn(client, "invalidateQueries").mockResolvedValue();
    const host = document.createElement("div"); const root = createRoot(host);
    let request!: ReturnType<typeof useRequestLicenseEmailCode>;
    let verify!: ReturnType<typeof useVerifyLicenseEmailCode>;
    function Reader() { request = useRequestLicenseEmailCode(); verify = useVerifyLicenseEmailCode(); return null; }
    try {
        await act(async () => root.render(<QueryClientProvider client={client}><Reader /></QueryClientProvider>));
        vi.mocked(licenseAPI.requestEmailCode).mockResolvedValue(undefined);
        await act(async () => request.mutateAsync("person@example.test"));
        expect(licenseAPI.requestEmailCode).toHaveBeenCalledWith("person@example.test");
        expect(invalidations).not.toHaveBeenCalled();
        vi.mocked(licenseAPI.verifyEmailCode).mockRejectedValueOnce(new Error("Invalid code"));
        await act(async () => { await expect(verify.mutateAsync({ email: "person@example.test", code: "000000" })).rejects.toThrow("Invalid code"); });
        expect(invalidations).not.toHaveBeenCalled();
        vi.mocked(licenseAPI.verifyEmailCode).mockResolvedValue({ tier: "personal" } as Awaited<ReturnType<typeof licenseAPI.verifyEmailCode>>);
        await act(async () => verify.mutateAsync({ email: "person@example.test", code: "123456" }));
        expect(licenseAPI.verifyEmailCode).toHaveBeenLastCalledWith("person@example.test", "123456");
        const keys = invalidations.mock.calls.map(([options]) => options?.queryKey);
        expect(keys).toContainEqual(["licenseState"]);
        expect(keys).toContainEqual(["licenseAuthContext"]);
        expect(keys).toContainEqual(["managedModels"]);
    } finally { await act(async () => root.unmount()); client.clear(); }
});
