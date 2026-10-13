import { lstat, readdir } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

// Host Mesa uses libwayland-client; bundling an older copy can abort WebKit
// with EGL_BAD_PARAMETER. CLI 2.12.1 excludes this library upstream. Other
// GTK dependencies remain bundled, so do not reject the stock fixed bundler
// simply because it includes libxkbcommon or other Wayland libraries:
// https://github.com/tauri-apps/tauri/issues/15976
const hostDisplayLibrary = /^libwayland-client\.so(?:\.\d+)*$/;

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
