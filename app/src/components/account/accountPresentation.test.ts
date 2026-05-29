import { describe, expect, it } from "vitest";
import type { LicenseAuthContext, LicenseState } from "../../lib/tauri";
import {
	calculateUsagePercent,
	getAccountModeDescription,
	getAccountModeLabel,
	getAccountStatusColor,
	isReauthRequiredForSession,
	isReauthRequiredReason,
} from "./accountPresentation";

const baseState: LicenseState = {
	tier: "personal",
	status: "active",
	user_id: "user-123",
	email: "person@example.com",
	org: {
		org_id: "org-123",
		org_name: "Kolboo Labs",
		inference_mode: "managed",
	},
	expires_at: null,
	cached_at: "2026-04-21T12:00:00.000Z",
	last_validated_at: null,
	usage: {
		stt_seconds_used: 720,
		llm_tokens_used: 3400,
		requests_today: 5,
	},
	limits: {
		stt_seconds_monthly: 6000,
		llm_tokens_monthly: 10000,
		requests_per_day: 50,
	},
	portal_available: false,
};

const activeContext: LicenseAuthContext = {
	authenticated: true,
	secure_session_present: true,
	subject_id: "user-123",
	issuer: "https://issuer.example.test",
	mode: "personal",
	org_id: "org-123",
	entitlements: ["managed_inference"],
	policy_status: "allow",
	reason_code: null,
};

describe("accountPresentation", () => {
	it("maps enterprise managed access to Managed Business", () => {
		const state: LicenseState = {
			...baseState,
			tier: "enterprise",
		};

		expect(getAccountModeLabel(state, activeContext)).toBe("Managed Business");
	});

	it("maps personal managed access to Personal", () => {
		expect(getAccountModeLabel(baseState, activeContext)).toBe("Personal");
	});

	it("falls back to BYOK when not authenticated", () => {
		const signedOutContext: LicenseAuthContext = {
			...activeContext,
			authenticated: false,
		};

		expect(getAccountModeLabel(baseState, signedOutContext)).toBe("BYOK");
	});

	it("treats token-invalid and reauth-required as reauthentication reasons", () => {
		expect(isReauthRequiredReason("reauth_required")).toBe(true);
		expect(isReauthRequiredReason("token_invalid")).toBe(true);
		expect(isReauthRequiredReason("membership_missing")).toBe(false);
	});

	it("does not show reauthentication copy when the device is signed out", () => {
		expect(isReauthRequiredForSession(false, "token_invalid")).toBe(false);
		expect(
			getAccountModeDescription({
				modeLabel: "BYOK",
				signedIn: false,
				reauthRequired: true,
			}),
		).toContain("Community/BYOK");
		expect(
			getAccountModeDescription({
				modeLabel: "BYOK",
				signedIn: false,
				reauthRequired: true,
			}),
		).toContain("settings sync");
	});

	it("describes signed-in BYOK users as a valid community state", () => {
		expect(
			getAccountModeDescription({
				modeLabel: "BYOK",
				signedIn: true,
				reauthRequired: false,
			}),
		).toContain("You're signed in");
		expect(
			getAccountModeDescription({
				modeLabel: "BYOK",
				signedIn: true,
				reauthRequired: false,
			}),
		).toContain("Community/BYOK");
		expect(
			getAccountModeDescription({
				modeLabel: "BYOK",
				signedIn: true,
				reauthRequired: false,
			}),
		).toContain("settings sync");
	});

	it("returns a warning description when reauthentication is needed", () => {
		expect(
			getAccountModeDescription({
				modeLabel: "Managed Business",
				signedIn: true,
				reauthRequired: true,
			}),
		).toContain("Re-authenticate");
	});

	it("uses caution color when reauthentication is required", () => {
		expect(
			getAccountStatusColor({ status: "active", reauthRequired: true }),
		).toBe("yellow");
	});

	it("caps usage percent at 100", () => {
		expect(calculateUsagePercent(150, 100)).toBe(100);
	});
});
