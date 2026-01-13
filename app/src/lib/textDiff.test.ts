import { describe, expect, it } from "vitest";

import { diffTextInline, isDiffTrivial } from "./textDiff";

describe("textDiff", () => {
	it("treats identical text as trivial", () => {
		const chunks = diffTextInline("hello world", "hello world");
		expect(isDiffTrivial(chunks)).toBe(true);
	});

	it("shows punctuation insertions as an added chunk", () => {
		const chunks = diffTextInline("hello world", "hello, world");
		const addedValues = chunks.filter((c) => c.added).map((c) => c.value);
		expect(addedValues).toContain(",");
	});

	it("normalizes CRLF vs LF so it doesn't create a diff", () => {
		const chunks = diffTextInline("a\r\nb", "a\nb");
		expect(isDiffTrivial(chunks)).toBe(true);
	});
});
