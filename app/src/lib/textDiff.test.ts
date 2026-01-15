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

	it("handles curly apostrophes without tokenization explosions", () => {
		const chunks = diffTextInline("I don’t know", "I don't know");
		const removed = chunks.filter((c) => c.removed).map((c) => c.value).join("");
		const added = chunks.filter((c) => c.added).map((c) => c.value).join("");

		expect(removed).toBe("don’t");
		expect(added).toBe("don't");
	});

	it("preserves whitespace as tokens", () => {
		const chunks = diffTextInline("hello  world", "hello world");
		const whitespaceEdits = chunks.filter(
			(c) => (c.added || c.removed) && c.value.trim().length === 0,
		);
		expect(whitespaceEdits.length).toBeGreaterThan(0);
	});

	it("shows newline insertions as an added chunk", () => {
		const chunks = diffTextInline("a\nb", "a\n\nb");
		const addedValues = chunks.filter((c) => c.added).map((c) => c.value);
		expect(addedValues.join("")).toContain("\n");
	});

	it("doesn't split accented words into weird tokens", () => {
		const chunks = diffTextInline("I like café", "I like cafe");
		const removed = chunks.filter((c) => c.removed).map((c) => c.value).join("");
		const added = chunks.filter((c) => c.added).map((c) => c.value).join("");

		expect(removed).toBe("café");
		expect(added).toBe("cafe");
	});
});
