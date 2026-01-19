import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
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

const runCargo = (args, { stdio = ["ignore", "pipe", "pipe"] } = {}) =>
	spawnSync("cargo", args, {
		cwd: appRoot,
		encoding: "utf8",
		stdio,
		windowsHide: true,
	});

const metadataResult = runCargo([
	"metadata",
	"--format-version",
	"1",
	"--no-deps",
	"--manifest-path",
	manifestPath,
]);

if (metadataResult.error) {
	throw metadataResult.error;
}

if (metadataResult.status !== 0) {
	throw new Error(
		`Cargo metadata failed (exit ${metadataResult.status}).\n${metadataResult.stderr ?? ""}`,
	);
}

const metadata = JSON.parse(metadataResult.stdout ?? "{}");
const targetDir = metadata.target_directory;

if (!targetDir) {
	throw new Error("Cargo metadata did not include target_directory.");
}

const buildResult = runCargo(
	["build", "--bins", "--manifest-path", manifestPath, "--quiet"],
	{ stdio: ["ignore", "inherit", "inherit"] },
);

if (buildResult.error) {
	throw buildResult.error;
}

if (buildResult.status !== 0) {
	throw new Error(
		`Cargo build failed (exit ${buildResult.status}).\n${buildResult.stderr ?? ""}`,
	);
}

const debugDir = path.join(targetDir, "debug");
const binExtension = process.platform === "win32" ? ".exe" : "";

for (const file of binFiles) {
	const binName = file.replace(/\.rs$/, "");
	const baseName = binName
		.replace(/^export_/, "")
		.replace(/_schema$/, "")
		.replace(/_/g, "-");
	const schemaFile = `${baseName}.schema.json`;
	const outputPath = path.join(outDir, schemaFile);

	const binPath = path.join(debugDir, `${binName}${binExtension}`);

	if (!existsSync(binPath)) {
		throw new Error(`Schema export binary not found at ${binPath}`);
	}

	const result = spawnSync(binPath, [], {
		cwd: appRoot,
		encoding: "utf8",
		stdio: ["ignore", "pipe", "pipe"],
		windowsHide: true,
	});

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
