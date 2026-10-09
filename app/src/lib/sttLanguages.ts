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
];

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
