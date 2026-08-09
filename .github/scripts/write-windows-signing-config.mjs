import { writeFile } from "node:fs/promises";

const [outputPath, rawThumbprint] = process.argv.slice(2);
const thumbprint = rawThumbprint?.replaceAll(/\s/g, "").toUpperCase();

if (!outputPath || !thumbprint || !/^[0-9A-F]{40,64}$/.test(thumbprint)) {
	throw new Error("A valid Windows code-signing certificate thumbprint is required.");
}

const config = {
	bundle: {
		windows: {
			certificateThumbprint: thumbprint,
			digestAlgorithm: "sha256",
			timestampUrl: "http://timestamp.digicert.com",
		},
	},
};

await writeFile(outputPath, `${JSON.stringify(config, null, 2)}\n`, "utf8");
