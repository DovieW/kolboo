import assert from "node:assert/strict";
import fs from "node:fs";
import { test } from "node:test";
import { validateDesktopCloudConfig } from "./validate-desktop-cloud-config.mjs";

const publicKeys = {
  TAURI_API_BASE_URL: "https://api.example.com",
  TAURI_MANAGED_INFERENCE_GATEWAY_URL: "https://api.example.com",
  TAURI_SUPABASE_URL: "https://example.supabase.co",
  TAURI_SUPABASE_PUBLISHABLE_KEY: "sb_publishable_synthetic_release_key",
  TAURI_PUBLIC_AUTH_PAGE_URL: "https://account.example.com/login",
};

for (const platform of ["windows", "macos", "linux"]) {
  test(`${platform} embeds only public config when explicitly enabled`, () => {
    const source = fs.readFileSync(new URL(`../workflows/${platform}-build.yml`, import.meta.url), "utf8");
    assert.equal((source.match(/service_enabled:\n\s+description: [^\n]+\n\s+type: boolean\n\s+default: false/g) ?? []).length, 2);
    for (const enabled of [false, true]) {
      const env = {};
      for (const [key, fallback] of [["TAURI_CLOUD_ENV", "community"], ["VITE_CLOUD_SERVICE_ENABLED", "false"]]) {
        const match = source.match(new RegExp(`^      ${key}: \\$\\{\\{ inputs.service_enabled && '([^']+)' \\|\\| '${fallback}' \\}\\}$`, "m"));
        assert.ok(match, `${platform}: ${key} must follow the explicit input`);
        env[key] = enabled ? match[1] : fallback;
      }
      for (const [key, value] of Object.entries(publicKeys)) {
        const variable = key.replace(/^TAURI_/, "KOLBOO_PROD_");
        assert.ok(source.includes(`${key}: \${{ inputs.service_enabled && vars.${variable} || '' }}`));
        env[key] = enabled ? value : "";
      }
      validateDesktopCloudConfig(env);
      if (enabled) for (const key of Object.keys(publicKeys)) {
        assert.throws(() => validateDesktopCloudConfig({ ...env, [key]: "" }));
      }
    }
  });
}

test("versioned release enables every platform; Linux-only Community beta does not", () => {
  const release = fs.readFileSync(new URL("../workflows/release.yml", import.meta.url), "utf8");
  for (const platform of ["windows", "macos", "linux"]) {
    assert.match(release, new RegExp(`uses: \\./\\.github/workflows/${platform}-build.yml\\n    with:\\n      service_enabled: true`));
  }
  assert.match(release, /name: Verify service release configuration/);
  const beta = fs.readFileSync(new URL("../workflows/linux-beta-release.yml", import.meta.url), "utf8");
  assert.ok(!beta.includes("service_enabled: true"));
  assert.match(release, /draft: true/);
});
