import assert from "node:assert/strict";
import { test } from "node:test";
import { desktopSentryConfig } from "./configure-desktop-sentry.mjs";

const devDsn = "https://public@o1.ingest.us.sentry.io/4510879887523840";
const prodDsn = "https://public@o1.ingest.us.sentry.io/4511413280243712";
const env = { GITHUB_SHA: "abcdef0123456789", SENTRY_PUBLIC_DEV_DSN: devDsn, SENTRY_PUBLIC_PROD_DSN: prodDsn, SENTRY_AUTH_TOKEN: "test-only-not-a-real-token" };

test("release runtime DSN, uploader project and release stay aligned for every channel", () => {
	for (const [ref, environment, dsn, project, release] of [
		["refs/tags/v1.2.3", "production", prodDsn, "kolboo-public-prod", "kolboo@1.2.3"],
		["refs/tags/v1.2.3-beta.2", "beta", devDsn, "kolboo-public-dev", "kolboo@1.2.3-beta.2"],
		["refs/heads/master", "development", devDsn, "kolboo-public-dev", "kolboo@1.2.3-dev.abcdef0"],
	]) {
		const result = desktopSentryConfig({ ...env, GITHUB_REF: ref }, "1.2.3");
		assert.equal(result.env.TAURI_SENTRY_DSN, dsn);
		assert.equal(result.env.SENTRY_PROJECT, project);
		assert.equal(result.env.TAURI_SENTRY_ENV, environment);
		assert.equal(result.env.VITE_SENTRY_ENV, environment);
		assert.equal(result.env.SENTRY_RELEASE, release);
		assert.equal(result.env.TAURI_SENTRY_RELEASE, release);
		assert.equal(result.env.VITE_SENTRY_RELEASE, release);
		assert.equal(result.outputs.reporting_enabled, "true");
	}
});

test("unconfigured development stays cloud-free; development-only packages never pretend to be releases", () => {
	assert.equal(desktopSentryConfig({ GITHUB_SHA: env.GITHUB_SHA }, "1.2.3").env.TAURI_SENTRY_DSN, "");
	const result = desktopSentryConfig({ ...env, GITHUB_REF: "refs/tags/v1.2.3", SENTRY_BUILD_KIND: "development" }, "1.2.3");
	assert.equal(result.env.TAURI_SENTRY_ENV, "development");
	assert.equal(result.env.TAURI_SENTRY_DSN, devDsn);
	assert.equal(desktopSentryConfig({ GITHUB_SHA: env.GITHUB_SHA, TAURI_SENTRY_DSN: devDsn }, "1.2.3").env.TAURI_SENTRY_DSN, devDsn);
});

test("published releases cannot silently lose reporting or point at a different project", () => {
	const tagEnv = { ...env, GITHUB_REF: "refs/tags/v1.2.3" };
	for (const token of [undefined, "", "replace_me"]) assert.throws(() => desktopSentryConfig({ ...tagEnv, SENTRY_AUTH_TOKEN: token }, "1.2.3"), /Public releases require/);
	assert.throws(() => desktopSentryConfig({ ...tagEnv, SENTRY_PUBLIC_PROD_DSN: "" }, "1.2.3"), /Public releases require/);
	for (const dsn of [devDsn, "https://public@example.test/4511413280243712", prodDsn.replace("https:", "http:"), prodDsn.replace("public@", "public:secret@"), `${prodDsn}?key=secret`, `${prodDsn}#extra`]) {
		assert.throws(() => desktopSentryConfig({ ...tagEnv, SENTRY_PUBLIC_PROD_DSN: dsn }, "1.2.3"), /refusing mismatched/);
	}
	assert.throws(() => desktopSentryConfig({ ...tagEnv, SENTRY_PUBLIC_PROD_DSN: "not a url" }, "1.2.3"), /Invalid desktop Sentry DSN/);
	assert.throws(() => desktopSentryConfig({ ...env, GITHUB_REF: "refs/tags/v1.2.3\nINJECT=yes" }, "1.2.3"), /Invalid desktop release version/);
	assert.throws(() => desktopSentryConfig({ GITHUB_SHA: "bad" }, "1.2.3"), /commit SHA/);
});
