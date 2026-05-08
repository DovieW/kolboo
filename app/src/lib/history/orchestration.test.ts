import { describe, expect, it } from "vitest";
import type { HistoryDeleteOptions, HistoryEntry } from "../tauri";
import {
	addHiddenHistoryEntryIds,
	buildRecordingProbePlan,
	classifyHistoryDeleteOptions,
	collectHistoryEntryIdsUsingRecording,
	getRetryLastFailedActionState,
	removeHiddenHistoryEntryIds,
} from "./orchestration";

function entry(
	overrides: Partial<
		Pick<HistoryEntry, "id" | "recording_request_id" | "timestamp" | "status">
	> = {},
): Pick<HistoryEntry, "id" | "recording_request_id" | "timestamp" | "status"> {
	return {
		id: "req-1",
		recording_request_id: null,
		timestamp: "2026-05-07T10:00:00.000Z",
		status: "success",
		...overrides,
	};
}

describe("History Feed orchestration helpers", () => {
	it("prioritizes recent or in-progress recording probes and de-dupes shared ids", () => {
		const now = new Date("2026-05-07T10:00:20.000Z").getTime();
		const recordingExistsById = new Map([
			["already-there", { exists: true, checkedAt: now - 1000 }],
			["retry-me", { exists: false, checkedAt: now - 1000 }],
		]);

		const plan = buildRecordingProbePlan(
			[
				entry({
					id: "a",
					recording_request_id: "shared",
					status: "in_progress",
				}),
				entry({
					id: "b",
					recording_request_id: "shared",
					status: "in_progress",
				}),
				entry({ id: "c", recording_request_id: "retry-me" }),
				entry({ id: "d", recording_request_id: "already-there" }),
				entry({ id: "e", recording_request_id: "first-seen" }),
			],
			recordingExistsById,
			now,
		);

		expect(plan.shouldPollAgain).toBe(true);
		expect(plan.batch).toEqual(["shared", "retry-me", "first-seen"]);
	});

	it("supports optimistic hidden-entry rollback without mutating previous sets", () => {
		const hidden = addHiddenHistoryEntryIds(new Set(["keep-me"]), [
			"a",
			"b",
			"a",
		]);
		expect([...hidden]).toEqual(["keep-me", "a", "b"]);

		const rolledBack = removeHiddenHistoryEntryIds(hidden, ["b", "missing"]);
		expect([...rolledBack]).toEqual(["keep-me", "a"]);
		expect([...hidden]).toEqual(["keep-me", "a", "b"]);
	});

	it("classifies delete options for transcript-only, unshared recording, and shared recording flows", () => {
		const noRecording: HistoryDeleteOptions = {
			recording_id: null,
			recording_exists: false,
			recording_ref_count: 0,
		};
		const unsharedRecording: HistoryDeleteOptions = {
			recording_id: "rec-1",
			recording_exists: true,
			recording_ref_count: 1,
		};
		const sharedRecording: HistoryDeleteOptions = {
			recording_id: "rec-2",
			recording_exists: true,
			recording_ref_count: 3,
		};

		expect(classifyHistoryDeleteOptions("entry-1", noRecording)).toEqual({
			kind: "delete_entry_only",
			mode: "entry_only",
		});
		expect(classifyHistoryDeleteOptions("entry-2", unsharedRecording)).toEqual({
			kind: "delete_entry_and_recording",
			mode: "entry_and_recording",
		});
		expect(classifyHistoryDeleteOptions("entry-3", sharedRecording)).toEqual({
			kind: "confirm_shared_recording",
			context: {
				entryId: "entry-3",
				recordingId: "rec-2",
				refCount: 3,
			},
		});
	});

	it("collects all visible entries sharing a recording before optimistic delete", () => {
		expect(
			collectHistoryEntryIdsUsingRecording(
				[
					entry({ id: "entry-1", recording_request_id: "rec-1" }),
					entry({ id: "entry-2", recording_request_id: "rec-1" }),
					entry({ id: "entry-3", recording_request_id: "rec-3" }),
				],
				"rec-1",
				"entry-1",
			),
		).toEqual(["entry-1", "entry-2"]);

		expect(
			collectHistoryEntryIdsUsingRecording([], "rec-missing", "entry-fallback"),
		).toEqual(["entry-fallback"]);
	});

	it("shapes retry-last-failed quick-action availability copy", () => {
		expect(getRetryLastFailedActionState(null)).toEqual({
			canRetry: false,
			tooltip: "No failed requests with saved audio found",
		});

		expect(
			getRetryLastFailedActionState({
				entryId: "entry-1",
				recordingRequestId: "rec-1",
			}),
		).toEqual({
			canRetry: true,
			tooltip: "Retry the most recent failed request (copies result)",
		});
	});
});
