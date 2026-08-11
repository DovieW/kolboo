import { spawn } from "node:child_process";
import {
	existsSync,
	mkdirSync,
	readFileSync,
	unlinkSync,
	writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import path, { delimiter } from "node:path";
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

const devDesktopEntryMarker = "X-Kolboo-Development=true";

function linuxDesktopEntryCandidates(identifier) {
	const userDataDir =
		process.env.XDG_DATA_HOME || path.join(homedir(), ".local", "share");
	const systemDataDirs = (
		process.env.XDG_DATA_DIRS || "/usr/local/share:/usr/share"
	).split(delimiter);
	return [userDataDir, ...systemDataDirs].map((dataDir) =>
		path.join(dataDir, "applications", `${identifier}.desktop`),
	);
}

function prepareLinuxPortalIdentity() {
	if (process.platform !== "linux") return () => {};

	const tauriConfig = JSON.parse(
		readFileSync(path.join(appDir, "src-tauri", "tauri.conf.json"), "utf8"),
	);
	const identifier = tauriConfig.identifier;
	const candidates = linuxDesktopEntryCandidates(identifier);
	const [userDesktopEntry, ...systemDesktopEntries] = candidates;

	if (systemDesktopEntries.some((candidate) => existsSync(candidate))) {
		return () => {};
	}

	if (existsSync(userDesktopEntry)) {
		const existing = readFileSync(userDesktopEntry, "utf8");
		if (!existing.includes(devDesktopEntryMarker)) return () => {};
	}

	const executable = path.join(
		appDir,
		"src-tauri",
		"target",
		"debug",
		"kolboo",
	);
	const icon = path.join(appDir, "src-tauri", "icons", "icon.png");
	const contents = [
		"[Desktop Entry]",
		"Type=Application",
		"Name=Kolboo (Development)",
		`Exec=${executable}`,
		`TryExec=${executable}`,
		`Icon=${icon}`,
		"Terminal=false",
		"NoDisplay=true",
		`StartupWMClass=${identifier}`,
		devDesktopEntryMarker,
		"",
	].join("\n");

	mkdirSync(path.dirname(userDesktopEntry), { recursive: true });
	writeFileSync(userDesktopEntry, contents, { mode: 0o644 });

	return () => {
		try {
			if (
				existsSync(userDesktopEntry) &&
				readFileSync(userDesktopEntry, "utf8") === contents
			) {
				unlinkSync(userDesktopEntry);
			}
		} catch (error) {
			console.warn(
				`Could not remove the temporary Linux desktop entry: ${error.message}`,
			);
		}
	};
}

const cleanupLinuxPortalIdentity = prepareLinuxPortalIdentity();
process.on("exit", cleanupLinuxPortalIdentity);

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
