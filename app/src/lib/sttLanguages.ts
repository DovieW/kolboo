export const STT_LANGUAGE_VALUES = [
	"auto",
	"en",
	"es",
	"fr",
	"de",
	"it",
	"pt",
	"zh",
	"ja",
	"ko",
	"hi",
	"ar",
	"ru",
	"yi",
	"he",
	"lk",
] as const;

export type SttLanguageValue = (typeof STT_LANGUAGE_VALUES)[number];

export const STT_LANGUAGE_OPTIONS: Array<{
	value: SttLanguageValue;
	label: string;
}> = [
	{ value: "auto", label: "Auto-detect" },
	{ value: "en", label: "English" },
	{ value: "es", label: "Spanish" },
	{ value: "fr", label: "French" },
	{ value: "de", label: "German" },
	{ value: "it", label: "Italian" },
	{ value: "pt", label: "Portuguese" },
	{ value: "zh", label: "Chinese" },
	{ value: "ja", label: "Japanese" },
	{ value: "ko", label: "Korean" },
	{ value: "hi", label: "Hindi" },
	{ value: "ar", label: "Arabic" },
	{ value: "ru", label: "Russian" },
	{ value: "yi", label: "Yiddish" },
	{ value: "he", label: "Hebrew" },
	{ value: "lk", label: "Lashon Kodesh (YiddishLabs Standard)" },
];

/** Hints are protocol capabilities, not duplicate language-specific models.
 * Preserve an existing unsupported hint visibly; never silently turn it into Auto. */
export function sttLanguageOptionsForModel(
	provider?: string | null,
	model?: string | null,
	saved?: string | null,
): Array<{ value: string; label: string; disabled?: boolean }> {
	const standard =
		provider === "yiddishlabs" && model === "yiddishlabs-standard";
	const supported = STT_LANGUAGE_OPTIONS.filter((option) =>
		provider === "yiddishlabs"
			? ["auto", "yi", "en", "he", ...(standard ? ["lk"] : [])].includes(
					option.value,
				)
			: option.value !== "lk",
	).map((option) =>
		provider === "speechmatics" && option.value === "auto"
			? { ...option, label: "Auto-detect unavailable — choose a language", disabled: true }
			: option,
	);
	if (saved && !supported.some((option) => option.value === saved)) {
		const label =
			STT_LANGUAGE_OPTIONS.find((option) => option.value === saved)?.label ??
			saved;
		return [
			...supported,
			{
				value: saved,
				label: `${label} (not supported by this model)`,
				disabled: true,
			},
		];
	}
	return supported;
}

export const DEFAULT_STT_LANGUAGE: SttLanguageValue = "auto";

export function normalizeSttLanguage(
	value: unknown,
	fallback: SttLanguageValue = DEFAULT_STT_LANGUAGE,
): SttLanguageValue {
	if (typeof value !== "string") return fallback;
	const trimmed = value.trim().toLowerCase();
	const match = STT_LANGUAGE_VALUES.find((option) => option === trimmed);
	return match ?? fallback;
}

export function normalizeSttLanguageOverride(
	value: unknown,
): SttLanguageValue | null {
	if (value == null) return null;
	if (typeof value !== "string") return null;
	const trimmed = value.trim().toLowerCase();
	if (!trimmed) return null;
	const match = STT_LANGUAGE_VALUES.find((option) => option === trimmed);
	return match ?? null;
}
