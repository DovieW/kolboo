import { useEffect, useRef } from "react";
import { tauriAPI, type HistorySummary } from "../tauri";
import { listenTyped } from "../tauri/events";
import { useBackendEvent } from "../tauri/useBackendEvent";
import { type AnalyticsPage, trackAggregateEvent } from "./posthog";

/** Main-window bridge for metadata-only desktop usage events. */
export function AggregateAnalyticsBridge({
	enabled,
	page,
}: {
	enabled: boolean;
	page: AnalyticsPage;
}) {
	const openedSent = useRef(false);

	useEffect(() => {
		if (enabled && !openedSent.current) {
			openedSent.current = true;
			void trackAggregateEvent("desktop_opened");
		}
	}, [enabled]);

	useEffect(() => {
		if (!enabled) return;
		void trackAggregateEvent("page_viewed", { page });
	}, [enabled, page]);

	useEffect(() => {
		if (!enabled) return;
		let disposed = false;
		let unlisten: (() => void) | undefined;
		let initialized = false;
		let fetching = false;
		let pending = false;
		const statuses = new Map<string, HistorySummary["status"]>();
		const refresh = async () => {
			if (fetching) {
				pending = true;
				return;
			}
			fetching = true;
			try {
				const page = await tauriAPI.getHistoryPage({
					page: 1,
					pageSize: 50,
					showFailed: true,
					showEmptyTranscript: true,
				});
				if (disposed) return;
				for (const item of page.items) {
					const prior = statuses.get(item.id);
					if (
						initialized &&
						prior !== item.status &&
						item.status !== "in_progress"
					) {
						void trackAggregateEvent("recording_finished", {
							status: item.status,
							mode: item.recording_mode,
							duration_seconds: item.duration_seconds,
							stt_provider: item.stt_provider,
							stt_model: item.stt_model,
						});
					}
					statuses.set(item.id, item.status);
				}
				initialized = true;
			} catch {
				// History remains usable if analytics observation fails.
			} finally {
				fetching = false;
				if (pending && !disposed) {
					pending = false;
					void refresh();
				}
			}
		};
		void listenTyped("history-changed", () => void refresh())
			.then((stop) => {
				if (disposed) stop();
				else unlisten = stop;
			})
			.catch(() => {});
		void refresh();
		return () => {
			disposed = true;
			unlisten?.();
		};
	}, [enabled]);

	useBackendEvent(
		"pipeline-recording-started",
		() => void trackAggregateEvent("recording_started"),
		enabled,
	);
	useBackendEvent(
		"pipeline-transcription-started",
		() => void trackAggregateEvent("pipeline_transcription_started"),
		enabled,
	);
	useBackendEvent(
		"pipeline-transcript-ready",
		(text) => {
			if (text.trim()) void trackAggregateEvent("pipeline_transcript_ready");
		},
		enabled,
	);
	useBackendEvent(
		"pipeline-error",
		() => void trackAggregateEvent("pipeline_error"),
		enabled,
	);

	return null;
}
