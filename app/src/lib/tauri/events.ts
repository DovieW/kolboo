import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
	ConnectionStateChangedPayload,
	EmptyEventPayload,
	LocalWhisperModelLoadEvent,
	MicTestAudioLevelPayload,
	OverlayAudioLevelPayload,
	PipelineErrorPayload,
	PipelineStateEvent,
	PipelineTranscriptReadyPayload,
	QuickAskAnswerPayload,
	QuickAskStartedPayload,
	SettingsChangedPayload,
	SystemEvent,
	WhisperModelDownloadProgress,
} from "../tauri";

export type EventMap = {
	"connection-state-changed": ConnectionStateChangedPayload;
	"history-changed": EmptyEventPayload;
	"local-whisper-model-load": LocalWhisperModelLoadEvent;
	"mic-test-audio-level": MicTestAudioLevelPayload;
	"overlay-audio-level": OverlayAudioLevelPayload;
	"overlay-hide-requested": EmptyEventPayload;
	"pipeline-cancelled": EmptyEventPayload;
	"pipeline-error": PipelineErrorPayload;
	"pipeline-recording-started": EmptyEventPayload;
	"pipeline-reset": EmptyEventPayload;
	"pipeline-rewriting-started": EmptyEventPayload;
	"pipeline-routing-started": EmptyEventPayload;
	"pipeline-state-changed": PipelineStateEvent;
	"pipeline-transcript-ready": PipelineTranscriptReadyPayload;
	"pipeline-transcription-started": EmptyEventPayload;
	"quick-ask-answer": QuickAskAnswerPayload;
	"quick-ask-started": QuickAskStartedPayload;
	"recording-start": EmptyEventPayload;
	"recording-stop": EmptyEventPayload;
	"request-disconnect": EmptyEventPayload;
	"settings-changed": SettingsChangedPayload;
	"stats-changed": EmptyEventPayload;
	"system-event": SystemEvent;
	"whisper-model-download-progress": WhisperModelDownloadProgress;
};

export type EventName = keyof EventMap;

export const EVENT_NAMES = [
	"connection-state-changed",
	"history-changed",
	"local-whisper-model-load",
	"mic-test-audio-level",
	"overlay-audio-level",
	"overlay-hide-requested",
	"pipeline-cancelled",
	"pipeline-error",
	"pipeline-recording-started",
	"pipeline-reset",
	"pipeline-rewriting-started",
	"pipeline-routing-started",
	"pipeline-state-changed",
	"pipeline-transcript-ready",
	"pipeline-transcription-started",
	"quick-ask-answer",
	"quick-ask-started",
	"recording-start",
	"recording-stop",
	"request-disconnect",
	"settings-changed",
	"stats-changed",
	"system-event",
	"whisper-model-download-progress",
] as const satisfies ReadonlyArray<EventName>;

export async function listenTyped<K extends EventName>(
	name: K,
	handler: (payload: EventMap[K]) => void,
): Promise<UnlistenFn> {
	return listen(name, (event) => {
		handler(event.payload as EventMap[K]);
	});
}

export function emitTyped<K extends EventName>(
	name: K,
	payload: EventMap[K],
): Promise<void> {
	return emit(name, payload);
}
