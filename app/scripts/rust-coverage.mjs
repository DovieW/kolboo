#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

export function conservativeCargoJobs(cpuCount = os.cpus().length) {
	return Math.min(8, Math.max(1, Math.floor(cpuCount / 2)));
}

export function buildRustCoverageArgs(options = {}) {
	const manifestPath = options.manifestPath ?? "src-tauri/Cargo.toml";
	const args = ["llvm-cov", "--manifest-path", manifestPath, "--summary-only"];

	for (const packageName of options.packages ?? []) {
		args.push("--package", packageName);
	}

	for (const testName of options.tests ?? []) {
		args.push("--test", testName);
	}

	if (options.allFeatures) {
		args.push("--all-features");
	}

	return args;
}

export function createRustCoverageEnvironment(baseEnv = process.env) {
	return {
		...baseEnv,
		CARGO_BUILD_JOBS:
			baseEnv.CARGO_BUILD_JOBS ?? String(conservativeCargoJobs()),
	};
}

export function validateRustCoverageOptions(options = {}) {
	if (options.requireTool === false) {
		return [];
	}

	return [
		"cargo llvm-cov must be installed before Rust in-scope coverage can be claimed.",
		"Install with: cargo install cargo-llvm-cov",
	];
}

export function isCliEntrypoint(
	metaUrl,
	argvPath = process.argv[1],
	platform = process.platform,
) {
	if (!argvPath) {
		return false;
	}

	const modulePath = path.resolve(fileURLToPath(metaUrl));
	const invokedPath = path.resolve(argvPath);
	const normalize = (value) =>
		platform === "win32" ? value.toLowerCase() : value;

	return normalize(modulePath) === normalize(invokedPath);
}

function parseArgs(argv) {
	const args = [...argv];
	const options = {
		manifestPath: "src-tauri/Cargo.toml",
		packages: [],
		tests: [],
		allFeatures: false,
	};

	while (args.length > 0) {
		const arg = args.shift();
		if (arg === "--manifest-path") {
			options.manifestPath = args.shift() ?? options.manifestPath;
		} else if (arg === "--package") {
			const packageName = args.shift();
			if (packageName) {
				options.packages.push(packageName);
			}
		} else if (arg === "--test") {
			const testName = args.shift();
			if (testName) {
				options.tests.push(testName);
			}
		} else if (arg === "--all-features") {
			options.allFeatures = true;
		}
	}

	return options;
}

export function runRustCoverageCli(argv = process.argv.slice(2)) {
	const options = parseArgs(argv);
	const cargoArgs = buildRustCoverageArgs(options);
	const env = createRustCoverageEnvironment();

	console.log(`[rust-coverage] cargo ${cargoArgs.join(" ")}`);
	console.log(`[rust-coverage] CARGO_BUILD_JOBS=${env.CARGO_BUILD_JOBS}`);

	const result = spawnSync("cargo", cargoArgs, {
		stdio: "inherit",
		env,
	});

	if (result.error) {
		console.error(`[rust-coverage] ${result.error.message}`);
		return 1;
	}

	return result.status ?? 1;
}

if (isCliEntrypoint(import.meta.url)) {
	process.exit(runRustCoverageCli());
}
