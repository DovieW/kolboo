import { spawnSync } from "node:child_process";
import { mkdirSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptDir, "..");
const srcTauriDir = path.join(appRoot, "src-tauri");
const outDir = path.join(appRoot, "src-tauri", "gen", "schemas");
const manifestPath = path.join(appRoot, "src-tauri", "Cargo.toml");

mkdirSync(outDir, { recursive: true });

const runCargo = (args, { stdio = ["ignore", "pipe", "pipe"] } = {}) =>
	spawnSync("cargo", args, {
		cwd: srcTauriDir,
		encoding: "utf8",
		stdio,
		windowsHide: true,
	});

const result = runCargo(
	[
		"run",
		"-p",
		"xtask",
		"--manifest-path",
		manifestPath,
		"--",
		"schemas",
		"--out-dir",
		outDir,
	],
	{ stdio: ["ignore", "inherit", "inherit"] },
);

if (result.error) {
	throw result.error;
}

if (result.status !== 0) {
	throw new Error(
		`Schema generation failed (exit ${result.status}).\n${result.stderr ?? ""}`,
	);
}

const generated = readdirSync(outDir)
	.filter((name) => name.endsWith(".schema.json"))
	.sort();

console.log(`Generated ${generated.length} schemas.`);
