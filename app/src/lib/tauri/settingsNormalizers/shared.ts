export const isRecord = (value: unknown): value is Record<string, unknown> => {
	return value != null && typeof value === "object" && !Array.isArray(value);
};

export function normalizeBooleanSetting(value: unknown): boolean | null {
	return typeof value === "boolean" ? value : null;
}

export function normalizeNonEmptyStringSetting(value: unknown): string | null {
	if (typeof value !== "string") return null;
	const trimmed = value.trim();
	return trimmed.length > 0 ? trimmed : null;
}
