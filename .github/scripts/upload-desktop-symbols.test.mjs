import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "node:test";
import { cliAsset, requireDebugInfo, uploadDesktopSymbols, verifyCli } from "./upload-desktop-symbols.mjs";

test("supported runners use version- and checksum-pinned official CLIs", () => {
	for (const [platform, arch] of [["linux", "x64"], ["darwin", "arm64"], ["darwin", "x64"], ["win32", "x64"]]) {
		const asset = cliAsset(platform, arch);
		assert.match(asset.url, /^https:\/\/github.com\/getsentry\/cli\/releases\/download\/0\.45\.0\//);
		assert.match(asset.sha256, /^[a-f0-9]{64}$/);
	}
	assert.throws(() => cliAsset("unknown", "x64"), /Unsupported/);
	const bytes = Buffer.from("test executable bytes");
	verifyCli(bytes, createHash("sha256").update(bytes).digest("hex"));
	assert.throws(() => verifyCli(bytes, "wrong"), /checksum mismatch/);
});

test("missing symbols cannot masquerade as a successful upload", () => {
	requireDebugInfo([{ usable: true, objects: [{ hasDebugInfo: true }] }]);
	for (const report of [{}, { usable: false, objects: [{ hasDebugInfo: true }] }, { usable: true, objects: [{ hasDebugInfo: false }] }]) {
		assert.throws(() => requireDebugInfo([report]), /no line\/debug information/);
	}
});

test("unconfigured development skips uploads without network, while configured uploads require exact existing files", async () => {
	await uploadDesktopSymbols([], {});
	await uploadDesktopSymbols([], { SENTRY_AUTH_TOKEN: "replace_me" });
	await assert.rejects(uploadDesktopSymbols([], { SENTRY_AUTH_TOKEN: "test-only" }), /exact desktop/);
	await assert.rejects(uploadDesktopSymbols([import.meta.dirname], { SENTRY_AUTH_TOKEN: "test-only" }), /must be a file/);
});
