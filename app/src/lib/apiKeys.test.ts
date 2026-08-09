import { describe, expect, it } from "vitest";

import { resolveApiKeyMutationIntent } from "./apiKeys";

describe("api key mutation planning", () => {
	it("clears a stored key when the draft is blank", () => {
		expect(
			resolveApiKeyMutationIntent({
				draftValue: "   ",
				savedValue: "existing-secret",
			}),
		).toEqual({ kind: "clear" });
	});

	it("does nothing when both the draft and stored value are blank", () => {
		expect(
			resolveApiKeyMutationIntent({
				draftValue: "   ",
				savedValue: null,
			}),
		).toBeNull();
	});

	it("does nothing when the trimmed draft matches the stored key", () => {
		expect(
			resolveApiKeyMutationIntent({
				draftValue: "  existing-secret  ",
				savedValue: "existing-secret",
			}),
		).toBeNull();
	});

	it("saves a trimmed key when the value changes", () => {
		expect(
			resolveApiKeyMutationIntent({
				draftValue: "  new-secret  ",
				savedValue: "existing-secret",
			}),
		).toEqual({ kind: "save", value: "new-secret" });
	});
});
