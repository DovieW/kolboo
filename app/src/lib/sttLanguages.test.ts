import { describe, expect, it } from "vitest";
import {
	DEFAULT_STT_LANGUAGE,
	normalizeSttLanguage,
	normalizeSttLanguageOverride,
	STT_LANGUAGE_OPTIONS,
	STT_LANGUAGE_VALUES,
	sttLanguageOptionsForModel,
} from "./sttLanguages";

describe("language hints", () => {
	it("explains Speechmatics Auto limitations without changing an inherited or saved hint", () => {
		for (const saved of [undefined, "auto", "lk"]) {
			expect(sttLanguageOptionsForModel("speechmatics", "enhanced", saved))
				.toContainEqual({ value: "auto", label: "Auto-detect unavailable — choose a language", disabled: true });
		}
		expect(sttLanguageOptionsForModel("speechmatics", "standard", "he"))
			.toContainEqual({ value: "he", label: "Hebrew" });
	});
	it("offers only documented YiddishLabs hints without discarding an existing incompatible hint", () => {
		expect(
			sttLanguageOptionsForModel("yiddishlabs", "yiddishlabs-3-turbo").map(
				(option) => option.value,
			),
		).toEqual(["auto", "en", "yi", "he"]);
		expect(
			sttLanguageOptionsForModel("yiddishlabs", "yiddishlabs-standard").map(
				(option) => option.value,
			),
		).toEqual(["auto", "en", "yi", "he", "lk"]);
		expect(
			sttLanguageOptionsForModel().some((option) => option.value === "lk"),
		).toBe(false);
		expect(
			sttLanguageOptionsForModel("openai", null, "lk").at(-1),
		).toMatchObject({ value: "lk", disabled: true });
		expect(
			sttLanguageOptionsForModel("yiddishlabs", null, "es").at(-1),
		).toEqual({
			value: "es",
			label: "Spanish (not supported by this model)",
			disabled: true,
		});
		expect(
			sttLanguageOptionsForModel("yiddishlabs", null, "unknown").at(-1),
		).toEqual({
			value: "unknown",
			label: "unknown (not supported by this model)",
			disabled: true,
		});
		expect(sttLanguageOptionsForModel("yiddishlabs", null, "yi")).toHaveLength(
			4,
		);
	});
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
