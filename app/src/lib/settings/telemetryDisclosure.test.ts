import { describe, expect, it, vi } from "vitest";
import {
	buildTelemetryDisclosureResolutionPatch,
	isTelemetryDisclosureResolved,
	POSTHOG_ANALYTICS_ENABLED_KEY,
	shouldSendProductAnalytics,
	TELEMETRY_DISCLOSURE_ACKNOWLEDGED_AT_KEY,
	TELEMETRY_DISCLOSURE_VERSION,
	TELEMETRY_DISCLOSURE_VERSION_KEY,
} from "./telemetryDisclosure";

describe("telemetry disclosure helpers", () => {
	it("treats only the current disclosure version as resolved", () => {
		expect(
			isTelemetryDisclosureResolved({
				telemetryDisclosureAcknowledgedAt: "2026-05-13T10:00:00.000Z",
				telemetryDisclosureVersion: TELEMETRY_DISCLOSURE_VERSION,
			}),
		).toBe(true);

		expect(
			isTelemetryDisclosureResolved({
				telemetryDisclosureAcknowledgedAt: "2026-05-13T10:00:00.000Z",
				telemetryDisclosureVersion: "older-copy",
			}),
		).toBe(false);
		expect(
			isTelemetryDisclosureResolved({
				telemetryDisclosureAcknowledgedAt: null,
				telemetryDisclosureVersion: TELEMETRY_DISCLOSURE_VERSION,
			}),
		).toBe(false);
	});

	it("requires both opt-in posture and resolved disclosure before analytics may send", () => {
		expect(
			shouldSendProductAnalytics({
				posthogAnalyticsEnabled: true,
				telemetryDisclosureAcknowledgedAt: "2026-05-13T10:00:00.000Z",
				telemetryDisclosureVersion: TELEMETRY_DISCLOSURE_VERSION,
			}),
		).toBe(true);

		expect(
			shouldSendProductAnalytics({
				posthogAnalyticsEnabled: false,
				telemetryDisclosureAcknowledgedAt: "2026-05-13T10:00:00.000Z",
				telemetryDisclosureVersion: TELEMETRY_DISCLOSURE_VERSION,
			}),
		).toBe(false);
		expect(
			shouldSendProductAnalytics({
				posthogAnalyticsEnabled: true,
				telemetryDisclosureAcknowledgedAt: null,
				telemetryDisclosureVersion: null,
			}),
		).toBe(false);
	});

	it("builds a resolved settings patch with the locked contract keys", () => {
		vi.useFakeTimers();
		vi.setSystemTime(new Date("2026-05-13T12:34:56.000Z"));

		expect(
			buildTelemetryDisclosureResolutionPatch({ analyticsEnabled: false }),
		).toEqual({
			[POSTHOG_ANALYTICS_ENABLED_KEY]: false,
			[TELEMETRY_DISCLOSURE_ACKNOWLEDGED_AT_KEY]: "2026-05-13T12:34:56.000Z",
			[TELEMETRY_DISCLOSURE_VERSION_KEY]: TELEMETRY_DISCLOSURE_VERSION,
		});

		vi.useRealTimers();
	});
});
