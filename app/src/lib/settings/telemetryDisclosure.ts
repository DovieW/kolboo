export const POSTHOG_ANALYTICS_ENABLED_KEY = "posthog_analytics_enabled";
export const TELEMETRY_DISCLOSURE_ACKNOWLEDGED_AT_KEY =
	"telemetry_disclosure_acknowledged_at";
export const TELEMETRY_DISCLOSURE_VERSION_KEY = "telemetry_disclosure_version";

// Keep this explicit and stable. If we revise the wording later, bump this
// version intentionally so the app can re-show the disclosure on purpose.
export const TELEMETRY_DISCLOSURE_VERSION = "2026-05-phase6b-v1";

export type TelemetryDisclosureState = {
	posthogAnalyticsEnabled: boolean;
	telemetryDisclosureAcknowledgedAt: string | null;
	telemetryDisclosureVersion: string | null;
};

function trimOrNull(value: string | null | undefined): string | null {
	const trimmed = (value ?? "").trim();
	return trimmed.length > 0 ? trimmed : null;
}

export function isTelemetryDisclosureResolved(
	state: Pick<
		TelemetryDisclosureState,
		"telemetryDisclosureAcknowledgedAt" | "telemetryDisclosureVersion"
	>,
): boolean {
	return (
		trimOrNull(state.telemetryDisclosureAcknowledgedAt) !== null &&
		trimOrNull(state.telemetryDisclosureVersion) ===
			TELEMETRY_DISCLOSURE_VERSION
	);
}

export function shouldSendProductAnalytics(
	state: TelemetryDisclosureState,
): boolean {
	return state.posthogAnalyticsEnabled && isTelemetryDisclosureResolved(state);
}

export function buildTelemetryDisclosureResolutionPatch(args: {
	analyticsEnabled: boolean;
	acknowledgedAt?: string;
}): Record<string, unknown> {
	const acknowledgedAt =
		trimOrNull(args.acknowledgedAt) ?? new Date().toISOString();

	return {
		[POSTHOG_ANALYTICS_ENABLED_KEY]: Boolean(args.analyticsEnabled),
		[TELEMETRY_DISCLOSURE_ACKNOWLEDGED_AT_KEY]: acknowledgedAt,
		[TELEMETRY_DISCLOSURE_VERSION_KEY]: TELEMETRY_DISCLOSURE_VERSION,
	};
}
