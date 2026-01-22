import type { HistoryEntry } from "./tauri";

export type RetryLastFailedCandidate = {
	// The history entry id we should retry from.
	entryId: string;
	// The request id that likely owns the WAV recording.
	recordingRequestId: string;
};

export function getRetryLastFailedCandidate(
	entries: HistoryEntry[],
): RetryLastFailedCandidate | null {
	// Prefer error entries that *explicitly* point at a recording.
	for (const entry of entries) {
		if (entry.status !== "error") continue;

		const entryId = entry.id.trim();
		if (!entryId) continue;

		const recordingRequestId = (entry.recording_request_id ?? "").trim();
		if (!recordingRequestId) continue;

		return { entryId, recordingRequestId };
	}

	// Fallback: some older entries might not have `recording_request_id` populated even
	// though a WAV exists under the entry id.
	for (const entry of entries) {
		if (entry.status !== "error") continue;

		const entryId = entry.id.trim();
		if (!entryId) continue;

		return { entryId, recordingRequestId: entryId };
	}

	return null;
}
