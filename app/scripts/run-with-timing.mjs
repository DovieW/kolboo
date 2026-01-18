#!/usr/bin/env node
import { spawn } from "node:child_process";
import path from "node:path";
import process from "node:process";

const args = process.argv.slice(2);
let useShell = false;

if (args[0] === "--shell") {
	useShell = true;
	args.shift();
}

if (args.length === 0) {
	console.error("Usage: run-with-timing.mjs [--shell] <command> [...args]");
	process.exit(1);
}

const start = Date.now();
console.log(`[time] start: ${new Date(start).toISOString()}`);

const binPath = path.join(process.cwd(), "node_modules", ".bin");
const env = {
	...process.env,
	PATH: [binPath, process.env.PATH ?? ""].join(path.delimiter),
};

let child;
if (useShell) {
	const command = args.join(" ");
	child = spawn(command, { stdio: "inherit", shell: true, env });
} else {
	const [command, ...commandArgs] = args;
	const shouldUseShell = process.platform === "win32";
	child = spawn(command, commandArgs, {
		stdio: "inherit",
		env,
		shell: shouldUseShell,
	});
}

child.on("close", (code, signal) => {
	const end = Date.now();
	console.log(`[time] end: ${new Date(end).toISOString()}`);
	const elapsedMs = end - start;
	const elapsedSec = (elapsedMs / 1000).toFixed(2);
	console.log(`[time] elapsed: ${elapsedMs} ms (${elapsedSec}s)`);

	if (signal) {
		console.error(`[time] terminated by signal ${signal}`);
		process.exit(1);
	}

	process.exit(code ?? 1);
});
