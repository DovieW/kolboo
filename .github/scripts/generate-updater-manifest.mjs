import { readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";

async function walk(directory) {
	const entries = await readdir(directory, { withFileTypes: true });
	const files = [];
	for (const entry of entries) {
		const candidate = path.join(directory, entry.name);
		if (entry.isDirectory()) files.push(...(await walk(candidate)));
		else files.push(candidate);
	}
	return files;
}

export async function generateManifest({ bundlesDir, notesPath, outputPath, tag, repository, publishedAt }) {
	const files = await walk(bundlesDir);
	const nsisSignatures = files.filter((file) => file.endsWith("-setup.exe.sig"));
	const msiSignatures = files.filter((file) => file.endsWith(".msi.sig"));
	if (nsisSignatures.length !== 1 || msiSignatures.length !== 1) {
		throw new Error("Exactly one signed NSIS and one signed MSI artifact are required; refusing to publish latest.json.");
	}
	const platforms = {};
	const artifacts = [["windows-x86_64-nsis", nsisSignatures[0]], ["windows-x86_64-msi", msiSignatures[0]]];
	for (const [target, suffix] of [["linux-x86_64-appimage", ".AppImage.sig"], ["linux-x86_64-deb", ".deb.sig"], ["darwin-aarch64", "Kolboo-universal.app.tar.gz.sig"]]) {
		const signatures = files.filter((file) => file.endsWith(suffix));
		if (signatures.length !== 1) throw new Error(`Exactly one signed ${target} artifact is required.`);
		artifacts.push([target, signatures[0]]);
	}
	for (const [target, signaturePath] of artifacts) {
		const artifactPath = signaturePath.slice(0, -4);
		await readFile(artifactPath); // Refuse a dangling signature, not just a missing signature.
		const signature = (await readFile(signaturePath, "utf8")).trim();
		if (!signature) throw new Error("The updater signature is empty.");
		platforms[target] = {
			signature,
			url: `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(path.basename(artifactPath))}`,
		};
	}
	// Raw executable/legacy clients have no embedded installer type. Packaged
	// MSI clients must get MSI, not a migration to the NSIS uninstall wizard.
	platforms["windows-x86_64"] = platforms["windows-x86_64-nsis"];
	platforms["linux-x86_64"] = platforms["linux-x86_64-appimage"];
	platforms["darwin-x86_64"] = platforms["darwin-aarch64"];

	const version = tag.replace(/^v/, "");
	if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
		throw new Error(`Release tag ${tag} is not a supported semantic version.`);
	}

	const manifest = {
		version,
		notes: await readFile(notesPath, "utf8"),
		pub_date: publishedAt,
		platforms,
	};

	await writeFile(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
}

if (import.meta.url === `file://${process.argv[1]}`) {
	const [bundlesDir, notesPath, outputPath] = process.argv.slice(2);
	if (!bundlesDir || !notesPath || !outputPath || !process.env.GITHUB_REF_NAME || !process.env.GITHUB_REPOSITORY) {
		throw new Error("Usage: generate-updater-manifest.mjs <bundles-dir> <notes> <output> in GitHub Actions.");
	}
	await generateManifest({
		bundlesDir,
		notesPath,
		outputPath,
		tag: process.env.GITHUB_REF_NAME,
		repository: process.env.GITHUB_REPOSITORY,
		publishedAt: new Date().toISOString(),
	});
}
