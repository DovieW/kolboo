import * as Sentry from "@sentry/react";

export type SentrySurface = "main" | "overlay" | "overlay_hover" | "quick_ask";

const SENSITIVE_KEY_PATTERN =
	/(?:api[-_]?key|access[-_]?token|refresh[-_]?token|token|secret|password|passwd|authorization|bearer|cookie|set-cookie)/i;

let initialized = false;

export function isSentryConfigured(): boolean {
	return Boolean(import.meta.env.VITE_SENTRY_DSN?.trim());
}

function toPrimitiveString(value: unknown): string {
	if (typeof value === "string") return value;
	if (
		typeof value === "number" ||
		typeof value === "boolean" ||
		typeof value === "bigint"
	) {
		return String(value);
	}
	return "unknown";
}

export function redactTelemetryValue(value: unknown): unknown {
	if (value == null) return value;

	if (Array.isArray(value)) {
		return value.map((entry) => redactTelemetryValue(entry));
	}

	if (typeof value === "object") {
		const out: Record<string, unknown> = {};
		for (const [key, entry] of Object.entries(
			value as Record<string, unknown>,
		)) {
			if (SENSITIVE_KEY_PATTERN.test(key)) {
				out[key] = "[REDACTED]";
				continue;
			}
			out[key] = redactTelemetryValue(entry);
		}
		return out;
	}

	if (typeof value === "string" && value.length > 512) {
		return `${value.slice(0, 512)}…`;
	}

	return value;
}

export function initSentry(surface: SentrySurface): boolean {
	const dsn = import.meta.env.VITE_SENTRY_DSN?.trim();
	if (!dsn || initialized) return false;

	Sentry.init({
		dsn,
		enabled: true,
		environment:
			import.meta.env.VITE_SENTRY_ENV?.trim() || import.meta.env.MODE,
		release: import.meta.env.VITE_APP_VERSION?.trim() || undefined,
		sampleRate: 1,
		tracesSampleRate: 0,
		beforeSend(event) {
			const safe = { ...event };
			delete safe.user;
			delete safe.request;
			if (safe.extra) {
				safe.extra = redactTelemetryValue(safe.extra) as Record<
					string,
					unknown
				>;
			}
			if (safe.contexts) {
				safe.contexts = redactTelemetryValue(
					safe.contexts,
				) as typeof safe.contexts;
			}
			return safe;
		},
		beforeBreadcrumb(breadcrumb) {
			if (
				breadcrumb.category?.toLowerCase().includes("xhr") ||
				breadcrumb.category?.toLowerCase().includes("fetch")
			) {
				return null;
			}
			return breadcrumb;
		},
	});

	Sentry.setTag("surface", surface);
	initialized = true;
	return true;
}

export function captureSentryException(
	error: unknown,
	params: {
		surface: SentrySurface;
		action: string;
		extra?: Record<string, unknown>;
	},
) {
	if (!isSentryConfigured()) return;

	Sentry.withScope((scope) => {
		scope.setTag("surface", params.surface);
		scope.setTag("action", params.action);
		if (params.extra) {
			scope.setContext(
				"license",
				redactTelemetryValue(params.extra) as Record<string, unknown>,
			);
		}

		if (error instanceof Error) {
			Sentry.captureException(error);
			return;
		}

		Sentry.captureException(new Error(toPrimitiveString(error)));
	});
}
