#!/usr/bin/env node
// Run cargo-machete with a clearer local error than Cargo's generic
// "no such command" message.
//
// We intentionally do NOT auto-install cargo-machete here because a quality
// command should not mutate a developer's global Cargo toolchain behind their
// back. CI installs the tool explicitly, and local developers can do the same
// once with `cargo install cargo-machete --locked`.
import { spawnSync } from "node:child_process";
import process from "node:process";

const args = process.argv.slice(2);
const targetPath = args[0] ?? "src-tauri";
const extraArgs = args.slice(1);
const cargoBin = process.platform === "win32" ? "cargo.exe" : "cargo";

const listResult = spawnSync(cargoBin, ["--list"], {
	encoding: "utf8",
});

if (listResult.error) {
	console.error(listResult.error.message);
	process.exit(1);
}

const availableCommands = `${listResult.stdout ?? ""}\n${listResult.stderr ?? ""}`;
const hasMachete = availableCommands
	.split(/\r?\n/)
	.some((line) => line.trimStart().startsWith("machete"));

if (!hasMachete) {
	console.error(
		[
			"cargo-machete is required for Rust dead-dependency checks, but it is not installed.",
			"Install it once with: cargo install cargo-machete --locked",
			"CI installs cargo-machete explicitly before running this check.",
		].join("\n"),
	);
	process.exit(1);
}

// Keep the Rust check scoped to the Tauri backend workspace instead of assuming
// the repo root is itself a Cargo project.
const result = spawnSync(cargoBin, ["machete", targetPath, ...extraArgs], {
	stdio: "inherit",
});

if (result.error) {
	console.error(result.error.message);
	process.exit(1);
}

process.exit(result.status ?? 1);
