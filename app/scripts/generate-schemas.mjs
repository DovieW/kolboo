import { spawnSync } from "node:child_process";
import { mkdirSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptDir, "..");
const binDir = path.join(appRoot, "src-tauri", "src", "bin");
const outDir = path.join(appRoot, "src-tauri", "gen", "schemas");
const manifestPath = path.join(appRoot, "src-tauri", "Cargo.toml");

mkdirSync(outDir, { recursive: true });

const binFiles = readdirSync(binDir)
	.filter((name) => name.startsWith("export_") && name.endsWith("_schema.rs"))
	.sort();

if (binFiles.length === 0) {
	throw new Error(`No schema export bins found under ${binDir}`);
}

const generated = [];

for (const file of binFiles) {
	const binName = file.replace(/\.rs$/, "");
	const baseName = binName
		.replace(/^export_/, "")
		.replace(/_schema$/, "")
		.replace(/_/g, "-");
	const schemaFile = `${baseName}.schema.json`;
	const outputPath = path.join(outDir, schemaFile);

	const result = spawnSync(
		"cargo",
		["run", "--quiet", "--bin", binName, "--manifest-path", manifestPath],
		{
			cwd: appRoot,
			encoding: "utf8",
			stdio: ["ignore", "pipe", "pipe"],
			windowsHide: true,
		},
	);

	if (result.error) {
		throw result.error;
	}

	if (result.status !== 0) {
		throw new Error(
			`Schema export failed for ${binName} (exit ${result.status}).\n${result.stderr ?? ""}`,
		);
	}

	const stdout = (result.stdout ?? "").toString();
	const normalized = `${stdout.replace(/\r\n/g, "\n").trimEnd()}\n`;
	writeFileSync(outputPath, normalized, "utf8");
	generated.push(schemaFile);
}

console.log(`Generated ${generated.length} schemas.`);
