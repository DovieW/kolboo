import { describe, expect, it } from "vitest";
import {
	buildAnalysisPrompt,
	estimateTokenCount,
	getHistoryEntryProfileBadgeLabel,
	getHistoryFeedEmptyState,
	groupHistoryByDate,
	groupHistoryForDisplay,
	normalizePersistedHistoryFilters,
} from "./readModel";

describe("History Feed read model", () => {
	it("normalizes persisted filters and drops unknown model keys", () => {
		expect(
			normalizePersistedHistoryFilters(
				{
					filterText: "meeting",
					showFailed: false,
					showEmptyTranscript: true,
					selectedSttModelKeys: ["stt-a", "missing", 7],
					selectedLlmModelKeys: ["llm-a", "unknown"],
				},
				{ stt: ["stt-a"], llm: ["llm-a"] },
			),
		).toEqual({
			filterText: "meeting",
			showFailed: false,
			showEmptyTranscript: true,
			selectedSttModelKeys: ["stt-a"],
			selectedLlmModelKeys: ["llm-a"],
		});
	});

	it("returns null for corrupted persisted filters", () => {
		expect(normalizePersistedHistoryFilters(null)).toBeNull();
		expect(normalizePersistedHistoryFilters("oops")).toBeNull();
	});

	it("falls back to defaults for malformed persisted filter fields", () => {
		expect(
			normalizePersistedHistoryFilters(
				{
					filterText: 42,
					showFailed: "nope",
					showEmptyTranscript: "nah",
					selectedSttModelKeys: "bad",
					selectedLlmModelKeys: ["llm-a", 1],
				},
				{ llm: ["llm-a"] },
			),
		).toEqual({
			filterText: "",
			showFailed: true,
			showEmptyTranscript: false,
			selectedSttModelKeys: [],
			selectedLlmModelKeys: ["llm-a"],
		});
	});

	it("groups entries by display date without reordering items", () => {
		const grouped = groupHistoryByDate([
			{ id: "1", text: "first", timestamp: "2024-01-01T10:00:00Z" },
			{ id: "2", text: "second", timestamp: "2024-01-01T11:00:00Z" },
			{ id: "3", text: "third", timestamp: "2024-01-02T10:00:00Z" },
		]);

		expect(grouped.map((group) => group.items.map((item) => item.id))).toEqual([
			["1", "2"],
			["3"],
		]);
	});

	it("builds display view models for history entries", () => {
		const grouped = groupHistoryForDisplay([
			{
				id: "error",
				text: "",
				timestamp: "2024-01-01T10:00:00Z",
				status: "error",
				error_message: "Backend said nope",
				profile_id: "custom-profile",
				preset_id: "draft",
			},
			{
				id: "empty",
				text: "   ",
				timestamp: "2024-01-01T10:05:00Z",
				profile_name: "Default",
				preset_name: "Default",
			},
		]);

		expect(grouped).toHaveLength(1);
		expect(grouped[0]?.items).toMatchObject([
			{
				id: "error",
				contentKind: "error",
				displayText: "Backend said nope",
				hasCopyValue: true,
				profilePresetLabel: "custom-profile: draft",
				recordingRequestId: "error",
			},
			{
				id: "empty",
				contentKind: "empty",
				displayText: "No transcript",
				hasCopyValue: false,
				profilePresetLabel: null,
				recordingRequestId: "empty",
			},
		]);
	});

	it("formats profile and preset badges only when they are meaningful", () => {
		expect(
			getHistoryEntryProfileBadgeLabel({
				profile_name: "Default",
				profile_id: "default",
				preset_name: "Default",
				preset_id: null,
			}),
		).toBeNull();

		expect(
			getHistoryEntryProfileBadgeLabel({
				profile_name: "Support",
				profile_id: "support",
				preset_name: null,
				preset_id: "rewrite",
			}),
		).toBe("Support: rewrite");
	});

	it("builds analysis prompts from non-empty successful transcripts only", () => {
		const prompt = buildAnalysisPrompt(
			[
				{ id: "old", text: "too old", timestamp: "2024-01-01T00:00:00Z" },
				{
					id: "ok",
					text: "  useful note  ",
					timestamp: "2024-01-02T00:30:00Z",
				},
				{ id: "empty", text: "   ", timestamp: "2024-01-02T00:45:00Z" },
				{
					id: "failed",
					text: "failure text",
					timestamp: "2024-01-02T00:50:00Z",
					status: "error",
				},
			],
			{
				includeFromLastHours: 2,
				nowMs: new Date("2024-01-02T01:00:00Z").getTime(),
				style: "structured",
			},
		);

		expect(prompt.includedCount).toBe(1);
		expect(prompt.availableTranscriptsCount).toBe(2);
		expect(prompt.systemPrompt).toContain("meticulous organizer");
		expect(prompt.userPrompt).toContain("useful note");
		expect(prompt.userPrompt).not.toContain("failure text");
		expect(prompt.userPrompt).not.toContain("too old");
	});

	it("sorts included transcripts chronologically and explains empty results", () => {
		const prompt = buildAnalysisPrompt(
			[
				{ id: "2", text: "second", timestamp: "2024-01-02T12:00:00Z" },
				{ id: "1", text: "first", timestamp: "2024-01-01T12:00:00Z" },
			],
			{ style: "productive" },
		);

		expect(prompt.userPrompt.indexOf("first")).toBeLessThan(
			prompt.userPrompt.indexOf("second"),
		);

		const emptyPrompt = buildAnalysisPrompt(
			[
				{
					id: "failed",
					text: "ignored",
					timestamp: "2024-01-02T00:50:00Z",
					status: "error",
				},
			],
			{ style: "productive" },
		);

		expect(emptyPrompt.includedCount).toBe(0);
		expect(emptyPrompt.prompt).toContain(
			"No non-empty transcripts matched your filter",
		);
	});

	it("selects the correct empty state copy for the history tab", () => {
		expect(
			getHistoryFeedEmptyState({ totalHistoryCount: 0, isFiltering: false }),
		).toEqual({
			title: "No dictation history yet",
			message:
				"Your transcribed text will appear here after you use voice dictation.",
		});

		expect(
			getHistoryFeedEmptyState({ totalHistoryCount: 12, isFiltering: true }),
		).toEqual({
			title: "No matches",
			message: "Try a different filter.",
		});

		expect(
			getHistoryFeedEmptyState({ totalHistoryCount: 12, isFiltering: false }),
		).toEqual({
			title: "Nothing to show",
			message: "Start your first recording to see it here.",
		});
	});

	it("estimates tokens defensively for empty and non-empty text", () => {
		expect(estimateTokenCount("   ")).toBe(0);
		expect(estimateTokenCount("12345")).toBe(2);
	});
});
