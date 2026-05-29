import { describe, expect, it } from "vitest";
import {
	normalizeLicenseState,
	normalizePolicyState,
	normalizeTokenExchangeTriggerSet,
} from "./policy";

describe("policy and license settings normalizer", () => {
	it("marks expired policy snapshots invalid and filters malformed enforced fields", () => {
		const normalized = normalizePolicyState({
			source: "cloud",
			is_valid: true,
			expires_at: "2000-01-01T00:00:00Z",
			version: 3,
			enforced_fields: [
				{ path: " request_logs_privacy_mode ", reason: "Org policy" },
				{ path: "   " },
				null,
			],
		});

		expect(normalized.is_valid).toBe(false);
		expect(normalized.version).toBe("3");
		expect(normalized.enforced_fields).toEqual([
			{ path: "request_logs_privacy_mode", reason: "Org policy" },
		]);
		expect(normalized.enforced_count).toBe(1);
	});

	it("normalizes license org, usage, and limits without requiring a full record", () => {
		const normalized = normalizeLicenseState({
			tier: "enterprise",
			status: "active",
			org: {
				org_id: " org-1 ",
				org_name: " Kolboo ",
				inference_mode: "managed",
			},
			usage: {
				stt_seconds_used: 10.9,
				llm_tokens_used: -5,
			},
			limits: {
				requests_per_day: 200.8,
			},
		});

		expect(normalized.org).toEqual({
			org_id: "org-1",
			org_name: "Kolboo",
			inference_mode: "managed",
		});
		expect(normalized.usage).toEqual({
			stt_seconds_used: 10,
			llm_tokens_used: 0,
			requests_today: 0,
		});
		expect(normalized.limits).toEqual({
			stt_seconds_monthly: 0,
			llm_tokens_monthly: 0,
			requests_per_day: 200,
		});
		expect(normalized.portal_available).toBe(false);
	});

	it("normalizes token-exchange trigger booleans and reviewed timestamps", () => {
		const normalized = normalizeTokenExchangeTriggerSet({
			multi_idp_required: 1,
			kill_switch_required: true,
			reviewed_at: "2026-02-03T04:05:06Z",
		});

		expect(normalized.multi_idp_required).toBe(true);
		expect(normalized.kill_switch_required).toBe(true);
		expect(normalized.embedded_claims_required).toBe(false);
		expect(normalized.desktop_idp_agnostic_required).toBe(false);
		expect(normalized.reviewed_at).toBe("2026-02-03T04:05:06.000Z");
	});
});
