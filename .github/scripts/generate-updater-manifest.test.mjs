import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { createHash } from "node:crypto";
import { generateManifest } from "./generate-updater-manifest.mjs";

const crossPlatformFiles = {
	"a.AppImage": "appimage", "a.AppImage.sig": "appimage-sig",
	"a.deb": "deb", "a.deb.sig": "deb-sig",
	"Kolboo-universal.app.tar.gz": "mac", "Kolboo-universal.app.tar.gz.sig": "mac-sig",
};

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

test("binds latest.json to each signed platform and installer artifact", async () => {
	const root = await mkdtemp(path.join(os.tmpdir(), "kolboo-updater-test-"));
	const bundle = path.join(root, "bundle", "nsis");
	await mkdir(bundle, { recursive: true });
	await writeFile(path.join(root, "notes.md"), "Release notes", "utf8");
	await writeFile(path.join(bundle, "Kolboo_1.2.3_x64-setup.exe"), "installer", "utf8");
	await writeFile(path.join(bundle, "Kolboo_1.2.3_x64-setup.exe.sig"), "trusted-signature", "utf8");
	await writeFile(path.join(bundle, "Kolboo_1.2.3_x64_en-US.msi"), "msi installer", "utf8");
	await writeFile(path.join(bundle, "Kolboo_1.2.3_x64_en-US.msi.sig"), "trusted-msi-signature", "utf8");
	for (const [file, content] of Object.entries(crossPlatformFiles)) await writeFile(path.join(bundle, file), content);

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
	assert.equal(manifest.platforms["linux-x86_64-deb"].signature, "deb-sig");
	assert.match(manifest.platforms["linux-x86_64-deb"].url, /a.deb$/);
	assert.equal(manifest.platforms["linux-x86_64-appimage"].signature, "appimage-sig");
	assert.deepEqual(manifest.platforms["linux-x86_64"], manifest.platforms["linux-x86_64-appimage"]);
	assert.equal(manifest.platforms["darwin-aarch64"].signature, "mac-sig");
	assert.match(manifest.platforms["darwin-aarch64"].url, /Kolboo-universal.app.tar.gz$/);
	assert.deepEqual(manifest.platforms["darwin-aarch64"], manifest.platforms["darwin-x86_64"]);
	assert.equal(Object.keys(manifest.platforms).length, 8);
});

test("committed updater configuration uses the committed public key", async () => {
	const repositoryRoot = path.resolve(import.meta.dirname, "../..");
	const config = JSON.parse(await readFile(path.join(repositoryRoot, "app/src-tauri/tauri.conf.json"), "utf8"));
	const publicKey = (await readFile(path.join(repositoryRoot, "app/src-tauri/updater.pubkey"), "utf8")).trim();
	assert.equal(config.plugins.updater.pubkey, publicKey);
});

test("stable releases sign and publish all updater formats without enabling unsigned development channels", async () => {
	const repositoryRoot = path.resolve(import.meta.dirname, "../..");
	const load = (file) => readFile(path.join(repositoryRoot, file), "utf8");
	const linux = await load(".github/workflows/linux-build.yml");
	const mac = await load(".github/workflows/macos-build.yml");
	const release = await load(".github/workflows/release.yml");
	const beta = await load(".github/workflows/linux-beta-release.yml");
	assert.match(linux, /release_signing:\s+type: boolean\s+default: false/);
	assert.match(linux, /VITE_SIGNED_UPDATER_ENABLED: \$\{\{ inputs.release_signing && 'true' \|\| 'false' \}\}/);
	assert.match(linux, /sign_args=\(--no-sign\)/);
	assert.match(linux, /test -n "\$TAURI_SIGNING_PRIVATE_KEY"/);
	assert.match(linux, /test -n "\$TAURI_SIGNING_PRIVATE_KEY_PASSWORD"/);
	assert.match(linux, /tauri signer sign "\$GITHUB_WORKSPACE\/\$deb_path"/);
	assert.match(linux, /cp "\$deb_path.sig" "\$appimage_path.sig" artifacts\//);
	assert.match(linux, /channel=\$\{\{ inputs.release_signing && 'versioned-community' \|\| 'linux-community-beta' \}\}/);
	assert.match(release, /uses: \.\/\.github\/workflows\/linux-build.yml\s+with:\s+release_signing: true/);
	assert.doesNotMatch(beta, /release_signing: true/);
	assert.match(mac, /VITE_SIGNED_UPDATER_ENABLED: \$\{\{ inputs.release_build && 'true' \|\| 'false' \}\}/);
	assert.match(mac, /test -n "\$TAURI_SIGNING_PRIVATE_KEY"/);
	assert.match(mac, /test -n "\$TAURI_SIGNING_PRIVATE_KEY_PASSWORD"/);
	assert.match(mac, /test -s "\$archive.sig"/);
	assert.match(mac, /cp "\$archive.sig" "artifacts\/Kolboo-\$\{KOLBOO_ARTIFACT_SUFFIX\}.app.tar.gz.sig"/);
	const config = JSON.parse(await load("app/src-tauri/tauri.macos-release.conf.json"));
	assert.equal(config.bundle.createUpdaterArtifacts, true);
	assert.equal(config.bundle.macOS.signingIdentity, "-");
	assert.equal(JSON.parse(await load("app/src-tauri/tauri.macos-development.conf.json")).bundle.createUpdaterArtifacts, false);
	assert.match(release, /architecture: universal\s+release_build: true/);
	assert.match(release, /generate-updater-manifest.mjs \.release \.release\/RELEASE_NOTES.md/);
	assert.match(release, /\.release\/macos\/\*\.app.tar.gz\*/);
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
	["missing AppImage", { "a.AppImage.sig": null }, "v1.2.3", /signed linux-x86_64-appimage/],
	["missing deb", { "a.deb.sig": null }, "v1.2.3", /signed linux-x86_64-deb/],
	["missing universal Mac", { "Kolboo-universal.app.tar.gz.sig": null }, "v1.2.3", /signed darwin-aarch64/],
	["duplicate deb", { "b.deb.sig": "sig" }, "v1.2.3", /signed linux-x86_64-deb/],
	["empty Mac signature", { "Kolboo-universal.app.tar.gz.sig": "" }, "v1.2.3", /signature is empty/],
	["dangling AppImage", { "a.AppImage": null }, "v1.2.3", /ENOENT/],
]) {
	test(`refuses an unsafe manifest: ${name}`, async () => {
		const root = await mkdtemp(path.join(os.tmpdir(), "kolboo-updater-invalid-"));
		const valid = { "a-setup.exe": "exe", "a-setup.exe.sig": "sig", "a.msi": "msi", "a.msi.sig": "sig", ...crossPlatformFiles };
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
