import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { availableParallelism, homedir } from "node:os";
import path from "node:path";

const LOCAL_BIN = path.join(homedir(), ".local", "bin");
const MOLD_LINKER_FLAG = "-C link-arg=-fuse-ld=mold";

export function withUserLocalBin(envPath = "") {
	const entries = envPath.split(path.delimiter).filter(Boolean);
	if (existsSync(LOCAL_BIN) && !entries.includes(LOCAL_BIN)) {
		entries.unshift(LOCAL_BIN);
	}
	return entries.join(path.delimiter);
}

export function commandExists(command, env = process.env) {
	const probe = process.platform === "win32" ? "where" : "which";
	return (
		spawnSync(probe, [command], {
			stdio: "ignore",
			env,
		}).status === 0
	);
}

export function moldLinkerWorks(env = process.env) {
	return (
		spawnSync("cc", ["-fuse-ld=mold", "-Wl,--version"], {
			stdio: "ignore",
			env,
		}).status === 0
	);
}

export function inspectRustBuildTools({
	platform = process.platform,
	env = process.env,
	commandExistsFn = commandExists,
	moldLinkerWorksFn = moldLinkerWorks,
} = {}) {
	const normalizedEnv = {
		...env,
		PATH: withUserLocalBin(env.PATH ?? ""),
	};
	const moldBinary =
		platform !== "linux" || commandExistsFn("mold", normalizedEnv);
	return {
		env: normalizedEnv,
		sccache: commandExistsFn("sccache", normalizedEnv),
		mold:
			moldBinary && (platform !== "linux" || moldLinkerWorksFn(normalizedEnv)),
		moldBinary,
		moldRequired: platform === "linux",
	};
}

export function rustBuildSetupInstructions(platform = process.platform) {
	if (platform === "linux") {
		return [
			"Install the required Rust build accelerators:",
			"  sudo apt-get install -y sccache mold",
			"Then verify the environment:",
			"  pnpm -C app setup:check",
		].join("\n");
	}
	if (platform === "darwin") {
		return [
			"Install the required Rust compiler cache:",
			"  brew install sccache",
			"Then verify the environment:",
			"  pnpm -C app setup:check",
		].join("\n");
	}
	return [
		"Install the required Rust compiler cache:",
		"  scoop install sccache",
		"Then verify the environment:",
		"  pnpm -C app setup:check",
	].join("\n");
}

export function configureRustBuildEnv(
	baseEnv = process.env,
	{ requireTools = true } = {},
) {
	const status = inspectRustBuildTools({ env: baseEnv });
	const missing = [
		...(status.sccache ? [] : ["sccache"]),
		...(status.mold ? [] : ["mold"]),
	];
	if (requireTools && missing.length > 0) {
		throw new Error(
			`Missing required Rust build tool${missing.length === 1 ? "" : "s"}: ${missing.join(", ")}\n\n${rustBuildSetupInstructions()}`,
		);
	}

	const env = { ...status.env };
	if (status.sccache && !env.RUSTC_WRAPPER) {
		env.RUSTC_WRAPPER = "sccache";
	}
	const usesSccache = /(^|[\\/])sccache(?:\.exe)?$/i.test(
		env.RUSTC_WRAPPER ?? "",
	);
	if (status.sccache && !env.SCCACHE_CACHE_SIZE) {
		env.SCCACHE_CACHE_SIZE = "20G";
	}
	if (usesSccache && env.CARGO_INCREMENTAL === undefined) {
		env.CARGO_INCREMENTAL = "0";
	}
	if (
		process.platform === "linux" &&
		status.mold &&
		env.KOLBOO_DISABLE_MOLD !== "1" &&
		!(env.RUSTFLAGS ?? "").includes(MOLD_LINKER_FLAG)
	) {
		env.RUSTFLAGS = [env.RUSTFLAGS, MOLD_LINKER_FLAG].filter(Boolean).join(" ");
	}
	if (!env.CARGO_BUILD_JOBS) {
		env.CARGO_BUILD_JOBS = String(
			Math.max(2, Math.min(8, availableParallelism() - 2)),
		);
	}

	return { env, status, missing };
}

export function describeRustBuildEnv(status, env) {
	const parts = [
		`sccache=${status.sccache ? "on" : "missing"}`,
		...(status.moldRequired ? [`mold=${status.mold ? "on" : "missing"}`] : []),
		`incremental=${env.CARGO_INCREMENTAL === "0" ? "off" : "on"}`,
		`jobs=${env.CARGO_BUILD_JOBS}`,
	];
	return parts.join(" ");
}
