import { describe, expect, it } from "vitest";
import { resolveManagedInferenceMode } from "./settings";

describe("resolveManagedInferenceMode", () => {
	it("returns managed for active personal tier", () => {
		expect(
			resolveManagedInferenceMode({
				license_state: { tier: "personal", status: "active" },
			}),
		).toBe("managed");
	});

	it("returns managed for grace personal tier", () => {
		expect(
			resolveManagedInferenceMode({
				license_state: { tier: "personal", status: "grace" },
			}),
		).toBe("managed");
	});

	it("returns byok for signed out personal tier", () => {
		expect(
			resolveManagedInferenceMode({
				license_state: { tier: "personal", status: "signed_out" },
			}),
		).toBe("byok");
	});

	it("returns managed for enterprise with valid eligible cloud policy", () => {
		expect(
			resolveManagedInferenceMode({
				license_state: { tier: "enterprise", status: "active" },
				policy_state: { source: "cloud", eligible: true, is_valid: true },
			}),
		).toBe("managed");
	});

	it("returns byok for community by default", () => {
		expect(
			resolveManagedInferenceMode({
				license_state: { tier: "community", status: "signed_out" },
			}),
		).toBe("byok");
	});
});
