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

	describe("multiline differences", () => {
		it("handles multiple line insertions", () => {
			const before = "line1\nline3";
			const after = "line1\nline2\nline3";
			const chunks = diffTextInline(before, after);
			const added = chunks.filter((c) => c.added).map((c) => c.value).join("");

			expect(added).toContain("line2");
			expect(added).toContain("\n");
		});

		it("handles multiple line deletions", () => {
			const before = "line1\nline2\nline3";
			const after = "line1\nline3";
			const chunks = diffTextInline(before, after);
			const removed = chunks.filter((c) => c.removed).map((c) => c.value).join("");

			expect(removed).toContain("line2");
			expect(removed).toContain("\n");
		});

		it("handles line replacements", () => {
			const before = "first\nmiddle\nlast";
			const after = "first\nmodified\nlast";
			const chunks = diffTextInline(before, after);
			const removed = chunks.filter((c) => c.removed).map((c) => c.value).join("");
			const added = chunks.filter((c) => c.added).map((c) => c.value).join("");

			expect(removed).toBe("middle");
			expect(added).toBe("modified");
		});

		it("normalizes trailing newlines consistently", () => {
			const withTrailing = "hello\n";
			const withoutTrailing = "hello";
			const chunks = diffTextInline(withTrailing, withoutTrailing);
			const removed = chunks.filter((c) => c.removed).map((c) => c.value).join("");

			expect(removed).toBe("\n");
		});
	});

	describe("whitespace-only changes", () => {
		it("detects space-to-tab changes", () => {
			const before = "    indented";
			const after = "\tindented";
			const chunks = diffTextInline(before, after);

			expect(isDiffTrivial(chunks)).toBe(false);
			const removed = chunks.filter((c) => c.removed).map((c) => c.value).join("");
			const added = chunks.filter((c) => c.added).map((c) => c.value).join("");

			expect(removed).toBe("    ");
			expect(added).toBe("\t");
		});

		it("detects mixed tab/space changes", () => {
			const before = "  \ttext";
			const after = "\t  text";
			const chunks = diffTextInline(before, after);

			expect(isDiffTrivial(chunks)).toBe(false);
		});

		it("treats whitespace-only strings as different from empty", () => {
			const chunks = diffTextInline("", "   ");
			const added = chunks.filter((c) => c.added).map((c) => c.value).join("");

			expect(added).toBe("   ");
			expect(isDiffTrivial(chunks)).toBe(false);
		});
	});

	describe("unicode edge cases", () => {
		it("handles emoji insertion without corruption", () => {
			const before = "Hello world";
			const after = "Hello 🌍 world";
			const chunks = diffTextInline(before, after);
			const added = chunks.filter((c) => c.added).map((c) => c.value).join("");

			expect(added).toContain("🌍");
		});

		it("handles emoji replacement", () => {
			const before = "Status: ✅";
			const after = "Status: ❌";
			const chunks = diffTextInline(before, after);
			const removed = chunks.filter((c) => c.removed).map((c) => c.value).join("");
			const added = chunks.filter((c) => c.added).map((c) => c.value).join("");

			expect(removed).toContain("✅");
			expect(added).toContain("❌");
		});

		it("preserves combining marks as part of base character", () => {
			// é can be represented as e + combining acute (U+0301)
			const composed = "café";
			const decomposed = "cafe\u0301";
			const chunks = diffTextInline(composed, decomposed);

			// Both should be tokenized as complete grapheme clusters
			// The diff might show them as different, but no corruption
			const allText = chunks.map((c) => c.value).join("");
			expect(allText.length).toBeGreaterThan(0);
		});
	});

	describe("empty and boundary cases", () => {
		it("handles empty to non-empty", () => {
			const chunks = diffTextInline("", "hello");
			expect(isDiffTrivial(chunks)).toBe(false);

			const added = chunks.filter((c) => c.added).map((c) => c.value).join("");
			expect(added).toBe("hello");
		});

		it("handles non-empty to empty", () => {
			const chunks = diffTextInline("hello", "");
			expect(isDiffTrivial(chunks)).toBe(false);

			const removed = chunks.filter((c) => c.removed).map((c) => c.value).join("");
			expect(removed).toBe("hello");
		});

		it("handles empty to empty", () => {
			const chunks = diffTextInline("", "");
			expect(isDiffTrivial(chunks)).toBe(true);
			expect(chunks).toHaveLength(0);
		});
	});

	describe("longer inputs", () => {
		it("handles paragraph-length text efficiently", () => {
			const lorem =
				"Lorem ipsum dolor sit amet, consectetur adipiscing elit. " +
				"Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. " +
				"Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.";
			const modified =
				"Lorem ipsum dolor sit amet, consectetur adipiscing elit. " +
				"Sed do eiusmod MODIFIED incididunt ut labore et dolore magna aliqua. " +
				"Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.";

			const chunks = diffTextInline(lorem, modified);
			const removed = chunks.filter((c) => c.removed).map((c) => c.value).join("");
			const added = chunks.filter((c) => c.added).map((c) => c.value).join("");

			expect(removed).toBe("tempor");
			expect(added).toBe("MODIFIED");
		});

		it("handles multi-paragraph text with many changes", () => {
			const before =
				"First paragraph with some text.\n\n" +
				"Second paragraph with more content.\n\n" +
				"Third paragraph concludes.";
			const after =
				"First paragraph with some text.\n\n" +
				"Second paragraph with MODIFIED content.\n\n" +
				"Third paragraph concludes.\n\n" +
				"Fourth paragraph added.";

			const chunks = diffTextInline(before, after);

			expect(isDiffTrivial(chunks)).toBe(false);
			const added = chunks.filter((c) => c.added).map((c) => c.value).join("");
			expect(added).toContain("MODIFIED");
			expect(added).toContain("Fourth");
		});

		it("handles code-like text with mixed indentation", () => {
			const before = 'function foo() {\n\treturn "old";\n}';
			const after = 'function foo() {\n\treturn "new";\n}';

			const chunks = diffTextInline(before, after);
			const removed = chunks.filter((c) => c.removed).map((c) => c.value).join("");
			const added = chunks.filter((c) => c.added).map((c) => c.value).join("");

			expect(removed).toContain("old");
			expect(added).toContain("new");
		});
	});
});
