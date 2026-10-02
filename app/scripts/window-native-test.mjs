// Linux-only native window acceptance, isolated from the user's desktop/data.
// Ordinary unit tests remain headless. Coverage explicitly includes this run.
import { spawn, spawnSync } from "node:child_process";
import {
	mkdtempSync,
	mkdirSync,
	openSync,
	closeSync,
	writeFileSync,
	readFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { commandExists, configureRustBuildEnv } from "./rust-build-env.mjs";

const script = fileURLToPath(import.meta.url);
const coverage = process.argv.includes("--coverage");
const inside = process.argv.includes("--inside");
if (process.platform !== "linux") {
	console.error(
		"Native window acceptance currently requires Linux/X11; Windows/macOS need their own native run.",
	);
	process.exit(1);
}

const env = configureRustBuildEnv(process.env, { requireTools: true }).env;
if (!inside) {
	for (const tool of [
		"xvfb-run",
		"Xvfb",
		"openbox",
		"dbus-run-session",
		"xprop",
		"unshare",
		"python3",
	]) {
		if (!commandExists(tool, env)) {
			console.error(
				"Install native window test prerequisites: sudo apt-get install xvfb openbox x11-utils",
			);
			process.exit(1);
		}
	}
	const directory = mkdtempSync(path.join(tmpdir(), "kolboo-window-test-"));
	for (const folder of ["data", "config", "cache"])
		mkdirSync(path.join(directory, folder));
	const result = spawnSync(
		"xvfb-run",
		[
			"-a",
			"-s",
			"-screen 0 2880x1800x24 -nolisten tcp",
			"dbus-run-session",
			"--",
			process.execPath,
			script,
			"--inside",
			...(coverage ? ["--coverage"] : []),
		],
		{
			stdio: "inherit",
			env: {
				...env,
				XDG_DATA_HOME: path.join(directory, "data"),
				XDG_CONFIG_HOME: path.join(directory, "config"),
				XDG_CACHE_HOME: path.join(directory, "cache"),
				KOLBOO_NATIVE_WINDOW_TEST: "1",
				KOLBOO_NATIVE_WINDOW_DIRECTORY: directory,
				GDK_BACKEND: "x11",
				WINIT_UNIX_BACKEND: "x11",
				GDK_SCALE: "1",
				GDK_DPI_SCALE: "1",
				WAYLAND_DISPLAY: "",
				GTK_MODULES: "",
				GTK_A11Y: "none",
			},
		},
	);
	console.log(`Native window test artifacts: ${directory}`);
	process.exit(result.status ?? 1);
}

const manager = spawn("openbox", ["--sm-disable"], {
	env,
	stdio: ["ignore", "ignore", "inherit"],
});
try {
	const deadline = Date.now() + 10000;
	for (;;) {
		const probe = spawnSync("xprop", ["-root", "_NET_SUPPORTING_WM_CHECK"], {
			env,
			encoding: "utf8",
		});
		if (probe.status === 0 && probe.stdout.includes("window id #")) break;
		if (manager.exitCode !== null || Date.now() >= deadline)
			throw new Error("Isolated window manager did not start");
		await new Promise((resolve) => setTimeout(resolve, 30));
	}
	const args = coverage
		? ["llvm-cov", "--no-report", "--lib"]
		: ["test", "--lib"];
	args.push(
		"--manifest-path",
		"src-tauri/Cargo.toml",
		"--",
		"--ignored",
		"--exact",
		"bootstrap::main_window::tests::native_window_manager_integration",
		"--test-threads=1",
		"--nocapture",
	);
	const result = spawnSync("cargo", args, {
		env,
		stdio: "inherit",
		timeout: 180000,
	});
	process.exitCode = result.status ?? 1;
	if (coverage && process.exitCode === 0) {
		// Exercise the real production startup call as well as the minimal native
		// harness. Network is unavailable and both D-Bus/data are isolated. Run
		// from the fixture directory so main.rs cannot load developer .env files.
		const directory = env.KOLBOO_NATIVE_WINDOW_DIRECTORY;
		const data = path.join(env.XDG_DATA_HOME, "com.kolboo.app");
		mkdirSync(data, { recursive: true });
		writeFileSync(
			path.join(data, "settings.json"),
			JSON.stringify({
				settings_guide_state: "pending",
				main_window_close_behavior: "exit_program",
			}),
		);
		const log = openSync(path.join(directory, "startup.log"), "w");
		const startup = spawn(
			"cargo",
			[
				"llvm-cov",
				"run",
				"--no-report",
				"--bin",
				"kolboo",
				"--manifest-path",
				path.resolve("src-tauri/Cargo.toml"),
			],
			{
				env: {
					...env,
					CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER:
						env.KOLBOO_NATIVE_WINDOW_RUNNER ??
						"unshare --user --map-current-user --net --",
				},
				cwd: directory,
				stdio: ["ignore", log, log],
			},
		);
		closeSync(log);
		const closed = new Promise((resolve, reject) => {
			startup.once("exit", resolve);
			startup.once("error", reject);
		});
		let exitTimer;
		let smoke;
		try {
			smoke = spawn(
				"python3",
				[path.join(path.dirname(script), "window-native-smoke.py")],
				{ env, stdio: "inherit" },
			);
			const checked = new Promise((resolve, reject) => {
				smoke.once("exit", resolve);
				smoke.once("error", reject);
			});
			const failedStartup = closed.then((status) => {
				if (status !== 0) {
					// This log belongs to the synthetic offline app, not the user's
					// desktop/data. Keep CI failures diagnosable after its VM is gone.
					console.error(
						readFileSync(path.join(directory, "startup.log"), "utf8")
							.split("\n")
							.slice(-80)
							.join("\n"),
					);
					throw new Error(
						`Native startup exited with ${status}; see ${path.join(directory, "startup.log")}`,
					);
				}
				// Successful exit follows Python's close request. Wait for its
				// assertions too, rather than masking a failed geometry check.
				return checked;
			});
			if ((await Promise.race([checked, failedStartup])) !== 0)
				throw new Error(
					`Native startup check failed; see ${path.join(directory, "startup.log")}`,
				);
			const exited = await Promise.race([
				closed,
				new Promise((_, reject) => {
					exitTimer = setTimeout(
						() => reject(new Error("Native app did not exit after closing")),
						10000,
					);
				}),
			]);
			if (exited !== 0) throw new Error(`Native startup exited with ${exited}`);
		} finally {
			clearTimeout(exitTimer);
			if (smoke && smoke.exitCode === null) smoke.kill("SIGTERM");
			if (startup.exitCode === null) startup.kill("SIGTERM");
		}
	}
} finally {
	manager.kill("SIGTERM");
}
