import { spawnSync } from "node:child_process";

const useCiTarget = process.argv.includes("--ci");
const useLocalWhisper = process.argv.includes("--local-whisper");

// The application currently keeps Windows UIA and modifier-only shortcut modules
// available to cross-platform tests. On Linux and macOS, rustc consequently sees
// those production paths as unreachable and reports them as dead even though they
// are live in the Windows product. Run the strict reachability gate on Windows,
// where every Windows-owned path can actually be compiled and assessed.
if (process.platform !== "win32") {
	console.log(
		"Skipping strict Rust dead-code denial on this non-Windows target; ordinary Clippy and tests still run here, and Windows CI owns the current strict reachability gate.",
	);
	process.exit(0);
}

const args = [
	"clippy",
	"--all-targets",
	"--manifest-path",
	"src-tauri/Cargo.toml",
];

if (useLocalWhisper) {
	args.push("--features", "local-whisper");
}

if (useCiTarget) {
	args.push("--target-dir", "src-tauri/target-ci");
}

args.push("--", "-D", "dead-code");

const result = spawnSync("cargo", args, {
	cwd: new URL("..", import.meta.url),
	stdio: "inherit",
	windowsHide: true,
});

if (result.error) throw result.error;
process.exit(result.status ?? 1);
