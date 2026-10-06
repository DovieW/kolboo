import assert from "node:assert/strict";
import { test } from "node:test";
import { validateDesktopCloudConfig } from "./validate-desktop-cloud-config.mjs";

const valid = {
	TAURI_CLOUD_ENV: "production",
	VITE_CLOUD_SERVICE_ENABLED: "true",
	TAURI_API_BASE_URL: "https://api.example.com",
	TAURI_MANAGED_INFERENCE_GATEWAY_URL: "https://api.example.com",
	TAURI_SUPABASE_URL: "https://prod.supabase.co",
	TAURI_PUBLIC_AUTH_PAGE_URL: "https://account.example.com/login",
	TAURI_SUPABASE_PUBLISHABLE_KEY: "sb_publishable_synthetic_public_config",
};

test("accepts complete public production config and legacy anon keys", () => {
	validateDesktopCloudConfig(valid);
	validateDesktopCloudConfig({ ...valid, TAURI_SUPABASE_PUBLISHABLE_KEY: `a.${Buffer.from('{"role":"anon"}').toString("base64url")}.b` });
});
test("Community packages contain no cloud credentials or origins and UI/build modes agree", () => {
	const community = { TAURI_CLOUD_ENV: "community", VITE_CLOUD_SERVICE_ENABLED: "false" };
	validateDesktopCloudConfig(community);
	for (const key of Object.keys(valid).filter(key => key.startsWith("TAURI_") && key !== "TAURI_CLOUD_ENV")) {
		assert.throws(() => validateDesktopCloudConfig({ ...community, [key]: valid[key] }), { message: `Community package must omit public cloud configuration: ${key}` });
		validateDesktopCloudConfig({ ...community, [key]: " " });
	}
	for (const env of [{ ...community, VITE_CLOUD_SERVICE_ENABLED: "true" }, { ...valid, VITE_CLOUD_SERVICE_ENABLED: "false" }, { ...valid, VITE_CLOUD_SERVICE_ENABLED: undefined }]) {
		assert.throws(() => validateDesktopCloudConfig(env));
	}
	assert.throws(() => validateDesktopCloudConfig({ ...community, TAURI_CLOUDFLARE_ACCESS_CLIENT_SECRET: "synthetic" }));
});
test("fails early for missing/development origins without exposing values", () => {
	assert.throws(() => validateDesktopCloudConfig({ ...valid, TAURI_CLOUD_ENV: "development" }));
	for (const key of ["TAURI_API_BASE_URL", "TAURI_MANAGED_INFERENCE_GATEWAY_URL", "TAURI_SUPABASE_URL", "TAURI_PUBLIC_AUTH_PAGE_URL"]) {
		for (const value of [undefined, "", "invalid", "http://api.example.com", "https://localhost", "https://kolboo.dovie.dev", "https://dev.example.com", "https://dev-api.example.com", "https://api.test", "https://user:secret@api.example.com", "https://api.example.com?secret=x", "https://api.example.com#x", "https://api.example.com\nBAD=1"]) {
			assert.throws(() => validateDesktopCloudConfig({ ...valid, [key]: value }), { message: `Invalid production public configuration: ${key}` });
		}
	}
});
test("rejects secret and malformed keys and Access tokens", () => {
	for (const key of [undefined, "", "sb_publishable_short", "sb_secret_DO_NOT_PRINT", "a.e30.b", "a.bm90LWpzb24.b", "a.eyJyb2xlIjoic2VydmljZV9yb2xlIn0.b", "sb_publishable_synthetic_public_config\nSECRET=1"]) {
		assert.throws(() => validateDesktopCloudConfig({ ...valid, TAURI_SUPABASE_PUBLISHABLE_KEY: key }), { message: "Only a Supabase publishable/anon key may enter desktop packages" });
	}
	for (const key of ["TAURI_CLOUDFLARE_ACCESS_CLIENT_ID", "TAURI_CLOUDFLARE_ACCESS_CLIENT_SECRET"]) {
		assert.throws(() => validateDesktopCloudConfig({ ...valid, [key]: "DO_NOT_PRINT" }), { message: `Forbidden release setting: ${key}` });
	}
});
