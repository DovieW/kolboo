import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { createHash } from "node:crypto";
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
		/Exactly one signed NSIS and one signed MSI artifact/,
	);
});

test("binds latest.json to the signed Windows artifact", async () => {
	const root = await mkdtemp(path.join(os.tmpdir(), "kolboo-updater-test-"));
	const bundle = path.join(root, "bundle", "nsis");
	await mkdir(bundle, { recursive: true });
	await writeFile(path.join(root, "notes.md"), "Release notes", "utf8");
	await writeFile(path.join(bundle, "Kolboo_1.2.3_x64-setup.exe"), "installer", "utf8");
	await writeFile(path.join(bundle, "Kolboo_1.2.3_x64-setup.exe.sig"), "trusted-signature", "utf8");
	await writeFile(path.join(bundle, "Kolboo_1.2.3_x64_en-US.msi"), "msi installer", "utf8");
	await writeFile(path.join(bundle, "Kolboo_1.2.3_x64_en-US.msi.sig"), "trusted-msi-signature", "utf8");

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
	assert.deepEqual(manifest.platforms["windows-x86_64-nsis"], manifest.platforms["windows-x86_64"]);
	assert.equal(manifest.platforms["windows-x86_64-msi"].signature, "trusted-msi-signature");
	assert.match(manifest.platforms["windows-x86_64-msi"].url, /Kolboo_1.2.3_x64_en-US.msi$/);
});

test("committed updater configuration uses the committed public key", async () => {
	const repositoryRoot = path.resolve(import.meta.dirname, "../..");
	const config = JSON.parse(await readFile(path.join(repositoryRoot, "app/src-tauri/tauri.conf.json"), "utf8"));
	const publicKey = (await readFile(path.join(repositoryRoot, "app/src-tauri/updater.pubkey"), "utf8")).trim();
	assert.equal(config.plugins.updater.pubkey, publicKey);
});

test("manual NSIS upgrades use a narrowly modified pinned upstream template", async () => {
	const repositoryRoot = path.resolve(import.meta.dirname, "../..");
	const config = JSON.parse(await readFile(path.join(repositoryRoot, "app/src-tauri/tauri.conf.json"), "utf8"));
	const packageJson = JSON.parse(await readFile(path.join(repositoryRoot, "app/package.json"), "utf8"));
	assert.equal(packageJson.devDependencies["@tauri-apps/cli"], "2.11.4", "rebase the template when upgrading the CLI");
	assert.equal(config.bundle.windows.nsis.template, "windows/installer.nsi");
	assert.equal(config.bundle.windows.nsis.installMode, "currentUser");
	assert.equal(config.plugins.updater.windows.installMode, "passive");
	const template = await readFile(path.join(repositoryRoot, "app/src-tauri/windows/installer.nsi"), "utf8");
	const change = "  ; KOLBOO: ordinary NSIS upgrades use the same in-place path as /UPDATE.\n" +
		"  ; Keep first installs, same-version maintenance, downgrades and MSI migration\n" +
		"  ; on their upstream paths. Never invoke the uninstaller for an NSIS upgrade.\n" +
		"  ${If} $R0 = 1\n  ${AndIf} $WixMode = 0\n    StrCpy $UpdateMode 1\n    Abort\n  ${EndIf}\n";
	assert.equal(template.split(change).length, 2);
	const skipPages = "; KOLBOO: in-place updates keep the restored install location and shortcuts.\n" +
		"Function SkipIfUpdating\n  ${IfThen} $UpdateMode = 1 ${|} Abort ${|}\n  Call SkipIfPassive\nFunctionEnd\n";
	assert.equal(template.split(skipPages).length, 2);
	assert.equal(template.split("MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfUpdating").length, 3);
	const upstream = template.split("\n").slice(4).join("\n").replace(change, "").replace(skipPages, "")
		.replaceAll("MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfUpdating", "MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive");
	assert.equal(createHash("sha256").update(upstream).digest("hex"),
		"20f4ecc730defb71f1342eaeaec4021df13be3d843abba0effe88ea5835fa079",
		"do not silently fork the installer or remove data-preservation safeguards");
});

for (const [name, files, tag, expected] of [
	["missing MSI", { "a-setup.exe": "exe", "a-setup.exe.sig": "sig" }, "v1.2.3", /Exactly one signed/],
	["duplicate NSIS", { "b-setup.exe.sig": "sig" }, "v1.2.3", /Exactly one signed/],
	["empty signature", { "a.msi.sig": " \n" }, "v1.2.3", /signature is empty/],
	["dangling signature", { "a.msi": null }, "v1.2.3", /ENOENT/],
	["invalid version", {}, "invalid", /semantic version/],
]) {
	test(`refuses an unsafe manifest: ${name}`, async () => {
		const root = await mkdtemp(path.join(os.tmpdir(), "kolboo-updater-invalid-"));
		const valid = { "a-setup.exe": "exe", "a-setup.exe.sig": "sig", "a.msi": "msi", "a.msi.sig": "sig" };
		const entries = name === "missing MSI" ? files : { ...valid, ...files };
		for (const [file, content] of Object.entries(entries)) {
			if (content !== null) await writeFile(path.join(root, file), content);
		}
		await writeFile(path.join(root, "notes.md"), "Notes");
		await assert.rejects(generateManifest({ bundlesDir: root, notesPath: path.join(root, "notes.md"),
			outputPath: path.join(root, "latest.json"), tag, repository: "DovieW/kolboo", publishedAt: "2026-10-07T00:00:00Z" }), expected);
		await assert.rejects(readFile(path.join(root, "latest.json")), /ENOENT/);
	});
}
