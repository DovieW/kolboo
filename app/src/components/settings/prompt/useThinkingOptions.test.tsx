import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
	openAiThinkingEffortsForModel,
	type UseThinkingOptionsParams,
	useThinkingOptions,
} from "./useThinkingOptions";

function renderHook<T>(hook: () => T) {
	const result = {} as { current: T };
	function Probe() {
		result.current = hook();
		return null;
	}
	renderToStaticMarkup(<Probe />);
	return { result, unmount: () => undefined };
}

const defaults: UseThinkingOptionsParams = {
	effectiveLlmProvider: null,
	effectiveLlmModel: null,
	effectiveQuickAskProvider: null,
	effectiveQuickAskModel: null,
	selectedQuickAskModelForUi: null,
	isDefaultScope: true,
	defaultAnthropicThinkingBudget: null,
	localProfileAnthropicThinkingBudget: "default",
	defaultQuickAskAnthropicThinkingBudget: null,
	localProfileQuickAskAnthropicThinkingBudget: "default",
};

describe("current provider thinking capabilities", () => {
	it("offers only supported OpenAI efforts in rewrite and Quick Ask", () => {
		for (const model of [
			"gpt-6-astra",
			"gpt-6.1-sol",
			"gpt-6-luna",
			"gpt-6-sol",
			"gpt-5",
			"o3",
			"gpt-4o-mini",
		]) {
			const { result, unmount } = renderHook(() =>
				useThinkingOptions({
					...defaults,
					effectiveLlmProvider: "openai",
					effectiveLlmModel: model,
					effectiveQuickAskProvider: "openai",
					effectiveQuickAskModel: model,
				}),
			);
			const efforts = openAiThinkingEffortsForModel(model);
			expect(result.current.supportsOpenAiThinking).toBe(efforts.length > 0);
			expect(result.current.supportsQuickAskOpenAiThinking).toBe(
				efforts.length > 0,
			);
			expect(result.current.openAiThinkingOptions.map((v) => v.value)).toEqual(
				efforts.length ? ["default", ...efforts] : [],
			);
			expect(result.current.quickAskOpenAiThinkingOptions).toEqual(
				result.current.openAiThinkingOptions,
			);
			if (model === "gpt-6-luna" || model === "gpt-6-sol")
				expect(efforts).toContain("none");
			if (model === "gpt-6-astra" || model === "gpt-6.1-sol") {
				expect(efforts).toEqual(["low", "medium", "high", "xhigh", "max"]);
			}
			unmount();
		}
	});

	it("does not offer minimal on new Flash or omit medium on 3.1 Pro", () => {
		for (const [model, levels] of [
			["gemini-3.8-flash", ["low", "medium", "high"]],
			["gemini-3.7-flash", ["low", "medium", "high"]],
			["gemini-3.1-pro-preview", ["low", "medium", "high"]],
			["gemini-3.6-flash", ["minimal", "low", "medium", "high"]],
			["gemini-3.5-flash-lite", ["minimal", "low", "medium", "high"]],
			["models/gemini-3-flash-preview", ["minimal", "low", "medium", "high"]],
			["models/gemini-3-pro-preview", ["low", "high"]],
		] as const) {
			const { result, unmount } = renderHook(() =>
				useThinkingOptions({
					...defaults,
					effectiveLlmProvider: "gemini",
					effectiveLlmModel: model,
					effectiveQuickAskProvider: "gemini",
					effectiveQuickAskModel: model,
				}),
			);
			expect(
				result.current.geminiThinkingLevelOptions.map((v) => v.value),
			).toEqual(["default", ...levels]);
			expect(result.current.quickAskGeminiThinkingLevelOptions).toEqual(
				result.current.geminiThinkingLevelOptions,
			);
			expect(result.current.isGemini3Flash).toBe(model.includes("-flash"));
			expect(result.current.isGemini3Pro).toBe(model.includes("-pro"));
			expect(result.current.isQuickAskGemini3Flash).toBe(
				model.includes("-flash"),
			);
			unmount();
		}
	});

	it("does not advertise obsolete token budgets for adaptive Claude models", () => {
		const { result } = renderHook(() =>
			useThinkingOptions({
				...defaults,
				effectiveLlmProvider: "anthropic",
				effectiveLlmModel: "claude-haiku-5-5",
				effectiveQuickAskProvider: "anthropic",
				effectiveQuickAskModel: "claude-opus-5-5",
			}),
		);
		expect(result.current.supportsAnthropicThinkingBudget).toBe(false);
		expect(result.current.supportsQuickAskAnthropicThinkingBudget).toBe(false);
		expect(result.current.supportsOpenAiThinking).toBe(false);
		expect(result.current.supportsQuickAskOpenAiThinking).toBe(false);
	});
});
