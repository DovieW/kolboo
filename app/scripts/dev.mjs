import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const appDir = path.resolve(scriptDir, "..");
const tauriCli = path.join(
	appDir,
	"node_modules",
	"@tauri-apps",
	"cli",
	"tauri.js",
);

const child = spawn(
	process.execPath,
	[tauriCli, "dev", ...process.argv.slice(2)],
	{
		cwd: appDir,
		stdio: "inherit",
		env: {
			...process.env,
			// Interactive development should be readable by a human. Structured JSON
			// remains the default for other launch paths and rolling file logs remain
			// unchanged.
			KOLBOO_LOG_FORMAT: "pretty",
		},
	},
);

child.on("error", (error) => {
	console.error(`Failed to start Tauri development mode: ${error.message}`);
	process.exitCode = 1;
});

child.on("exit", (code) => {
	process.exitCode = code ?? 1;
});
