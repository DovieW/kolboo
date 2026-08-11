import { describe, expect, it, vi } from "vitest";
import {
	byokModelsWithLiveCatalog,
	fetchModelsDevByokLlmCatalog,
	MODELS_DEV_API_URL,
	parseModelsDevByokLlmCatalog,
} from "./modelsDev";

function fixtureCatalog() {
	return {
		openai: {
			models: {
				"gpt-current": {
					id: "gpt-current",
					name: "GPT Current",
					modalities: { input: ["text"], output: ["text"] },
					cost: { input: 0.4, output: 1.6 },
				},
				"gpt-image": {
					id: "gpt-image",
					name: "GPT Image",
					modalities: { input: ["text"], output: ["image"] },
				},
				"gpt-retired": {
					id: "gpt-retired",
					name: "GPT Retired",
					status: "deprecated",
					modalities: { input: ["text"], output: ["text"] },
				},
			},
		},
		google: {
			models: {
				"gemini-current": {
					id: "gemini-current",
					name: "Gemini Current",
					modalities: { input: ["text"], output: ["text"] },
				},
			},
		},
	};
}

describe("models.dev BYOK catalog", () => {
	it("maps supported providers and text-generation models with pricing", () => {
		expect(parseModelsDevByokLlmCatalog(fixtureCatalog())).toEqual({
			openai: [
				{
					value: "gpt-current",
					label: "GPT Current",
					pricing: {
						input_per_million: 0.4,
						output_per_million: 1.6,
						cache_read_per_million: undefined,
						source: "models.dev",
					},
				},
			],
			gemini: [
				{
					value: "gemini-current",
					label: "Gemini Current",
					pricing: {
						input_per_million: undefined,
						output_per_million: undefined,
						cache_read_per_million: undefined,
						source: "models.dev",
					},
				},
			],
		});
	});

	it("replaces live provider lists but keeps bundled unsupported providers", () => {
		const bundled = {
			openai: [{ value: "gpt-bundled", label: "GPT Bundled" }],
			ollama: [],
		};
		const live = {
			openai: [{ value: "gpt-live", label: "GPT Live" }],
		};
		expect(byokModelsWithLiveCatalog(bundled, live)).toEqual({
			openai: live.openai,
			ollama: [],
		});
	});

	it("caches the compact supported catalog and reuses it within one day", async () => {
		const values = new Map<string, string>();
		const storage = {
			getItem: (key: string) => values.get(key) ?? null,
			setItem: (key: string, value: string) => values.set(key, value),
		};
		const fetchImpl = vi.fn(async (url: string | URL | Request) => {
			expect(String(url)).toBe(MODELS_DEV_API_URL);
			return new Response(JSON.stringify(fixtureCatalog()), {
				status: 200,
				headers: { "content-type": "application/json" },
			});
		});

		const first = await fetchModelsDevByokLlmCatalog({
			fetchImpl,
			storage,
			now: 1_000,
		});
		const second = await fetchModelsDevByokLlmCatalog({
			fetchImpl,
			storage,
			now: 2_000,
		});

		expect(second).toEqual(first);
		expect(fetchImpl).toHaveBeenCalledTimes(1);
	});
});
