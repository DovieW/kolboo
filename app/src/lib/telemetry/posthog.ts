import { Store } from "@tauri-apps/plugin-store";
import { invoke } from "@tauri-apps/api/core";
import { STT_MODELS, BUNDLED_MANAGED_MODELS } from "../modelOptions";
import {
	POSTHOG_ANALYTICS_ENABLED_KEY,
	shouldSendAggregateAnalytics,
	shouldSendProductAnalytics,
	TELEMETRY_DISCLOSURE_ACKNOWLEDGED_AT_KEY,
	TELEMETRY_DISCLOSURE_VERSION_KEY,
} from "../settings/telemetryDisclosure";
import { loadRuntimeConfig } from "../tauri/runtimeConfig";

const DISTINCT_ID_STORAGE_KEY = "kolboo_posthog_distinct_id_v1";
const POSTHOG_HOSTS = new Set([
	"https://us.i.posthog.com",
	"https://eu.i.posthog.com",
]);

// Basic events have an explicit, narrow property projection. Never forward
// native payloads, account data, user-entered model IDs, or arbitrary URLs.
export const AGGREGATE_EVENT_NAMES = [
	"desktop_opened",
	"page_viewed",
	"recording_started",
	"recording_finished",
	"pipeline_transcription_started",
	"pipeline_transcript_ready",
	"pipeline_error",
] as const;
export type AggregateEventName = (typeof AGGREGATE_EVENT_NAMES)[number];

export const ANALYTICS_PAGES = [
	"home",
	"transcribe-file",
	"settings",
	"logs",
	"usage-stats",
	"account",
] as const;
export type AnalyticsPage = (typeof ANALYTICS_PAGES)[number];

const KNOWN_STT_PROVIDERS = new Set(Object.keys(STT_MODELS));
const KNOWN_STT_MODELS = new Set([
	...Object.values(STT_MODELS).flatMap((models) =>
		models.map((model) => model.value),
	),
	...BUNDLED_MANAGED_MODELS.filter((model) =>
		model.capabilities.includes("transcription"),
	).map((model) => model.id),
]);

function basicEventProperties(
	event: AggregateEventName,
	provided: Record<string, unknown>,
): Record<string, string | number> {
	if (event === "page_viewed") {
		return { page: provided.page as AnalyticsPage };
	}
	if (event !== "recording_finished") return {};
	const properties: Record<string, string | number> = {
		status: provided.status as "success" | "error",
	};
	if (provided.mode === "dictation" || provided.mode === "meeting") {
		properties.mode = provided.mode;
	} else {
		properties.mode = "unknown";
	}
	if (
		typeof provided.duration_seconds === "number" &&
		Number.isFinite(provided.duration_seconds) &&
		provided.duration_seconds >= 0 &&
		provided.duration_seconds <= 14_400
	) {
		properties.duration_seconds = Math.round(provided.duration_seconds);
	}
	properties.stt_provider =
		typeof provided.stt_provider === "string" &&
		KNOWN_STT_PROVIDERS.has(provided.stt_provider)
			? provided.stt_provider
			: "other";
	properties.stt_model =
		typeof provided.stt_model === "string" &&
		KNOWN_STT_MODELS.has(provided.stt_model)
			? provided.stt_model
			: "other";
	return properties;
}

const OPTIONAL_EVENT_NAMES = new Set([
	"cloud_sync_action_succeeded",
	"cloud_sync_action_failed",
	"cloud_sync_enabled_changed",
	"cloud_sync_auto_push_changed",
	"analytics_opted_in",
]);

const SAFE_ERROR_KINDS = new Set([
	"Error",
	"TypeError",
	"NetworkError",
	"AbortError",
	"TimeoutError",
	"object",
	"string",
	"undefined",
]);

function trimOrEmpty(value: string | undefined): string {
	return (value ?? "").trim();
}

function normalizePosthogHost(host: string): string | null {
	const normalized = host.replace(/\/+$/, "");
	return POSTHOG_HOSTS.has(normalized) ? normalized : null;
}

function newEventId(): string {
	return typeof globalThis.crypto?.randomUUID === "function"
		? globalThis.crypto.randomUUID()
		: `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function getDistinctId(): string {
	try {
		const existing = globalThis.localStorage?.getItem(DISTINCT_ID_STORAGE_KEY);
		if (existing && existing.trim().length > 0) {
			return existing.trim();
		}

		const next = `kolboo-${newEventId()}`;
		globalThis.localStorage?.setItem(DISTINCT_ID_STORAGE_KEY, next);
		return next;
	} catch {
		return `kolboo-ephemeral-${newEventId()}`;
	}
}

function optionalEventProperties(
	event: string,
	provided: Record<string, unknown>,
): Record<string, string | boolean> {
	const safe: Record<string, string | boolean> = {};
	if (
		event === "cloud_sync_action_succeeded" ||
		event === "cloud_sync_action_failed"
	) {
		if (provided.action === "push" || provided.action === "pull") {
			safe.action = provided.action;
		}
		if (event === "cloud_sync_action_failed") {
			safe.error_kind = SAFE_ERROR_KINDS.has(provided.error_kind as string)
				? (provided.error_kind as string)
				: "other";
		}
	}
	if (
		(event === "cloud_sync_enabled_changed" ||
			event === "cloud_sync_auto_push_changed") &&
		typeof provided.enabled === "boolean"
	) {
		safe.enabled = provided.enabled;
	}
	if (event === "analytics_opted_in" && provided.surface === "settings") {
		safe.surface = "settings";
	}
	return safe;
}

export async function isPosthogConfigured(): Promise<boolean> {
	try {
		const config = await loadRuntimeConfig();
		const apiKey = trimOrEmpty(config.posthog_api_key ?? undefined);
		const host = normalizePosthogHost(
			trimOrEmpty(config.posthog_host ?? undefined),
		);
		return apiKey.length > 0 && host !== null;
	} catch {
		return false;
	}
}

async function canSendAnalytics(
	kind: "aggregate" | "optional",
): Promise<boolean> {
	try {
		// Re-read policy and disclosure on every event. The user control applies
		// only to the optional, per-installation stream; an organization may still
		// disable all product analytics through policy.
		const store = await Store.load("settings.json");
		const disclosure = {
			telemetryDisclosureAcknowledgedAt:
				(await store.get<string | null>(
					TELEMETRY_DISCLOSURE_ACKNOWLEDGED_AT_KEY,
				)) ?? null,
			telemetryDisclosureVersion:
				(await store.get<string | null>(TELEMETRY_DISCLOSURE_VERSION_KEY)) ??
				null,
		};
		const policy = await store.get<Record<string, unknown>>(
			"policy_effective_values",
		);
		const organizationDisablesAnalytics =
			policy?.disable_product_analytics === true;
		if (kind === "aggregate") {
			return shouldSendAggregateAnalytics(
				disclosure,
				organizationDisablesAnalytics,
			);
		}
		return (
			!organizationDisablesAnalytics &&
			shouldSendProductAnalytics({
				...disclosure,
				posthogAnalyticsEnabled:
					(await store.get<boolean>(POSTHOG_ANALYTICS_ENABLED_KEY)) ?? true,
			})
		);
	} catch {
		// Privacy wins if we can't read the disclosure state confidently.
		return false;
	}
}

async function optionalIdentity(): Promise<string> {
	try {
		const context = await invoke<{
			authenticated: boolean;
			subject_id: string | null;
		}>("license_get_auth_context");
		if (
			context.authenticated &&
			context.subject_id &&
			/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
				context.subject_id,
			)
		) {
			return `kolboo-account:${context.subject_id.toLowerCase()}`;
		}
	} catch {
		// BYOK/offline use must not depend on the account service.
	}
	return getDistinctId();
}

async function capture(
	event: string,
	properties: Record<string, unknown>,
	kind: "aggregate" | "optional",
): Promise<void> {
	if (!(await canSendAnalytics(kind))) return;
	if (typeof fetch !== "function") return;

	// Runtime configuration can be unavailable while the native bridge starts.
	const config = await loadRuntimeConfig().catch(() => null);
	if (!config) return;
	const apiKey = trimOrEmpty(config.posthog_api_key ?? undefined);
	const host = normalizePosthogHost(
		trimOrEmpty(config.posthog_host ?? undefined),
	);
	if (!apiKey || !host) return;

	const safeProperties =
		kind === "optional"
			? optionalEventProperties(event, properties)
			: basicEventProperties(event as AggregateEventName, properties);
	const linked = kind === "optional" || (await canSendAnalytics("optional"));

	const payload = {
		api_key: apiKey,
		event,
		// PostHog requires a distinct ID for every event. Without opt-in, use
		// a fresh ID even for basic events; never retroactively alias it.
		distinct_id: linked ? await optionalIdentity() : newEventId(),
		properties: {
			...safeProperties,
			// Shared PostHog project: this must be filterable to avoid counting
			// local development as production usage.
			environment:
				import.meta.env.DEV || config.sentry_env === "development"
					? "development"
					: /-[0-9a-z.-]+$/i.test(config.app_version ?? "")
						? "beta"
						: "production",
			// Reserved fields come last so optional caller properties cannot
			// override identity or force person-profile / geo-IP processing.
			$process_person_profile: false,
			$geoip_disable: true,
			$lib: "kolboo-desktop",
			$lib_version: trimOrEmpty(config.app_version ?? undefined) || "dev",
		},
	};

	try {
		const response = await fetch(`${host}/i/v0/e/`, {
			method: "POST",
			headers: {
				"content-type": "application/json",
			},
			body: JSON.stringify(payload),
		});
		if (!response.ok) {
			console.warn(`PostHog capture rejected (HTTP ${response.status})`);
		}
	} catch {
		// Never block product flow on analytics transport.
		console.warn("PostHog capture request failed");
	}
}

export async function trackAggregateEvent(
	event: AggregateEventName,
	properties: Record<string, unknown> = {},
): Promise<void> {
	if (!(AGGREGATE_EVENT_NAMES as readonly string[]).includes(event)) return;
	if (
		event === "page_viewed" &&
		!(ANALYTICS_PAGES as readonly unknown[]).includes(properties.page)
	)
		return;
	if (
		event === "recording_finished" &&
		properties.status !== "success" &&
		properties.status !== "error"
	)
		return;
	await capture(event, properties, "aggregate");
}

export async function trackProductEvent(
	event: string,
	properties: Record<string, unknown> = {},
): Promise<void> {
	if (!OPTIONAL_EVENT_NAMES.has(event)) return;
	await capture(event, properties, "optional");
}
