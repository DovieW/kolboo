import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { checkAppImageLibraries } from "./check-appimage-libraries.mjs";

async function fixture(t) {
	const root = await mkdtemp(path.join(os.tmpdir(), "kolboo-appimage-inventory-"));
	t.after(() => rm(root, { recursive: true, force: true }));
	await mkdir(path.join(root, "usr/lib/x86_64-linux-gnu"), { recursive: true });
	return root;
}

test("accepts application libraries and does not follow links outside the bundle", async (t) => {
	const root = await fixture(t);
	await writeFile(path.join(root, "usr/lib/libwebkit2gtk-4.1.so.0"), "synthetic");
	for (const name of ["libwayland-server.so.0", "libwayland-cursor.so.0", "libwayland-egl.so.1", "libxkbcommon.so.0", "libxcb-randr.so.0", "libxcb-render.so.0", "libxcb-shm.so.0", "libXau.so.6", "libXdmcp.so.6"]) {
		await writeFile(path.join(root, "usr/lib", name), "synthetic fixed-bundler dependency");
	}
	await symlink("missing-library.so", path.join(root, "usr/lib/libapplication.so"));
	await symlink("/unavailable-host-directory", path.join(root, "usr/lib/host"));
	await checkAppImageLibraries(root);
	const cli = spawnSync(process.execPath, [new URL("./check-appimage-libraries.mjs", import.meta.url).pathname, root], { encoding: "utf8" });
	assert.equal(cli.status, 0, cli.stderr);
	assert.match(cli.stdout, /inventory passed/);
});

for (const name of ["libwayland-client.so.0", "libwayland-client.so", "libwayland-client.so.0.23.0"]) {
	test(`rejects host dependency ${name}, including nested libraries and dangling symlinks`, async (t) => {
		const root = await fixture(t);
		const relative = `usr/lib/x86_64-linux-gnu/${name}`;
		await symlink("absent-target.so", path.join(root, relative));
		await assert.rejects(checkAppImageLibraries(root), (error) => {
			assert.match(error.message, /shadows host display libraries/);
			assert.ok(error.message.includes(relative));
			return true;
		});
		const cli = spawnSync(process.execPath, [new URL("./check-appimage-libraries.mjs", import.meta.url).pathname, root], { encoding: "utf8" });
		assert.notEqual(cli.status, 0);
		assert.ok(cli.stderr.includes(relative));
	});
}

test("fails closed for missing, non-directory or linked usr roots and invalid CLI input", async (t) => {
	const root = await fixture(t);
	await assert.rejects(checkAppImageLibraries(path.join(root, "missing")), /ENOENT/);
	await mkdir(path.join(root, "invalid"));
	await symlink("../usr", path.join(root, "invalid/usr"));
	await assert.rejects(checkAppImageLibraries(path.join(root, "invalid")), /real usr directory/);
	await rm(path.join(root, "invalid/usr"));
	await writeFile(path.join(root, "invalid/usr"), "synthetic");
	await assert.rejects(checkAppImageLibraries(path.join(root, "invalid")), /real usr directory/);
	for (const args of [[], [root, "extra"], [path.join(root, "missing")]]) {
		const cli = spawnSync(process.execPath, [new URL("./check-appimage-libraries.mjs", import.meta.url).pathname, ...args], { encoding: "utf8" });
		assert.notEqual(cli.status, 0);
	}
});

test("reports every incompatible real file in deterministic order", async (t) => {
	const root = await fixture(t);
	await writeFile(path.join(root, "usr/lib/libwayland-client.so.0"), "synthetic");
	await writeFile(path.join(root, "usr/lib/x86_64-linux-gnu/libwayland-client.so.0.23.0"), "synthetic");
	await assert.rejects(checkAppImageLibraries(root), (error) => {
		assert.ok(error.message.endsWith("usr/lib/libwayland-client.so.0\nusr/lib/x86_64-linux-gnu/libwayland-client.so.0.23.0"));
		return true;
	});
});

test("Linux packaging checks the extracted image before copying artifacts", async () => {
	const workflow = await readFile(new URL("../workflows/linux-build.yml", import.meta.url), "utf8");
	assert.match(workflow, /node --test \.github\/scripts\/check-appimage-libraries.test.mjs/);
	const inventory = workflow.indexOf('node .github/scripts/check-appimage-libraries.mjs "$appimage_extract_dir/squashfs-root"');
	assert.ok(inventory > workflow.indexOf("--appimage-extract >/dev/null"));
	assert.ok(inventory < workflow.indexOf('cp "$appimage_path" artifacts/'));
});
