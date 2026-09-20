#!/usr/bin/env node
import { spawn } from "node:child_process";
import { readdir, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
	configureRustBuildEnv,
	describeRustBuildEnv,
} from "./rust-build-env.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const appDir = path.resolve(scriptDir, "..");
const tauriCli = path.join(
	appDir,
	"node_modules",
	"@tauri-apps",
	"cli",
	"tauri.js",
);

function parseArgs(args) {
	let host = null;
	let install = false;
	for (let index = 0; index < args.length; index += 1) {
		const argument = args[index];
		if (argument === "--host") {
			host = args[index + 1] ?? null;
			index += 1;
		} else if (argument === "--install") {
			install = true;
		} else {
			throw new Error(`Unknown argument: ${argument}`);
		}
	}
	if (host && !/^[a-zA-Z0-9._@-]+$/.test(host)) {
		throw new Error(`Unsafe SSH host: ${host}`);
	}
	if (install && !host) {
		throw new Error("--install requires --host");
	}
	return { host, install };
}

function run(command, args, options = {}) {
	return new Promise((resolve, reject) => {
		const child = spawn(command, args, {
			cwd: appDir,
			stdio: "inherit",
			...options,
		});
		child.on("error", reject);
		child.on("exit", (code, signal) => {
			if (code === 0) resolve();
			else {
				reject(
					new Error(
						`${command} failed${signal ? ` with ${signal}` : ` with exit code ${code}`}`,
					),
				);
			}
		});
	});
}

async function newestDeb() {
	const directory = path.join(
		appDir,
		"src-tauri",
		"target",
		"debug",
		"bundle",
		"deb",
	);
	const candidates = (await readdir(directory))
		.filter((entry) => entry.endsWith(".deb"))
		.map((entry) => path.join(directory, entry));
	if (candidates.length === 0) {
		throw new Error(`No Debian package was produced under ${directory}`);
	}
	const withStats = await Promise.all(
		candidates.map(async (candidate) => ({
			candidate,
			mtimeMs: (await stat(candidate)).mtimeMs,
		})),
	);
	return withStats.sort((a, b) => b.mtimeMs - a.mtimeMs)[0].candidate;
}

async function main() {
	if (process.platform !== "linux") {
		throw new Error("The fast Linux package path can only run on Linux.");
	}
	const { host, install } = parseArgs(process.argv.slice(2));
	const { env, status } = configureRustBuildEnv(process.env, {
		requireTools: true,
	});
	const buildEnv = {
		...env,
		TAURI_API_BASE_URL: env.TAURI_API_BASE_URL ?? "https://kolboo.dovie.dev",
		TAURI_MANAGED_INFERENCE_GATEWAY_URL:
			env.TAURI_MANAGED_INFERENCE_GATEWAY_URL ?? "https://kolboo.dovie.dev",
		VITE_SIGNED_UPDATER_ENABLED: env.VITE_SIGNED_UPDATER_ENABLED ?? "false",
	};

	console.log(
		`Fast Linux package: ${describeRustBuildEnv(status, buildEnv)} profile=dev`,
	);
	const start = Date.now();
	await run(
		process.execPath,
		[tauriCli, "build", "--debug", "--no-sign", "--bundles", "deb"],
		{ env: buildEnv },
	);
	const deb = await newestDeb();
	console.log(
		`Built ${deb} in ${((Date.now() - start) / 1000).toFixed(1)} seconds.`,
	);

	if (!host) return;
	const remoteDirectory = "~/.cache/kolboo-dev";
	const remoteDeb = `${remoteDirectory}/${path.basename(deb)}`;
	await run("ssh", [host, "mkdir", "-p", remoteDirectory]);
	await run("scp", [deb, `${host}:${remoteDeb}`]);
	console.log(`Copied package to ${host}:${remoteDeb}`);
	if (install) {
		await run("ssh", ["-t", host, "sudo", "dpkg", "-i", remoteDeb]);
		console.log(`Installed the fast development package on ${host}.`);
	} else {
		console.log(`Install when ready: ssh -t ${host} sudo dpkg -i ${remoteDeb}`);
	}
}

main().catch((error) => {
	console.error(error instanceof Error ? error.message : String(error));
	process.exit(1);
});
