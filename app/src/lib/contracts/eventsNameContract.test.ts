import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { EVENT_NAMES } from "../tauri/events";

const rustRoot = fileURLToPath(
	new URL("../../../src-tauri/src", import.meta.url),
);

const ignoredDirs = new Set([
	"target",
	"target-ci",
	"target-rust-analyzer",
	"gen",
]);

function listRustFiles(dir: string): string[] {
	const entries = fs.readdirSync(dir, { withFileTypes: true });
	const files: string[] = [];

	for (const entry of entries) {
		if (entry.name.startsWith(".")) continue;
		const fullPath = path.join(dir, entry.name);
		if (entry.isDirectory()) {
			if (ignoredDirs.has(entry.name)) continue;
			files.push(...listRustFiles(fullPath));
			continue;
		}
		if (entry.isFile() && entry.name.endsWith(".rs")) {
			files.push(fullPath);
		}
	}

	return files;
}

function extractEventNames(source: string): string[] {
	const names = new Set<string>();
	const emitRegex = /\.emit\(\s*"([^"]+)"/g;
	const quickAskRegex = /emit_to_quick_ask\(\s*[^,]+,\s*"([^"]+)"/g;
	const constEventRegex =
		/const\s+[A-Z0-9_]*EVENT[A-Z0-9_]*\s*:\s*&str\s*=\s*"([^"]+)"/g;

	for (const match of source.matchAll(emitRegex)) {
		const name = match[1];
		if (name) names.add(name);
	}

	for (const match of source.matchAll(quickAskRegex)) {
		const name = match[1];
		if (name) names.add(name);
	}

	for (const match of source.matchAll(constEventRegex)) {
		const name = match[1];
		if (name) names.add(name);
	}

	return [...names];
}

describe("event name contract: Rust emits vs TS EventMap", () => {
	it("keeps Rust emit event names in sync with EventMap keys", () => {
		const rustFiles = listRustFiles(rustRoot);
		const rustEventNames = new Set<string>();
		for (const file of rustFiles) {
			const source = fs.readFileSync(file, "utf8");
			for (const name of extractEventNames(source)) {
				rustEventNames.add(name);
			}
		}

		const tsEventNames = new Set<string>(EVENT_NAMES);

		// Events emitted by the frontend (no Rust emit), still part of the UI contract.
		const allowMissingInRust = new Set<string>(["connection-state-changed"]);

		const missingInTs = [...rustEventNames]
			.filter((name) => !tsEventNames.has(name))
			.sort();
		const missingInRust = EVENT_NAMES.filter(
			(name) => !rustEventNames.has(name) && !allowMissingInRust.has(name),
		).sort();

		expect(
			missingInTs,
			`Rust emits events not present in EventMap: ${missingInTs.join(", ")}`,
		).toEqual([]);

		expect(
			missingInRust,
			`EventMap contains events not emitted in Rust: ${missingInRust.join(
				", ",
			)}`,
		).toEqual([]);
	});
});
