import type { AudioInputDevicesQueryData } from "../../audioDevices";
import type { QueryFnDeps } from "./shared";

export const createSettingsQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getSettings();

export const createAudioInputDevicesQueryFn =
	(deps: QueryFnDeps) => async (): Promise<AudioInputDevicesQueryData> => {
		const [devices, defaultDeviceName] = await Promise.all([
			deps.tauriAPI.listAudioInputDevicesV2(),
			deps.tauriAPI.getDefaultAudioInputDeviceName(),
		]);

		return {
			devices,
			defaultDeviceName,
		};
	};

export const createSystemProxyInfoQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getSystemProxyInfo();

export const createSettingsGuideStateQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getSettingsGuideState();

export const createAudioMuteSupportedQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.isAudioMuteSupported();
