import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const appDir = path.resolve(__dirname, "..", "..");

function commandExists(command) {
	const probe = process.platform === "win32" ? "where" : "which";
	const res = spawnSync(probe, [command], {
		stdio: "ignore",
		shell: true,
	});
	return res.status === 0;
}

if (!commandExists("sccache")) {
	console.error(
		[
			"sccache was not found on PATH.",
			"",
			"Install it first, then rerun:",
			"  pnpm -C app dev:sccache",
			"",
			"Windows install options:",
			"- scoop install sccache",
			"- choco install sccache",
			"- cargo install sccache",
		].join("\n"),
	);
	process.exit(1);
}

const env = { ...process.env };

// Enable Rust compiler caching for Cargo builds invoked by Tauri.
// Docs: https://github.com/mozilla/sccache/blob/main/docs/Rust.md
if (!env.RUSTC_WRAPPER) {
	env.RUSTC_WRAPPER = "sccache";
}

// Optional knobs (leave unset unless you know you want them):
// - SCCACHE_DIR: cache location (put on a fast SSD)
// - SCCACHE_CACHE_SIZE: e.g. "20G"

const tauriCmd = process.platform === "win32" ? "tauri.cmd" : "tauri";
const child = spawn(tauriCmd, ["dev"], {
	cwd: appDir,
	stdio: "inherit",
	shell: true,
	env,
});

child.on("exit", (code) => {
	process.exit(code ?? 1);
});
