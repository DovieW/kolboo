import { describe, expect, it } from "vitest";
import { getRetryLastFailedCandidate } from "./historyRetry";
import type { HistoryEntry } from "./tauri";

function entry(overrides: Partial<HistoryEntry> = {}): HistoryEntry {
	return {
		id: "id",
		timestamp: "2026-01-01T00:00:00Z",
		text: "",
		status: "success",
		error_message: null,
		profile_id: null,
		profile_name: null,
		preset_id: null,
		preset_name: null,
		stt_provider: null,
		stt_model: null,
		llm_provider: null,
		llm_model: null,
		recording_request_id: null,
		...overrides,
	};
}

describe("getRetryLastFailedCandidate", () => {
	it("returns null when there are no failed entries", () => {
		expect(
			getRetryLastFailedCandidate([entry(), entry({ id: "x" })]),
		).toBeNull();
	});

	it("prefers a failed entry that explicitly points at a recording", () => {
		const result = getRetryLastFailedCandidate([
			entry({ id: "a", status: "error", recording_request_id: null }),
			entry({ id: "b", status: "error", recording_request_id: "rec-b" }),
			entry({ id: "c", status: "error", recording_request_id: "rec-c" }),
		]);

		expect(result).toEqual({ entryId: "b", recordingRequestId: "rec-b" });
	});

	it("falls back to the entry id when recording_request_id is missing", () => {
		const result = getRetryLastFailedCandidate([
			entry({ id: "a", status: "success", recording_request_id: "rec-a" }),
			entry({ id: "b", status: "error", recording_request_id: null }),
		]);

		expect(result).toEqual({ entryId: "b", recordingRequestId: "b" });
	});
});
