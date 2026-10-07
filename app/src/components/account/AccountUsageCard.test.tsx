// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import type { LicenseState } from "../../lib/tauri";
import { AccountUsageCard } from "./AccountUsageCard";
it("shows Pro allowances and UTC resets without implying a bill; Community has no managed usage", async () => {
    Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
    const host = document.createElement("div");
    const root = createRoot(host);
    const state = { beta_access: { source: "complimentary_beta", status: "approved", limits: { stt_seconds_monthly: 18000, llm_tokens_monthly: 500000, requests_per_day: 500 } }, usage_period: { period_start: "2026-10-01T00:00:00Z", monthly_reset_at: "2026-11-01T00:00:00Z", daily_reset_at: "2026-10-07T00:00:00Z" }, usage: { stt_seconds_used: 3600, llm_tokens_used: 10, requests_today: 2 }, limits: { stt_seconds_monthly: 18000, llm_tokens_monthly: 500000, requests_per_day: 500 } } as LicenseState;
    const render = async (modeLabel: "Community" | "Pro", licenseState: LicenseState | null, loading = false) => {
        await act(async () => root.render(<MantineProvider env="test"><AccountUsageCard modeLabel={modeLabel} licenseState={licenseState} loading={loading} /></MantineProvider>));
    };
    try {
        await render("Pro", state);
        expect(host.textContent).toContain("Complimentary Pro beta");
        expect(host.textContent).toContain("1 / 5 hours");
        expect(host.textContent).toContain("11/1/2026");
        expect(host.textContent).toContain("00:00 UTC");
        expect(host.textContent).not.toMatch(/bill|payment/i);
        await render("Community", state);
        expect(host.querySelector(".account-panel")).toBeNull();
        await render("Pro", null, true);
        expect(host.querySelectorAll(".mantine-Skeleton-root")).toHaveLength(3);
        await render("Pro", null);
        expect(host.querySelector(".account-panel")).toBeNull();
        await render("Pro", { ...state, beta_access: null, usage_period: null, usage: { stt_seconds_used: 17000, llm_tokens_used: 400000, requests_today: 499 } });
        expect(host.textContent).not.toContain("Complimentary");
        expect(host.textContent).not.toContain("Monthly reset");
        expect(host.textContent).toContain("499 / 500 today");
    } finally { await act(async () => root.unmount()); }
});
