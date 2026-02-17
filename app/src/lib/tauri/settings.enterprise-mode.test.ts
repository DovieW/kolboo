import { describe, expect, it } from "vitest";
import { resolveManagedInferenceMode } from "./settings";

describe("resolveManagedInferenceMode enterprise normalization", () => {
	it("returns managed for enterprise with valid eligible cloud policy", () => {
		expect(
			resolveManagedInferenceMode({
				license_state: { tier: "enterprise", status: "active" },
				policy_state: { source: "cloud", is_valid: true, eligible: true },
			}),
		).toBe("managed");
	});

	it("returns byok for enterprise when policy source is none", () => {
		expect(
			resolveManagedInferenceMode({
				license_state: { tier: "enterprise", status: "active" },
				policy_state: { source: "none", is_valid: true, eligible: true },
			}),
		).toBe("byok");
	});

	it("returns byok for enterprise when policy is invalid", () => {
		expect(
			resolveManagedInferenceMode({
				license_state: { tier: "enterprise", status: "active" },
				policy_state: { source: "cloud", is_valid: false, eligible: true },
			}),
		).toBe("byok");
	});

	it("returns byok for enterprise when policy is ineligible", () => {
		expect(
			resolveManagedInferenceMode({
				license_state: { tier: "enterprise", status: "active" },
				policy_state: { source: "cloud", is_valid: true, eligible: false },
			}),
		).toBe("byok");
	});
});
