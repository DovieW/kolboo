/// <reference types="vite/client" />
/// <reference types="vite-plugin-svgr/client" />

interface ImportMetaEnv {
	readonly VITE_SENTRY_DSN?: string;
	readonly VITE_SENTRY_ENV?: string;
	readonly VITE_APP_VERSION?: string;
	readonly VITE_POSTHOG_API_KEY?: string;
	readonly VITE_POSTHOG_HOST?: string;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
