import { lstat, readdir } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

// These host display libraries must not shadow the host Mesa drivers. Older
// linuxdeploy bundled them and caused WebKit EGL_BAD_PARAMETER on newer Mesa:
// https://github.com/tauri-apps/tauri/issues/15976
const hostDisplayLibrary = /^lib(?:wayland-(?:client|server|cursor|egl)|xkbcommon|xcb-(?:randr|render|shm)|Xau|Xdmcp)\.so(?:\.\d+)*$/;

export async function checkAppImageLibraries(appDir) {
	if (!(await lstat(path.join(appDir, "usr"))).isDirectory()) {
		throw new Error("Expected an extracted AppImage with a real usr directory.");
	}
	const incompatible = [];
	async function walk(directory) {
		for (const entry of await readdir(directory, { withFileTypes: true })) {
			const candidate = path.join(directory, entry.name);
			if (hostDisplayLibrary.test(entry.name)) incompatible.push(path.relative(appDir, candidate));
			// Do not follow library/directory links outside the extracted package.
			if (entry.isDirectory()) await walk(candidate);
		}
	}
	await walk(appDir);
	if (incompatible.length) {
		throw new Error(`AppImage shadows host display libraries; rebuild with the fixed Tauri bundler:\n${incompatible.sort().join("\n")}`);
	}
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	const [appDir, ...extra] = process.argv.slice(2);
	if (!appDir || extra.length) throw new Error("Usage: check-appimage-libraries.mjs <extracted-appimage-directory>");
	await checkAppImageLibraries(appDir);
	console.log("AppImage host display-library inventory passed.");
}
