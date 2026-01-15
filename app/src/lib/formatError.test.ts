import { describe, expect, it } from "vitest";

import { formatErrorMessage } from "./formatError";

describe("formatErrorMessage", () => {
	it.each([
		["null", null, "Unknown error"],
		["undefined", undefined, "Unknown error"],
		["string", "hello", "hello"],
		["unicode string", "snowman ☃", "snowman ☃"],
		["number", 123, "123"],
		["boolean", true, "true"],
		["bigint", 123n, "123"],
		["plain object", { status: 500 }, "{\"status\":500}"],
	])("formats %s", (_label, input, expected) => {
		expect(formatErrorMessage(input)).toBe(expected);
	});

	it("uses Error.message when available", () => {
		expect(formatErrorMessage(new Error("boom"))).toBe("boom");
	});

	it("handles Error with cause", () => {
		const error = new Error("top-level", { cause: new Error("root") });
		expect(formatErrorMessage(error)).toBe("top-level");
	});

	it("falls back to Error.toString when message is empty", () => {
		expect(formatErrorMessage(new Error(""))).toBe("Error");
	});

	it("reads string message field from plain objects", () => {
		expect(formatErrorMessage({ message: " hi " })).toBe(" hi ");
	});

	it("reads string error field from plain objects", () => {
		expect(formatErrorMessage({ error: "bad" })).toBe("bad");
	});

	it("stringifies objects, including circular references", () => {
		const obj: { self?: unknown } = {};
		obj.self = obj;
		expect(formatErrorMessage(obj)).toBe("{\"self\":\"[Circular]\"}");
	});

	it("avoids stringifying objects with sensitive keys", () => {
		const output = formatErrorMessage({ apiKey: "secret" });
		expect(output).toBe("[object Object]");
		expect(output).not.toContain("secret");
		expect(output).not.toContain("apiKey");
	});

	it("falls back to String(object) for empty objects", () => {
		expect(formatErrorMessage({})).toBe("[object Object]");
	});
});
