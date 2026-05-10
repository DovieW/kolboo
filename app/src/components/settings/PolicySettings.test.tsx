import { describe, expect, it } from "vitest";
import {
	diagnosticsToJson,
	formatPolicySourceLabel,
	formatPolicyTimestampLabel,
	policyStatusSummary,
} from "./policyDiagnostics";

describe("PolicySettings helpers", () => {
	it("renders policy source labels", () => {
		expect(formatPolicySourceLabel("none")).toBe("Unmanaged");
		expect(formatPolicySourceLabel("file")).toBe("Local file");
		expect(formatPolicySourceLabel("cloud")).toBe("Cloud");
	});

	it("formats timestamps safely", () => {
		expect(formatPolicyTimestampLabel(null)).toBe("—");
		expect(formatPolicyTimestampLabel("not-a-date")).toBe("—");
		expect(formatPolicyTimestampLabel("2026-02-13T12:00:00Z")).toContain(
			"2026",
		);
	});

	it("summarizes status from policy state", () => {
		expect(
			policyStatusSummary({
				source: "none",
				is_valid: true,
				last_updated: null,
				expires_at: null,
				version: null,
				enforced_fields: [],
			}),
		).toBe("No active policy");

		expect(
			policyStatusSummary({
				source: "cloud",
				is_valid: false,
				last_updated: null,
				expires_at: null,
				version: null,
				enforced_fields: [],
			}),
		).toBe("Policy invalid");
	});

	it("serializes diagnostics JSON", () => {
		const json = diagnosticsToJson({ redaction_applied: true });
		expect(json).toContain('"redaction_applied": true');
	});
});
