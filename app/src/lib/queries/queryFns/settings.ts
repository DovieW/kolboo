import type { QueryFnDeps } from "./shared";

export const createSettingsQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getSettings();

export const createSystemProxyInfoQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getSystemProxyInfo();

export const createSettingsGuideStateQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getSettingsGuideState();

export const createAudioMuteSupportedQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.isAudioMuteSupported();
