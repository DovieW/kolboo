import type { ModelOption } from "./modelOptions";

export const MODELS_DEV_API_URL = "https://models.dev/api.json";
export const MODELS_DEV_CACHE_TTL_MS = 24 * 60 * 60 * 1000;
const MODELS_DEV_CACHE_KEY = "kolboo.models-dev.byok-llm.v1";

export type ByokLlmModelCatalog = Record<string, ModelOption[]>;

type StorageLike = Pick<Storage, "getItem" | "setItem">;

type CachedCatalog = {
	stored_at: number;
	models: ByokLlmModelCatalog;
};

type ModelsDevModel = {
	id?: unknown;
	name?: unknown;
	status?: unknown;
	modalities?: {
		input?: unknown;
		output?: unknown;
	};
	cost?: {
		input?: unknown;
		output?: unknown;
		cache_read?: unknown;
	};
};

const providerIds = {
	anthropic: "anthropic",
	cerebras: "cerebras",
	cohere: "cohere",
	fireworks: "fireworks-ai",
	gemini: "google",
	groq: "groq",
	openai: "openai",
} as const;

function isRecord(value: unknown): value is Record<string, unknown> {
	return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function safeStorage(): StorageLike | undefined {
	try {
		return typeof localStorage === "undefined" ? undefined : localStorage;
	} catch {
		return undefined;
	}
}

function readCache(storage: StorageLike | undefined): CachedCatalog | null {
	if (!storage) return null;
	try {
		const raw = storage.getItem(MODELS_DEV_CACHE_KEY);
		if (!raw) return null;
		const parsed = JSON.parse(raw) as CachedCatalog;
		if (
			!Number.isFinite(parsed.stored_at) ||
			!isRecord(parsed.models) ||
			Object.values(parsed.models).some((models) => !Array.isArray(models))
		) {
			return null;
		}
		return parsed;
	} catch {
		return null;
	}
}

function writeCache(
	storage: StorageLike | undefined,
	value: CachedCatalog,
): void {
	if (!storage) return;
	try {
		storage.setItem(MODELS_DEV_CACHE_KEY, JSON.stringify(value));
	} catch {
		// Catalog caching is an optimization; model selection still uses memory or
		// the bundled list when storage is unavailable.
	}
}

function stringArrayIncludes(value: unknown, expected: string): boolean {
	return Array.isArray(value) && value.includes(expected);
}

function optionalPrice(value: unknown): number | undefined {
	return typeof value === "number" && Number.isFinite(value) && value >= 0
		? value
		: undefined;
}

function parseProviderModels(value: unknown): ModelOption[] {
	if (!isRecord(value) || !isRecord(value.models)) return [];

	const models: ModelOption[] = [];
	for (const rawModel of Object.values(value.models).slice(0, 500)) {
		if (!isRecord(rawModel)) continue;
		const model = rawModel as ModelsDevModel;
		if (
			typeof model.id !== "string" ||
			model.id.trim().length === 0 ||
			model.id.length > 200 ||
			model.status === "deprecated" ||
			!stringArrayIncludes(model.modalities?.input, "text") ||
			!stringArrayIncludes(model.modalities?.output, "text")
		) {
			continue;
		}

		const displayName =
			typeof model.name === "string" && model.name.trim().length > 0
				? model.name.trim()
				: model.id.trim();
		models.push({
			value: model.id.trim(),
			label: displayName.slice(0, 200),
			pricing: {
				input_per_million: optionalPrice(model.cost?.input),
				output_per_million: optionalPrice(model.cost?.output),
				cache_read_per_million: optionalPrice(model.cost?.cache_read),
				source: "models.dev",
			},
		});
	}

	return models.sort((a, b) => a.label.localeCompare(b.label));
}

export function parseModelsDevByokLlmCatalog(
	value: unknown,
): ByokLlmModelCatalog {
	if (!isRecord(value)) {
		throw new Error("models.dev returned an invalid catalog");
	}

	const catalog: ByokLlmModelCatalog = {};
	for (const [kolbooProvider, modelsDevProvider] of Object.entries(
		providerIds,
	)) {
		const models = parseProviderModels(value[modelsDevProvider]);
		if (models.length > 0) catalog[kolbooProvider] = models;
	}
	if (Object.keys(catalog).length === 0) {
		throw new Error("models.dev returned no supported BYOK models");
	}
	return catalog;
}

export function byokModelsWithLiveCatalog(
	bundled: ByokLlmModelCatalog,
	live: ByokLlmModelCatalog | null | undefined,
): ByokLlmModelCatalog {
	if (!live) return bundled;
	return Object.fromEntries(
		Object.entries(bundled).map(([provider, models]) => [
			provider,
			live[provider]?.length ? live[provider] : models,
		]),
	);
}

export async function fetchModelsDevByokLlmCatalog(options?: {
	fetchImpl?: typeof fetch;
	storage?: StorageLike;
	now?: number;
}): Promise<ByokLlmModelCatalog> {
	const fetchImpl = options?.fetchImpl ?? fetch;
	const storage = options?.storage ?? safeStorage();
	const now = options?.now ?? Date.now();
	const cached = readCache(storage);
	if (cached && now - cached.stored_at < MODELS_DEV_CACHE_TTL_MS) {
		return cached.models;
	}

	try {
		const response = await fetchImpl(MODELS_DEV_API_URL, {
			headers: { accept: "application/json" },
		});
		if (!response.ok) {
			throw new Error(`models.dev catalog request failed (${response.status})`);
		}
		const models = parseModelsDevByokLlmCatalog(await response.json());
		writeCache(storage, { stored_at: now, models });
		return models;
	} catch (error) {
		if (cached) return cached.models;
		throw error;
	}
}
