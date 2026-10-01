#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { configureRustBuildEnv } from "./rust-build-env.mjs";

export function conservativeCargoJobs(cpuCount = os.cpus().length) {
	return Math.min(8, Math.max(1, Math.floor(cpuCount / 2)));
}

// Rust v0 symbols include build-specific crate disambiguators. A unit test
// and the same code linked into the desktop binary therefore have different
// symbols. Merge only identical demangled functions, retaining generic types,
// closure indices and the source-file boundary. Keep a raw symbol as the alias
// so the gate's strict Tauri-wrapper checks still recognize LLVM record shapes.
export function mergeRustBuildIdentities(lcov, demangle) {
	const names = [
		...new Set(
			lcov
				.split("\n")
				.filter((line) => /^FN:\d+,_R/u.test(line))
				.map((line) => line.slice(line.indexOf(",") + 1)),
		),
	];
	if (names.length === 0) return lcov;
	const decoded = demangle(names);
	if (decoded.length !== names.length || decoded.some((name) => !name))
		throw new Error("Incomplete Rust coverage demangling");
	const identities = new Map(
		names.map((name, index) => [name, decoded[index]]),
	);
	const files = new Map();
	let aliases = new Map();
	let definitions = new Map();
	return lcov
		.split("\n")
		.map((line) => {
			if (line.startsWith("SF:")) {
				aliases = files.get(line) ?? new Map();
				files.set(line, aliases);
				definitions = new Map();
			}
			if (!/^FN(?:DA)?:\d+,_R/u.test(line)) return line;
			const separator = line.indexOf(",");
			const raw = line.slice(separator + 1);
			const identity = identities.get(raw);
			if (!identity) throw new Error("Rust function count lacks a definition");
			if (line.startsWith("FN:"))
				definitions.set(raw, line.slice(3, separator));
			const location = definitions.get(raw);
			if (!location)
				throw new Error("Rust function count lacks a source location");
			const key = `${location}:${identity}`;
			const alias = aliases.get(key) ?? raw;
			aliases.set(key, alias);
			return line.slice(0, separator + 1) + alias;
		})
		.join("\n");
}

export function buildRustCoverageArgs(options = {}) {
	const manifestPath = options.manifestPath ?? "src-tauri/Cargo.toml";
	const args = ["llvm-cov", "--manifest-path", manifestPath];
	if (options.lcov) {
		args.push("--lcov");
		if (options.outputPath) args.push("--output-path", options.outputPath);
	} else {
		args.push("--summary-only");
	}

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
		"Install with: cargo +stable install cargo-llvm-cov --locked --version 0.9.1",
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
		lcov: false,
		outputPath: undefined,
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
		} else if (arg === "--lcov") {
			options.lcov = true;
		} else if (arg === "--output-path") {
			options.outputPath = args.shift();
		}
	}

	return options;
}

export function runRustCoverageCli(argv = process.argv.slice(2)) {
	const options = parseArgs(argv);
	const cargoArgs = buildRustCoverageArgs(options);
	const env = configureRustBuildEnv(createRustCoverageEnvironment(), {
		requireTools: true,
	}).env;

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

	if (result.status !== 0) return result.status ?? 1;
	// Keep normal unit tests headless. Linux coverage additionally exercises the
	// real window boundary in an isolated display, then merges those profiles.
	if (
		process.platform === "linux" &&
		options.tests.length === 0 &&
		options.packages.length === 0
	) {
		const native = spawnSync(
			process.execPath,
			[
				fileURLToPath(new URL("./window-native-test.mjs", import.meta.url)),
				"--coverage",
			],
			{ env, stdio: "inherit" },
		);
		if (native.status !== 0) return native.status ?? 1;
		const report = spawnSync(
			"cargo",
			[cargoArgs[0], "report", ...cargoArgs.slice(1)],
			{ env, stdio: "inherit" },
		);
		if (report.status !== 0) return report.status ?? 1;
		if (options.lcov && options.outputPath) {
			const merged = mergeRustBuildIdentities(
				readFileSync(options.outputPath, "utf8"),
				(names) => {
					const result = spawnSync(
						"c++filt",
						["--format=rust", "--no-verbose"],
						{
							input: `${names.join("\n")}\n`,
							encoding: "utf8",
							maxBuffer: 32 * 1024 * 1024,
							env,
						},
					);
					if (result.status !== 0)
						throw new Error(
							"Rust coverage requires binutils c++filt with Rust v0 support",
						);
					return result.stdout.trimEnd().split("\n");
				},
			);
			writeFileSync(options.outputPath, merged);
		}
		return 0;
	}
	return 0;
}

if (isCliEntrypoint(import.meta.url)) {
	process.exit(runRustCoverageCli());
}
