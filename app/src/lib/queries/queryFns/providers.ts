import type { QueryFnDeps } from "./shared";

export const createDefaultSectionsQueryFn = (deps: QueryFnDeps) => () =>
	deps.configAPI.getDefaultSections();

export const createAvailableProvidersQueryFn = (deps: QueryFnDeps) => () =>
	deps.configAPI.getAvailableProviders();

export const createIsLocalWhisperAvailableQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.isLocalWhisperAvailable();

export const createLocalWhisperBackendStatusQueryFn =
	(deps: QueryFnDeps) => () =>
		deps.tauriAPI.getLocalWhisperBackendStatus();

export const createWhisperModelsQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getWhisperModels();

export const createFireworksModelsQueryFn = (deps: QueryFnDeps) => () =>
	deps.llmAPI.getFireworksModels();

export const createOllamaModelsQueryFn = (deps: QueryFnDeps) => () =>
	deps.llmAPI.getOllamaModels();

export const createIsLocalWhisperModelLoadedQueryFn =
	(deps: QueryFnDeps) => () =>
		deps.tauriAPI.isLocalWhisperModelLoaded();

export const createWhisperModelsDirQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getWhisperModelsDir();
