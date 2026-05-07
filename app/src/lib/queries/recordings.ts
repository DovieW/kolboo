import { useMutation, useQuery } from "@tanstack/react-query";

import { audioSettingsTestAPI } from "../tauri";
import {
	createAudioMuteSupportedQueryFn,
	createDataStorageSummaryQueryFn,
	createLastRecordingDiagnosticsQueryFn,
	createRecordingsStatsQueryFn,
} from "./queryFns";
import { queryFnDeps } from "./shared";

// Recording/data-lifecycle hooks stay together because they all describe the
// local capture artifacts the settings UI can inspect.
export function useLastRecordingDiagnostics() {
	return useQuery({
		queryKey: ["lastRecordingDiagnostics"],
		queryFn: createLastRecordingDiagnosticsQueryFn(queryFnDeps),
		staleTime: 0,
		refetchOnWindowFocus: true,
		// Keep UI in sync if user records via hotkey while settings is open.
		refetchInterval: 2000,
	});
}

export function useAudioSettingsTestStartRecording() {
	return useMutation({
		mutationFn: () => audioSettingsTestAPI.startRecording(),
	});
}

export function useAudioSettingsTestStopRecording() {
	return useMutation({
		mutationFn: () => audioSettingsTestAPI.stopRecording(),
	});
}

export function useRecordingsStats() {
	return useQuery({
		queryKey: ["recordingsStats"],
		queryFn: createRecordingsStatsQueryFn(queryFnDeps),
		staleTime: 0,
		refetchOnWindowFocus: true,
		refetchInterval: 10000,
	});
}

export function useDataStorageSummary() {
	return useQuery({
		queryKey: ["dataStorageSummary"],
		queryFn: createDataStorageSummaryQueryFn(queryFnDeps),
		staleTime: 0,
		refetchOnWindowFocus: true,
		refetchInterval: 10000,
	});
}

export function useIsAudioMuteSupported() {
	return useQuery({
		queryKey: ["audioMuteSupported"],
		queryFn: createAudioMuteSupportedQueryFn(queryFnDeps),
		staleTime: Number.POSITIVE_INFINITY,
	});
}
