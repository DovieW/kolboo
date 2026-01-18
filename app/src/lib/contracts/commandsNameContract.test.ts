import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const tsTauriPath = fileURLToPath(new URL("../tauri.ts", import.meta.url));
const rustLibPath = fileURLToPath(
	new URL("../../../src-tauri/src/lib.rs", import.meta.url),
);

function extractInvokeCommandNames(source: string): string[] {
	const names = new Set<string>();
	const re = /invoke(?:<[^>]*>)?\s*\(\s*"([^"]+)"/g;

	for (const match of source.matchAll(re)) {
		const name = match[1];
		if (name) names.add(name);
	}

	return [...names].sort();
}

function extractRustCommandNames(source: string): string[] {
	const handlerMatch = source.match(/generate_handler!\s*\[([\s\S]*?)\]/);
	if (!handlerMatch) return [];

	const cleaned = handlerMatch[1]
		.replace(/\/\/.*$/gm, "")
		.replace(/\/\*[\s\S]*?\*\//g, "");

	const entries = cleaned
		.split(",")
		.map((entry) => entry.trim())
		.filter(Boolean);

	const names = entries.map((entry) => {
		const compact = entry.replace(/\s+/g, "");
		const parts = compact.split("::");
		return parts[parts.length - 1] ?? compact;
	});

	return [...new Set(names)].sort();
}

describe("command name contract: TS invoke vs Rust generate_handler", () => {
	it("keeps TS invoke command names present in Rust", () => {
		const tsSource = fs.readFileSync(tsTauriPath, "utf8");
		const rustSource = fs.readFileSync(rustLibPath, "utf8");

		const tsCommands = extractInvokeCommandNames(tsSource);
		const rustCommands = new Set(extractRustCommandNames(rustSource));

		const missingInRust = tsCommands.filter(
			(command) => !rustCommands.has(command),
		);

		expect(
			missingInRust,
			`TS invokes commands not registered in Rust: ${missingInRust.join(", ")}`,
		).toEqual([]);
	});
});
