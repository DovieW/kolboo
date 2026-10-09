import { describe, expect, it } from "vitest";
import {
	DEFAULT_STT_LANGUAGE,
	normalizeSttLanguage,
	normalizeSttLanguageOverride,
	STT_LANGUAGE_OPTIONS,
	STT_LANGUAGE_VALUES,
} from "./sttLanguages";

describe("language hints", () => {
	it("defaults missing or invalid hints to auto without changing explicit hints", () => {
		expect(DEFAULT_STT_LANGUAGE).toBe("auto");
		for (const input of [undefined, null, 2, "", "not-a-language"])
			expect(normalizeSttLanguage(input)).toBe("auto");
		for (const language of STT_LANGUAGE_VALUES)
			expect(normalizeSttLanguage(` ${language.toUpperCase()} `)).toBe(
				language,
			);
		expect(normalizeSttLanguage(undefined, "en")).toBe("en");
		expect(normalizeSttLanguage("invalid", "he")).toBe("he");
		expect(STT_LANGUAGE_OPTIONS.map(({ value }) => value)).toEqual([
			...STT_LANGUAGE_VALUES,
		]);
		expect(STT_LANGUAGE_OPTIONS).toContainEqual({
			value: "yi",
			label: "Yiddish",
		});
		expect(STT_LANGUAGE_OPTIONS).toContainEqual({
			value: "he",
			label: "Hebrew",
		});
	});

	it("keeps inheritance distinct from an explicit auto-detect override", () => {
		for (const input of [undefined, null, 1, "", " ", "unknown"])
			expect(normalizeSttLanguageOverride(input)).toBeNull();
		for (const language of STT_LANGUAGE_VALUES)
			expect(normalizeSttLanguageOverride(` ${language.toUpperCase()} `)).toBe(
				language,
			);
	});
});
