import { describe, expect, it } from "vitest";
import {
	formatLicenseStatusLabel,
	formatLicenseTierLabel,
	getOrgDisplayName,
	isLicenseStatusDegraded,
	isReauthRequiredReason,
	shouldShowOrgId,
} from "./AccountSettings";

const baseLicenseState = {
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
};

describe("AccountSettings helpers", () => {
	it("formats tier labels", () => {
		expect(formatLicenseTierLabel("community")).toBe("Community");
		expect(formatLicenseTierLabel("personal")).toBe("Personal");
		expect(formatLicenseTierLabel("enterprise")).toBe("Enterprise");
	});

	it("formats status labels", () => {
		expect(formatLicenseStatusLabel("signed_out")).toBe("Signed out");
		expect(formatLicenseStatusLabel("active")).toBe("Active");
		expect(formatLicenseStatusLabel("grace")).toBe("Grace");
		expect(formatLicenseStatusLabel("expired")).toBe("Expired");
	});

	it("marks degraded states", () => {
		expect(isLicenseStatusDegraded("active")).toBe(false);
		expect(isLicenseStatusDegraded("signed_out")).toBe(false);
		expect(isLicenseStatusDegraded("grace")).toBe(true);
		expect(isLicenseStatusDegraded("expired")).toBe(true);
	});

	it("renders org context only when org is present", () => {
		expect(getOrgDisplayName(undefined)).toBe("—");
		expect(getOrgDisplayName({ ...baseLicenseState, org: null })).toBe("—");
		expect(
			getOrgDisplayName({
				...baseLicenseState,
				tier: "enterprise",
				org: {
					org_id: "org_123",
					org_name: "Acme Co",
				},
			}),
		).toBe("Acme Co");

		expect(shouldShowOrgId(undefined)).toBe(false);
		expect(shouldShowOrgId({ ...baseLicenseState, org: null })).toBe(false);
		expect(
			shouldShowOrgId({
				...baseLicenseState,
				tier: "enterprise",
				org: {
					org_id: "org_123",
					org_name: "Acme Co",
				},
			}),
		).toBe(true);
	});

	it("identifies reasons that require re-authentication", () => {
		expect(isReauthRequiredReason("reauth_required")).toBe(true);
		expect(isReauthRequiredReason("token_invalid")).toBe(true);
		expect(isReauthRequiredReason("policy_denied")).toBe(false);
		expect(isReauthRequiredReason(null)).toBe(false);
	});
});
