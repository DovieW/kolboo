import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { generateManifest } from "./generate-updater-manifest.mjs";

test("refuses to publish unsigned updater artifacts", async () => {
	const root = await mkdtemp(path.join(os.tmpdir(), "kolboo-updater-test-"));
	await writeFile(path.join(root, "notes.md"), "Notes", "utf8");
	await assert.rejects(
		generateManifest({
			bundlesDir: root,
			notesPath: path.join(root, "notes.md"),
			outputPath: path.join(root, "latest.json"),
			tag: "v1.2.3",
			repository: "DovieW/kolboo",
			publishedAt: "2026-08-09T00:00:00.000Z",
		}),
		/No signed Windows updater artifact/,
	);
});

test("binds latest.json to the signed Windows artifact", async () => {
	const root = await mkdtemp(path.join(os.tmpdir(), "kolboo-updater-test-"));
	const bundle = path.join(root, "bundle", "nsis");
	await mkdir(bundle, { recursive: true });
	await writeFile(path.join(root, "notes.md"), "Release notes", "utf8");
	await writeFile(path.join(bundle, "Kolboo_1.2.3_x64-setup.exe"), "installer", "utf8");
	await writeFile(path.join(bundle, "Kolboo_1.2.3_x64-setup.exe.sig"), "trusted-signature", "utf8");

	const outputPath = path.join(root, "latest.json");
	await generateManifest({
		bundlesDir: path.join(root, "bundle"),
		notesPath: path.join(root, "notes.md"),
		outputPath,
		tag: "v1.2.3",
		repository: "DovieW/kolboo",
		publishedAt: "2026-08-09T00:00:00.000Z",
	});

	const manifest = JSON.parse(await readFile(outputPath, "utf8"));
	assert.equal(manifest.version, "1.2.3");
	assert.equal(manifest.platforms["windows-x86_64"].signature, "trusted-signature");
	assert.match(manifest.platforms["windows-x86_64"].url, /Kolboo_1.2.3_x64-setup.exe$/);
});

test("committed updater configuration uses the committed public key", async () => {
	const repositoryRoot = path.resolve(import.meta.dirname, "../..");
	const config = JSON.parse(await readFile(path.join(repositoryRoot, "app/src-tauri/tauri.conf.json"), "utf8"));
	const publicKey = (await readFile(path.join(repositoryRoot, "app/src-tauri/updater.pubkey"), "utf8")).trim();
	assert.equal(config.plugins.updater.pubkey, publicKey);
});
