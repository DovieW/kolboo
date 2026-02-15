import { describe, expect, it, vi } from "vitest";
import {
	createLicenseStateQueryFn,
	createRefreshLicenseEntitlementMutationFn,
} from "./queries";

describe("license query-layer function builders", () => {
	it("forwards license state reads to tauri api", async () => {
		const getLicenseState = vi.fn(async () => ({
			tier: "personal" as const,
			status: "active" as const,
			user_id: "user_123",
			email: "user@example.com",
			org: null,
			expires_at: null,
			cached_at: "2026-01-01T00:00:00Z",
			last_validated_at: null,
			usage: {
				stt_seconds_used: 0,
				llm_tokens_used: 0,
				requests_today: 0,
			},
			limits: {
				stt_seconds_monthly: 0,
				llm_tokens_monthly: 0,
				requests_per_day: 0,
			},
		}));
		const queryFn = createLicenseStateQueryFn({ getLicenseState });

		await expect(queryFn()).resolves.toMatchObject({ status: "active" });
		expect(getLicenseState).toHaveBeenCalledTimes(1);
	});

	it("forwards refresh failure simulation without swallowing errors", async () => {
		const refreshEntitlement = vi
			.fn<(simulateFailure?: boolean) => Promise<never>>()
			.mockRejectedValueOnce(new Error("refresh failed"));
		const mutationFn = createRefreshLicenseEntitlementMutationFn({
			refreshEntitlement,
		});

		await expect(mutationFn(true)).rejects.toThrow("refresh failed");
		expect(refreshEntitlement).toHaveBeenCalledWith(true);
	});
});
