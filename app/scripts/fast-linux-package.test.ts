// @vitest-environment node
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

const service = {
	TAURI_CLOUD_ENV: "production",
	VITE_CLOUD_SERVICE_ENABLED: "true",
	TAURI_API_BASE_URL: "https://api.example.com",
	TAURI_MANAGED_INFERENCE_GATEWAY_URL: "https://api.example.com",
	TAURI_SUPABASE_URL: "https://auth.example.com",
	TAURI_SUPABASE_PUBLISHABLE_KEY: "sb_publishable_synthetic_public_config",
	TAURI_PUBLIC_AUTH_PAGE_URL: "https://account.example.com/login",
};

it.skipIf(process.platform !== "linux")(
	"refuses incomplete or unsafe cloud packages before building or copying anything",
	() => {
		for (const [overrides, message] of [
			[{ TAURI_CLOUD_ENV: "" }, "Explicit matching"],
			[{ TAURI_SUPABASE_URL: "" }, "TAURI_SUPABASE_URL"],
			[{ TAURI_SUPABASE_PUBLISHABLE_KEY: "" }, "publishable/anon"],
			[
				{ TAURI_API_BASE_URL: "https://kolboo.dovie.dev" },
				"TAURI_API_BASE_URL",
			],
			[{ VITE_CLOUD_SERVICE_ENABLED: "false" }, "Explicit matching"],
			[
				{
					TAURI_CLOUDFLARE_ACCESS_CLIENT_SECRET:
						"synthetic-secret-do-not-print",
				},
				"Forbidden release setting",
			],
		] as const) {
			const result = spawnSync(
				process.execPath,
				[fileURLToPath(new URL("./fast-linux-package.mjs", import.meta.url))],
				{
					env: {
						...process.env,
						TAURI_CLOUDFLARE_ACCESS_CLIENT_ID: "",
						TAURI_CLOUDFLARE_ACCESS_CLIENT_SECRET: "",
						...service,
						...overrides,
					},
					encoding: "utf8",
				},
			);
			expect(result.status).toBe(1);
			expect(result.stderr).toContain(message);
			expect(result.stderr).not.toContain("synthetic-secret-do-not-print");
			expect(result.stdout).toBe("");
		}
	},
);
