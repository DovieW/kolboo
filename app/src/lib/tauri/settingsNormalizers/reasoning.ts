import type { OpenAiReasoningEffort } from "../types";

export function normalizeOpenAiReasoningEffort(
	value: unknown,
): OpenAiReasoningEffort | null {
	if (typeof value !== "string") return null;
	const v = value.trim().toLowerCase();
	if (
		v === "none" ||
		v === "minimal" ||
		v === "low" ||
		v === "medium" ||
		v === "high" ||
		v === "xhigh"
	) {
		return v as OpenAiReasoningEffort;
	}
	return null;
}

export function normalizeGeminiThinkingLevel(
	value: unknown,
): "minimal" | "low" | "medium" | "high" | null {
	if (typeof value !== "string") return null;
	const v = value.trim().toLowerCase();
	if (v === "minimal" || v === "low" || v === "medium" || v === "high") {
		return v;
	}
	return null;
}

export function normalizeGeminiThinkingBudget(value: unknown): number | null {
	if (value == null) return null;
	if (typeof value !== "number" || !Number.isFinite(value)) return null;
	// Keep it integer-ish (Gemini expects an integer token budget).
	return Math.trunc(value);
}

export function normalizeAnthropicThinkingBudget(
	value: unknown,
): number | null {
	if (value == null) return null;
	if (typeof value !== "number" || !Number.isFinite(value)) return null;
	// Keep it integer-ish; Anthropic expects an integer token budget.
	const n = Math.trunc(value);
	// The cookbook notes a minimum budget of 1024 for extended thinking.
	if (n < 1024) return 1024;
	// Defensive cap; actual max varies by model.
	return Math.min(32768, n);
}

export function normalizeAnthropicThinkingBudgetAllowOff(
	value: unknown,
): number | null {
	// For per-profile overrides we want an explicit "off" state even if the
	// Default/global setting enables thinking. Represent that as 0.
	if (value == null) return null;
	if (typeof value !== "number" || !Number.isFinite(value)) return null;
	const n = Math.trunc(value);
	if (n <= 0) return 0;
	if (n < 1024) return 1024;
	return Math.min(32768, n);
}
