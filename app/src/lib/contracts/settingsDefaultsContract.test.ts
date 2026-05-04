import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS_VALUES } from "../tauri/settingsDefaults";

const rustDefaultsPath = path.resolve(
	process.cwd(),
	"src-tauri/src/settings/default_values.rs",
);

function readRustDefaultConst(name: string): string {
	const source = readFileSync(rustDefaultsPath, "utf8");
	const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
	const match = source.match(
		new RegExp(`pub const ${escaped}: [^=]+ = (?<value>[^;]+);`),
	);
	if (!match?.groups?.value) {
		throw new Error(`Missing Rust default const ${name}`);
	}
	return match.groups.value.trim();
}

function parseRustStringConst(name: string): string {
	const value = readRustDefaultConst(name);
	const match = value.match(/^"(?<inner>.*)"$/);
	if (!match?.groups?.inner) {
		throw new Error(`Rust default const ${name} is not a string literal`);
	}
	return match.groups.inner;
}

function parseRustNumberConst(name: string): number {
	const value = readRustDefaultConst(name).replace(/_u?(?:32|64|size)?$/, "");
	return Number(value.replaceAll("_", ""));
}

function parseRustBooleanConst(name: string): boolean {
	const value = readRustDefaultConst(name);
	if (value === "true") return true;
	if (value === "false") return false;
	throw new Error(`Rust default const ${name} is not a boolean literal`);
}

describe("settings defaults cross-layer contract", () => {
	it("keeps shared string defaults aligned between TypeScript and Rust", () => {
		expect(parseRustStringConst("DEFAULT_STT_LANGUAGE")).toBe(
			DEFAULT_SETTINGS_VALUES.stt_language,
		);
		expect(parseRustStringConst("DEFAULT_OVERLAY_MODE")).toBe(
			DEFAULT_SETTINGS_VALUES.overlay_mode,
		);
		expect(parseRustStringConst("DEFAULT_OUTPUT_MODE")).toBe(
			DEFAULT_SETTINGS_VALUES.output_mode,
		);
		expect(parseRustStringConst("DEFAULT_OCR_AUTH_MODE")).toBe(
			DEFAULT_SETTINGS_VALUES.ocr_auth_mode,
		);
		expect(parseRustStringConst("DEFAULT_OCR_RESIZE_FILTER")).toBe(
			DEFAULT_SETTINGS_VALUES.ocr_resize_filter,
		);
	});

	it("keeps shared numeric and boolean defaults aligned between TypeScript and Rust", () => {
		expect(parseRustNumberConst("DEFAULT_OCR_REQUEST_TIMEOUT_MS")).toBe(
			DEFAULT_SETTINGS_VALUES.ocr_request_timeout_ms,
		);
		expect(parseRustNumberConst("DEFAULT_OCR_CONTEXT_MAX_CHARS")).toBe(
			DEFAULT_SETTINGS_VALUES.ocr_context_max_chars,
		);
		expect(parseRustNumberConst("DEFAULT_REQUEST_LOGS_RETENTION_AMOUNT")).toBe(
			DEFAULT_SETTINGS_VALUES.request_logs_retention_amount,
		);
		expect(parseRustNumberConst("DEFAULT_STATS_RETENTION_MAX_BYTES")).toBe(
			DEFAULT_SETTINGS_VALUES.stats_retention_max_bytes,
		);
		expect(parseRustBooleanConst("DEFAULT_SOUND_ENABLED")).toBe(
			DEFAULT_SETTINGS_VALUES.sound_enabled,
		);
		expect(parseRustBooleanConst("DEFAULT_REWRITE_LLM_ENABLED")).toBe(
			DEFAULT_SETTINGS_VALUES.rewrite_llm_enabled,
		);
	});
});
