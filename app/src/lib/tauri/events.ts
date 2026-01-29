import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { BACKEND_EVENT_NAMES, type BackendEventName } from "./events.generated";
import type {
	ConnectionStateChangedPayload,
	EmptyEventPayload,
	LocalWhisperModelLoadEvent,
	MicTestAudioLevelPayload,
	OverlayAudioLevelPayload,
	OverlayOcrContextUnavailablePayload,
	PipelineErrorPayload,
	PipelineStateEvent,
	PipelineTranscriptReadyPayload,
	QuickAskAnswerPayload,
	QuickAskStartedPayload,
	SettingsChangedPayload,
	SystemEvent,
	WhisperModelDownloadProgress,
} from "./types";

export type EventMap = {
	"connection-state-changed": ConnectionStateChangedPayload;
	"history-changed": EmptyEventPayload;
	"local-whisper-model-load": LocalWhisperModelLoadEvent;
	"mic-test-audio-level": MicTestAudioLevelPayload;
	"overlay-audio-level": OverlayAudioLevelPayload;
	"overlay-ocr-context-unavailable": OverlayOcrContextUnavailablePayload;
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
	"transcript-copied-to-clipboard": EmptyEventPayload;
	"stats-changed": EmptyEventPayload;
	"system-event": SystemEvent;
	"whisper-model-download-progress": WhisperModelDownloadProgress;
};

export type EventName = BackendEventName;

type MissingInTs = Exclude<BackendEventName, keyof EventMap>;
type ExtraInTs = Exclude<keyof EventMap, BackendEventName>;
// If backend adds/removes/renames an event, this should become a type error.
const _EVENT_MAP_KEYS_MATCH_BACKEND: MissingInTs extends never
	? ExtraInTs extends never
		? true
		: never
	: never = true;

export const EVENT_NAMES =
	BACKEND_EVENT_NAMES satisfies ReadonlyArray<EventName>;

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
