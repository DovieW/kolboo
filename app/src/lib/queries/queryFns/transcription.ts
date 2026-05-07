import type { QueryFnDeps } from "./shared";

export const createHasLastAudioQueryFn = (deps: QueryFnDeps) => () =>
	deps.sttAPI.hasLastAudio();

export const createLastRecordingDiagnosticsQueryFn =
	(deps: QueryFnDeps) => () =>
		deps.sttAPI.getLastRecordingDiagnostics();
