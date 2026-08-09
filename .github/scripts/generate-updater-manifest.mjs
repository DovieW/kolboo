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
	const signaturePath = files.find((file) => file.endsWith("-setup.exe.sig"))
		?? files.find((file) => file.endsWith(".msi.sig"));
	if (!signaturePath) {
		throw new Error("No signed Windows updater artifact was found; refusing to publish latest.json.");
	}

	const artifactPath = signaturePath.slice(0, -4);
	const artifactName = path.basename(artifactPath);
	const signature = (await readFile(signaturePath, "utf8")).trim();
	if (!signature) throw new Error("The updater signature is empty.");

	const version = tag.replace(/^v/, "");
	if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
		throw new Error(`Release tag ${tag} is not a supported semantic version.`);
	}

	const manifest = {
		version,
		notes: await readFile(notesPath, "utf8"),
		pub_date: publishedAt,
		platforms: {
			"windows-x86_64": {
				signature,
				url: `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(artifactName)}`,
			},
		},
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
