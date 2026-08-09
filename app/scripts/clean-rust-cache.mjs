import { lstat, readdir, rm, stat } from "node:fs/promises";
import path from "node:path";

const rustRoot = path.resolve(import.meta.dirname, "../src-tauri");
const targetNames = ["target", "target-ci", "target-cli"];
const apply = process.argv.includes("--apply");

async function directoryBytes(directory) {
	let total = 0;
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const entryPath = path.join(directory, entry.name);
		if (entry.isSymbolicLink()) continue;
		if (entry.isDirectory()) total += await directoryBytes(entryPath);
		else total += (await stat(entryPath)).size;
	}
	return total;
}

function formatBytes(bytes) {
	const gib = bytes / 1024 ** 3;
	return gib >= 0.1 ? `${gib.toFixed(1)} GiB` : `${(bytes / 1024 ** 2).toFixed(1)} MiB`;
}

let total = 0;
for (const name of targetNames) {
	const target = path.join(rustRoot, name);
	if (path.dirname(target) !== rustRoot || !target.startsWith(`${rustRoot}${path.sep}`)) {
		throw new Error(`Refusing unsafe cache path: ${target}`);
	}

	let metadata;
	try {
		metadata = await lstat(target);
	} catch (error) {
		if (error?.code === "ENOENT") continue;
		throw error;
	}
	if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
		throw new Error(`Refusing non-directory or symlink cache path: ${target}`);
	}

	const bytes = await directoryBytes(target);
	total += bytes;
	console.log(`${apply ? "Removing" : "Would remove"} ${target} (${formatBytes(bytes)})`);
	if (apply) await rm(target, { recursive: true, force: false });
}

console.log(`${apply ? "Reclaimed" : "Potentially reclaimable"}: ${formatBytes(total)}`);
if (!apply) console.log("Re-run with --apply to remove only the listed Cargo target directories.");
