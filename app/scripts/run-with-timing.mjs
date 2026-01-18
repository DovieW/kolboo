#!/usr/bin/env node
// Usage: run-with-timing.mjs [--shell] <command> [...args]
// --shell is an explicit fallback for shell-only commands (pipes, &&).
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
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

const binPath = path.join(process.cwd(), "node_modules", ".bin");
const resolveBin = (command) => {
	if (path.isAbsolute(command) || command.includes(path.sep)) {
		return command;
	}

	const candidates =
		process.platform === "win32"
			? [`${command}.cmd`, `${command}.exe`, `${command}.bat`, command]
			: [command];

	for (const candidate of candidates) {
		const candidatePath = path.join(binPath, candidate);
		if (existsSync(candidatePath)) {
			return candidatePath;
		}
	}

	return command;
};

const start = Date.now();
console.log(`[time] start: ${new Date(start).toISOString()}`);

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
	const resolvedCommand = resolveBin(command);
	child = spawn(resolvedCommand, commandArgs, {
		stdio: "inherit",
		env,
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
