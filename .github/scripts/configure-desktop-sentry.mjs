import { appendFileSync, readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const PROJECT_IDS = {
	"kolboo-public-dev": "4510879887523840",
	"kolboo-public-prod": "4511413280243712",
};

export function desktopSentryConfig(env, version) {
	const tag = env.SENTRY_BUILD_KIND !== "development" && env.GITHUB_REF?.startsWith("refs/tags/v");
	const releaseVersion = tag ? env.GITHUB_REF.slice("refs/tags/v".length) : version;
	if (!/^\d+\.\d+\.\d+(?:-[\w.-]+)?$/.test(releaseVersion)) throw new Error("Invalid desktop release version");
	const environment = tag ? (releaseVersion.includes("-") ? "beta" : "production") : "development";
	const project = environment === "production" ? "kolboo-public-prod" : "kolboo-public-dev";
	const dsn = ((environment === "production" ? env.SENTRY_PUBLIC_PROD_DSN : env.SENTRY_PUBLIC_DEV_DSN || env.TAURI_SENTRY_DSN) || "").trim();
	if (dsn) {
		let parsed;
		try { parsed = new URL(dsn); } catch { throw new Error("Invalid desktop Sentry DSN"); }
		if (/[\r\n]/.test(dsn) || parsed.protocol !== "https:" || !parsed.username || parsed.password || parsed.search || parsed.hash || !/\.ingest\.(?:[a-z]+\.)?sentry\.io$/.test(parsed.hostname) || parsed.pathname !== `/${PROJECT_IDS[project]}`) {
			throw new Error(`Desktop DSN must belong to ${project}; refusing mismatched report/source-map routing`);
		}
	}
	const uploadConfigured = Boolean(env.SENTRY_AUTH_TOKEN?.trim()) && env.SENTRY_AUTH_TOKEN.trim() !== "replace_me";
	if (tag && (!dsn || !uploadConfigured)) throw new Error("Public releases require both the matching Sentry DSN and SENTRY_AUTH_TOKEN for source maps");
	const sha = env.GITHUB_SHA?.slice(0, 7);
	if (!tag && !/^[a-f\d]{7}$/i.test(sha ?? "")) throw new Error("Development reporting requires the build commit SHA");
	const release = `kolboo@${releaseVersion}${tag ? "" : `-dev.${sha}`}`;
	return {
		env: {
			SENTRY_ORG: "dov-weinstock", SENTRY_PROJECT: project, SENTRY_RELEASE: release,
			TAURI_APP_VERSION: version, TAURI_SENTRY_DSN: dsn, TAURI_SENTRY_ENV: environment,
			TAURI_SENTRY_RELEASE: release, VITE_SENTRY_ENV: environment, VITE_SENTRY_RELEASE: release,
		},
		outputs: { version, release, project, environment, reporting_enabled: String(Boolean(dsn)), sourcemaps_required: String(Boolean(tag)) },
	};
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	const { version } = JSON.parse(readFileSync(new URL("../../app/package.json", import.meta.url), "utf8"));
	const config = desktopSentryConfig(process.env, version);
	for (const [file, entries] of [[process.env.GITHUB_ENV, config.env], [process.env.GITHUB_OUTPUT, config.outputs]]) {
		if (!file) throw new Error("This release configuration command requires GitHub Actions output files");
		appendFileSync(file, Object.entries(entries).map(([key, value]) => `${key}=${value}\n`).join(""));
	}
	console.log(`Sentry: ${config.outputs.project}, ${config.outputs.environment}, ${config.outputs.release}, runtime enabled=${config.outputs.reporting_enabled}`);
}
