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

const color = {
	reset: "\u001b[0m",
	gray: "\u001b[90m",
	cyan: "\u001b[36m",
	green: "\u001b[32m",
	yellow: "\u001b[33m",
	red: "\u001b[31m",
};

const formatLocalTime = (timestamp) => {
	const formatter = new Intl.DateTimeFormat(undefined, {
		year: "numeric",
		month: "2-digit",
		day: "2-digit",
		hour: "numeric",
		minute: "2-digit",
		second: "2-digit",
		fractionalSecondDigits: 3,
		hour12: true,
	});

	return formatter.format(new Date(timestamp));
};

const formatDuration = (elapsedMs) => {
	if (elapsedMs < 1000) {
		return `${elapsedMs}ms`;
	}

	if (elapsedMs < 60_000) {
		return `${(elapsedMs / 1000).toFixed(2)}s`;
	}

	if (elapsedMs < 3_600_000) {
		const minutes = Math.floor(elapsedMs / 60_000);
		const seconds = Math.floor((elapsedMs % 60_000) / 1000)
			.toString()
			.padStart(2, "0");
		return `${minutes}m ${seconds}s`;
	}

	const hours = Math.floor(elapsedMs / 3_600_000);
	const minutes = Math.floor((elapsedMs % 3_600_000) / 60_000)
		.toString()
		.padStart(2, "0");
	return `${hours}h ${minutes}m`;
};

const logTime = (label, message, tint) => {
	const prefix = `${color.cyan}[time]${color.reset}`;
	const coloredLabel = tint ? `${tint}${label}${color.reset}` : label;
	const coloredMessage = tint ? `${tint}${message}${color.reset}` : message;
	console.log(`${prefix} ${coloredLabel}: ${coloredMessage}`);
};

const start = Date.now();
let commandLabel = "";

const env = {
	...process.env,
	PATH: [binPath, process.env.PATH ?? ""].join(path.delimiter),
};

let child;
if (useShell) {
	const command = args.join(" ");
	commandLabel = command;
	child = spawn(command, { stdio: "inherit", shell: true, env });
} else {
	const [command, ...commandArgs] = args;
	commandLabel = [command, ...commandArgs].join(" ");
	const resolvedCommand = resolveBin(command);
	const isWindows = process.platform === "win32";
	const isCmdShim = isWindows && /\.(cmd|bat)$/i.test(resolvedCommand);

	if (isCmdShim) {
		child = spawn(
			"cmd.exe",
			["/d", "/s", "/c", resolvedCommand, ...commandArgs],
			{
				stdio: "inherit",
				env,
			},
		);
	} else {
		child = spawn(resolvedCommand, commandArgs, {
			stdio: "inherit",
			env,
		});
	}
}

logTime("start", `${formatLocalTime(start)} (${commandLabel})`, color.green);

child.on("close", (code, signal) => {
	const end = Date.now();
	const elapsedMs = end - start;
	logTime("end", `${formatLocalTime(end)} (${commandLabel})`, color.green);
	logTime("elapsed", formatDuration(elapsedMs), color.yellow);

	if (signal) {
		console.error(
			`${color.red}[time] terminated by signal ${signal}${color.reset}`,
		);
		process.exit(1);
	}

	process.exit(code ?? 1);
});
