import { describe, expect, it } from "vitest";

import { formatErrorMessage } from "./formatError";

describe("formatErrorMessage", () => {
	it("returns a default message for null/undefined", () => {
		expect(formatErrorMessage(null)).toBe("Unknown error");
		expect(formatErrorMessage(undefined)).toBe("Unknown error");
	});

	it("returns strings unchanged", () => {
		expect(formatErrorMessage("hello")).toBe("hello");
	});

	it("stringifies primitives", () => {
		expect(formatErrorMessage(123)).toBe("123");
		expect(formatErrorMessage(true)).toBe("true");
		expect(formatErrorMessage(123n)).toBe("123");
	});

	it("uses Error.message when available", () => {
		expect(formatErrorMessage(new Error("boom"))).toBe("boom");
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

	it("falls back to String(object) for empty objects", () => {
		expect(formatErrorMessage({})).toBe("[object Object]");
	});
});
