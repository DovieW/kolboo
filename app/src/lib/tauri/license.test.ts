import { describe, expect, it } from "vitest";
import {
	buildLicenseSentryContext,
	getLicenseErrorMessage,
	getLicenseTransitionFromSettingsPayload,
} from "./license";

describe("license helpers", () => {
	it("normalizes unknown errors", () => {
		expect(getLicenseErrorMessage(new Error("Boom"))).toBe("Boom");
		expect(getLicenseErrorMessage("Nope")).toBe("Nope");
		expect(getLicenseErrorMessage({})).toContain("Something went wrong");
	});

	it("extracts valid transition payloads", () => {
		expect(
			getLicenseTransitionFromSettingsPayload({
				license_state_changed: true,
				license_transition: {
					from: "active",
					to: "grace",
					occurred_at: "2026-01-01T00:00:00Z",
					reason: "refresh_failed",
				},
			}),
		).toEqual({
			from: "active",
			to: "grace",
			occurred_at: "2026-01-01T00:00:00Z",
			reason: "refresh_failed",
		});
	});

	it("ignores malformed transition payloads", () => {
		expect(
			getLicenseTransitionFromSettingsPayload({
				license_transition: {
					from: "maybe",
					to: "active",
					occurred_at: "2026-01-01T00:00:00Z",
					reason: "bad",
				},
			}),
		).toBeNull();
		expect(getLicenseTransitionFromSettingsPayload(null)).toBeNull();
		expect(getLicenseTransitionFromSettingsPayload(undefined)).toBeNull();
	});

	it("redacts sensitive license telemetry context", () => {
		expect(
			buildLicenseSentryContext({
				action: "refresh",
				access_token: "secret-token",
				provider_hint: "personal",
				nested: {
					refreshToken: "secret-refresh",
					ok: true,
				},
			}),
		).toEqual({
			action: "refresh",
			access_token: "[REDACTED]",
			provider_hint: "personal",
			nested: {
				refreshToken: "[REDACTED]",
				ok: true,
			},
		});
	});
});
