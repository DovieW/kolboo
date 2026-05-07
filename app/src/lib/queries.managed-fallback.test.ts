import { describe, expect, it } from "vitest";
import { toManagedInferenceMessage } from "./queries/shared";

describe("toManagedInferenceMessage", () => {
	it("returns BYOK recovery guidance for temporary outages", () => {
		expect(
			toManagedInferenceMessage({ category: "temporarily_unavailable" }),
		).toContain("switch to BYOK providers");
	});

	it("returns quota guidance for over_quota", () => {
		expect(toManagedInferenceMessage({ category: "over_quota" })).toContain(
			"managed usage limit",
		);
	});

	it("prefers explicit auth reason codes when present", () => {
		expect(
			toManagedInferenceMessage({
				category: "unauthorized",
				reason_code: "policy_denied",
			}),
		).toContain("organization policy");

		expect(
			toManagedInferenceMessage({
				category: "ineligible",
				reason_code: "insufficient_tier",
			}),
		).toContain("BYOK providers");
	});

	it("falls back to temporary-unavailable guidance for unknown errors", () => {
		expect(toManagedInferenceMessage(new Error("boom"))).toContain(
			"temporarily unavailable",
		);
		expect(toManagedInferenceMessage(null)).toContain(
			"switch to BYOK providers",
		);
	});
});
