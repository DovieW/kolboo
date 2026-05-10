import { describe, expect, it } from "vitest";
import {
	formatPolicySourceLabel,
	formatPolicyTimestampLabel,
	policyStatusColor,
	policyStatusSummary,
} from "./policyDiagnostics";

describe("PolicyDiagnosticsCard helpers", () => {
	it("formats policy source labels including degraded modes", () => {
		expect(formatPolicySourceLabel("none")).toBe("Unmanaged");
		expect(formatPolicySourceLabel("file")).toBe("Local file");
		expect(formatPolicySourceLabel("cloud")).toBe("Cloud");
		expect(formatPolicySourceLabel("cached")).toBe("Cached");
		expect(formatPolicySourceLabel("degraded_expired")).toBe("Degraded");
	});

	it("formats timestamps safely", () => {
		expect(formatPolicyTimestampLabel(null)).toBe("—");
		expect(formatPolicyTimestampLabel("not-a-date")).toBe("—");
		expect(formatPolicyTimestampLabel("2026-02-13T12:00:00Z")).toContain(
			"2026",
		);
	});

	it("summarizes degraded/cached policy states", () => {
		expect(
			policyStatusSummary({
				source: "cached",
				is_valid: true,
				last_updated: null,
				expires_at: null,
				version: "1",
				enforced_fields: [],
			}),
		).toBe("Using cached policy");

		expect(
			policyStatusSummary({
				source: "degraded_expired",
				is_valid: false,
				last_updated: null,
				expires_at: null,
				version: "1",
				enforced_fields: [],
			}),
		).toBe("Policy degraded (expired)");
	});

	it("returns warning/error badge colors for cached and degraded states", () => {
		expect(
			policyStatusColor({
				source: "cached",
				is_valid: true,
				last_updated: null,
				expires_at: null,
				version: "1",
				enforced_fields: [],
			}),
		).toBe("yellow");

		expect(
			policyStatusColor({
				source: "degraded_expired",
				is_valid: false,
				last_updated: null,
				expires_at: null,
				version: "1",
				enforced_fields: [],
			}),
		).toBe("red");
	});
});
