import { describe, expect, it, vi } from "vitest";
import {
	createLicenseStateQueryFn,
	createRefreshLicenseEntitlementMutationFn,
	createSignUpLicenseMutationFn,
} from "./queries/license";
import { createPolicySyncMutationFn } from "./queries/policy";
import {
	applySettingsQueryInvalidations,
	invalidateLicenseRelatedQueries,
	invalidateLogoutRelatedQueries,
	invalidatePolicyRelatedQueries,
	invalidateSettingsQueries,
} from "./queries/shared";

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
			portal_available: false,
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

	it("forwards license signup requests", async () => {
		const signUp = vi.fn(async () => ({
			confirmation_required: true,
			email: "new@example.com",
			state: {
				tier: "community" as const,
				status: "signed_out" as const,
				user_id: null,
				email: null,
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
				portal_available: false,
			},
		}));
		const mutationFn = createSignUpLicenseMutationFn({ signUp });

		await expect(
			mutationFn({ email: "new@example.com", password: "password123" }),
		).resolves.toMatchObject({ confirmation_required: true });
		expect(signUp).toHaveBeenCalledWith({
			email: "new@example.com",
			password: "password123",
		});
	});

	it("invalidates policy and settings queries when policy lock state changes", async () => {
		const invalidateQueries = vi.fn(async () => undefined);

		await invalidatePolicyRelatedQueries({ invalidateQueries });

		expect(invalidateQueries).toHaveBeenNthCalledWith(1, {
			queryKey: ["policyState"],
		});
		expect(invalidateQueries).toHaveBeenNthCalledWith(2, {
			queryKey: ["settings"],
		});
	});

	it("applies settings-runtime query invalidation decisions", async () => {
		const invalidateQueries = vi.fn(async () => undefined);

		await applySettingsQueryInvalidations({ invalidateQueries }, [
			{ queryKey: ["settings"], reason: "settings" },
			{ queryKey: ["policyState"], reason: "policy" },
		]);

		expect(invalidateQueries).toHaveBeenNthCalledWith(1, {
			queryKey: ["settings"],
		});
		expect(invalidateQueries).toHaveBeenNthCalledWith(2, {
			queryKey: ["policyState"],
		});
	});

	it("invalidates settings plus any extra query keys for shared settings mutations", async () => {
		const invalidateQueries = vi.fn(async () => undefined);

		await invalidateSettingsQueries({ invalidateQueries }, [
			{ queryKey: ["requestLogs"], reason: "settings" },
		]);

		expect(invalidateQueries).toHaveBeenNthCalledWith(1, {
			queryKey: ["settings"],
		});
		expect(invalidateQueries).toHaveBeenNthCalledWith(2, {
			queryKey: ["requestLogs"],
		});
	});

	it("invalidates license queries when auth state changes", async () => {
		const invalidateQueries = vi.fn(async () => undefined);

		await invalidateLicenseRelatedQueries({ invalidateQueries });

		expect(invalidateQueries).toHaveBeenNthCalledWith(1, {
			queryKey: ["licenseState"],
		});
		expect(invalidateQueries).toHaveBeenNthCalledWith(2, {
			queryKey: ["licenseAuthContext"],
		});
	});

	it("invalidates auth and policy queries on logout", async () => {
		const invalidateQueries = vi.fn(async () => undefined);

		await invalidateLogoutRelatedQueries({ invalidateQueries });

		expect(invalidateQueries).toHaveBeenCalledWith({
			queryKey: ["licenseState"],
		});
		expect(invalidateQueries).toHaveBeenCalledWith({
			queryKey: ["licenseAuthContext"],
		});
		expect(invalidateQueries).toHaveBeenCalledWith({
			queryKey: ["policyState"],
		});
		expect(invalidateQueries).toHaveBeenCalledWith({
			queryKey: ["settings"],
		});
	});

	it("policy sync mutation routes runtime side effects through sync policy", async () => {
		const syncPolicy = vi.fn(async () => ({ source: "cloud" }));
		const invoke = vi.fn(async () => undefined);
		const emitSettingsChanged = vi.fn(async () => undefined);
		const mutationFn = createPolicySyncMutationFn({
			syncPolicy,
			invoke,
			emitSettingsChanged,
		});

		await expect(
			mutationFn({ policyPack: { enforced_fields: [] } }),
		).resolves.toEqual({ source: "cloud" });

		expect(syncPolicy).toHaveBeenCalledWith({
			policyPack: { enforced_fields: [] },
		});
		expect(invoke).toHaveBeenCalledTimes(1);
		expect(invoke).toHaveBeenCalledWith("sync_pipeline_config");
		expect(emitSettingsChanged).not.toHaveBeenCalled();
	});
});
