import { sentryCliBinaryExists, sentryVitePlugin } from "@sentry/vite-plugin";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { loadEnv } from "vite";
import svgr from "vite-plugin-svgr";
import { defineConfig } from "vitest/config";

function trimEnvValue(value: string | undefined): string {
	return value?.trim() ?? "";
}

function isConfiguredEnvValue(value: string | undefined): boolean {
	const trimmed = trimEnvValue(value);
	return Boolean(trimmed) && trimmed.toLowerCase() !== "replace_me";
}

function isProductionLikeEnv(value: string | undefined): boolean {
	const normalized = trimEnvValue(value).toLowerCase();
	return normalized === "prod" || normalized === "production";
}

type DesktopSentryBuildConfig = {
	authToken: string;
	org: string;
	project: string;
	release: string;
};

function resolveDesktopSentryProject(env: Record<string, string>): string {
	const explicitProject = trimEnvValue(env.SENTRY_PROJECT);
	if (explicitProject) {
		return explicitProject;
	}

	// Keep the public/private split at the project level. Tags then handle the
	// per-surface segmentation (`main`, `overlay`, etc.) inside that family.
	return isProductionLikeEnv(env.TAURI_SENTRY_ENV)
		? "kolboo-public-prod"
		: "kolboo-public-dev";
}

function resolveDesktopSentryRelease(env: Record<string, string>): string {
	const explicitRelease =
		trimEnvValue(env.TAURI_SENTRY_RELEASE) || trimEnvValue(env.SENTRY_RELEASE);
	if (explicitRelease) {
		return explicitRelease;
	}

	const version =
		trimEnvValue(env.TAURI_APP_VERSION) ||
		trimEnvValue(env.npm_package_version) ||
		"0.0.0";
	const sha = trimEnvValue(env.GITHUB_SHA).slice(0, 7);

	if (sha && !isProductionLikeEnv(env.TAURI_SENTRY_ENV)) {
		return `kolboo@${version}-dev.${sha}`;
	}

	return `kolboo@${version}`;
}

function resolveDesktopSentryBuildConfig(
	env: Record<string, string>,
): DesktopSentryBuildConfig | null {
	const authToken = trimEnvValue(env.SENTRY_AUTH_TOKEN);
	if (!isConfiguredEnvValue(authToken)) {
		return null;
	}

	if (!sentryCliBinaryExists()) {
		throw new Error(
			"Sentry source-map upload was enabled, but @sentry/vite-plugin cannot find its CLI binary. Reinstall dependencies with build scripts enabled or approve @sentry/cli for pnpm builds.",
		);
	}

	return {
		authToken,
		org: trimEnvValue(env.SENTRY_ORG) || "dov-weinstock",
		project: resolveDesktopSentryProject(env),
		release: resolveDesktopSentryRelease(env),
	};
}

export default defineConfig(({ mode }) => {
	const env = {
		...process.env,
		...loadEnv(mode, process.cwd(), ""),
	} as Record<string, string>;
	const host = trimEnvValue(env.TAURI_DEV_HOST);
	const desktopSentryBuild = resolveDesktopSentryBuildConfig(env);
	const plugins = [react(), tailwindcss(), svgr()];

	if (desktopSentryBuild) {
		plugins.push(
			...sentryVitePlugin({
				authToken: desktopSentryBuild.authToken,
				org: desktopSentryBuild.org,
				project: desktopSentryBuild.project,
				release: {
					name: desktopSentryBuild.release,
					setCommits: {
						auto: true,
						ignoreEmpty: true,
						ignoreMissing: true,
					},
				},
				sourcemaps: {
					assets: "./dist/**/*",
					filesToDeleteAfterUpload: ["./dist/**/*.map"],
				},
				telemetry: false,
			}),
		);
	}

	return {
		plugins,
		clearScreen: false,
		test: {
			coverage: {
				provider: "v8",
				reporter: ["text", "html"],
				reportsDirectory: "coverage",
				include: ["src/**/*.{ts,tsx}"],
				exclude: [
					"src/**/*.d.ts",
					"src/vite-env.d.ts",
					"src/main.tsx",
					"src/overlay-main.tsx",
					"src/overlay-hover-main.tsx",
					"src/quick-ask-main.tsx",
				],
				thresholds: {
					"src/lib/tauri.ts": {
						statements: 80,
						branches: 70,
						functions: 80,
						lines: 80,
					},
					"src/lib/tauri/commands.ts": {
						statements: 30,
						branches: 40,
						functions: 18,
						lines: 30,
					},
					"src/lib/tauri/settings.ts": {
						statements: 44,
						branches: 57,
						functions: 38,
						lines: 43,
					},
					"src/lib/tauri/events.ts": {
						statements: 70,
						branches: 60,
						functions: 70,
						lines: 70,
					},
				},
			},
		},
		server: {
			host: host || false,
			port: 5173,
			strictPort: true,
			hmr: host
				? {
						protocol: "ws",
						host,
						port: 5173,
					}
				: undefined,
			watch: {
				ignored: ["**/src-tauri/**"],
			},
		},
		envPrefix: ["VITE_", "TAURI_"],
		build: {
			target:
				env.TAURI_PLATFORM === "windows"
					? "chrome105"
					: env.TAURI_PLATFORM === "macos"
						? "safari13"
						: "chrome105",
			minify: !env.TAURI_DEBUG ? "esbuild" : false,
			// Keep debug builds readable, but switch release-like builds to hidden
			// source maps when Sentry upload is configured so stack traces stay
			// useful without shipping public source-map references.
			sourcemap: desktopSentryBuild ? "hidden" : Boolean(env.TAURI_DEBUG),
			rollupOptions: {
				input: {
					main: "index.html",
					overlay: "overlay.html",
					overlayHover: "overlay-hover.html",
					quickAsk: "quick-ask.html",
				},
			},
		},
	};
});
