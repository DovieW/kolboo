import type {
	ActiveWindowOcrMode,
	OcrAuthMode,
	OcrAutoCaptureTiming,
	OcrResizeFilter,
} from "../types";

export function normalizeActiveWindowOcrMode(
	value: unknown,
): ActiveWindowOcrMode {
	if (value === "off" || value === "auto" || value === "manual") return value;
	return "off";
}

export function normalizeActiveWindowOcrModeOverride(
	value: unknown,
): ActiveWindowOcrMode | null {
	if (value === "off" || value === "auto" || value === "manual") return value;
	return null;
}

export function normalizeOcrAuthMode(value: unknown): OcrAuthMode {
	if (value === "none" || value === "bearer_api_key") return value;
	return "none";
}

export function normalizeOcrAutoCaptureTiming(
	value: unknown,
): OcrAutoCaptureTiming {
	if (value === "on_stop" || value === "on_start") return value;
	return "on_start";
}

export function normalizeOcrResizeFilter(value: unknown): OcrResizeFilter {
	if (
		value === "nearest" ||
		value === "triangle" ||
		value === "catmullrom" ||
		value === "lanczos3"
	)
		return value;
	return "nearest";
}
