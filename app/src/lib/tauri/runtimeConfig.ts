import { invoke } from "@tauri-apps/api/core";

export interface RuntimeConfig {
	app_version: string | null;
	api_base_url: string | null;
	managed_inference_gateway_url: string | null;
	sentry_dsn: string | null;
	sentry_env: string | null;
	sentry_release: string | null;
	posthog_api_key: string | null;
	posthog_host: string | null;
}

const DEFAULT_RUNTIME_CONFIG: RuntimeConfig = {
	app_version: null,
	api_base_url: null,
	managed_inference_gateway_url: null,
	sentry_dsn: null,
	sentry_env: null,
	sentry_release: null,
	posthog_api_key: null,
	posthog_host: null,
};

let runtimeConfigPromise: Promise<RuntimeConfig> | null = null;

function normalizeOptionalString(value: unknown): string | null {
	if (typeof value !== "string") return null;
	const trimmed = value.trim();
	return trimmed.length > 0 ? trimmed : null;
}

function normalizeConfig(
	raw: Partial<RuntimeConfig> | null | undefined,
): RuntimeConfig {
	return {
		app_version: normalizeOptionalString(raw?.app_version),
		api_base_url: normalizeOptionalString(raw?.api_base_url),
		managed_inference_gateway_url: normalizeOptionalString(
			raw?.managed_inference_gateway_url,
		),
		sentry_dsn: normalizeOptionalString(raw?.sentry_dsn),
		sentry_env: normalizeOptionalString(raw?.sentry_env),
		sentry_release: normalizeOptionalString(raw?.sentry_release),
		posthog_api_key: normalizeOptionalString(raw?.posthog_api_key),
		posthog_host: normalizeOptionalString(raw?.posthog_host),
	};
}

export function loadRuntimeConfig(): Promise<RuntimeConfig> {
	if (!runtimeConfigPromise) {
		runtimeConfigPromise = invoke<RuntimeConfig>("get_runtime_config")
			.then((config) => normalizeConfig(config))
			.catch(() => DEFAULT_RUNTIME_CONFIG);
	}

	return runtimeConfigPromise;
}
