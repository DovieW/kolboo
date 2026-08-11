import { spawn, spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const appDir = path.resolve(__dirname, "..");

function commandExists(command) {
	const probe = process.platform === "win32" ? "where" : "which";
	const res = spawnSync(probe, [command], {
		stdio: "ignore",
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

// Keep interactive development readable even when compiler caching is enabled.
env.KOLBOO_LOG_FORMAT = "pretty";

// Enable Rust compiler caching for Cargo builds invoked by Tauri.
// Docs: https://github.com/mozilla/sccache/blob/main/docs/Rust.md
if (!env.RUSTC_WRAPPER) {
	env.RUSTC_WRAPPER = "sccache";
}

if (!env.CARGO_BUILD_JOBS) {
	const logicalCores = Number.parseInt(
		process.env.NUMBER_OF_PROCESSORS ?? "",
		10,
	);
	const halfCores = Number.isFinite(logicalCores)
		? Math.floor(logicalCores / 2)
		: 4;
	env.CARGO_BUILD_JOBS = String(Math.max(1, Math.min(8, halfCores)));
}

// Optional knobs (leave unset unless you know you want them):
// - SCCACHE_DIR: cache location (put on a fast SSD)
// - SCCACHE_CACHE_SIZE: e.g. "20G"

const child =
	process.platform === "win32"
		? spawn("cmd.exe", ["/d", "/s", "/c", "tauri", "dev"], {
				cwd: appDir,
				stdio: "inherit",
				env,
			})
		: spawn("tauri", ["dev"], {
				cwd: appDir,
				stdio: "inherit",
				env,
			});

child.on("exit", (code) => {
	process.exit(code ?? 1);
});
