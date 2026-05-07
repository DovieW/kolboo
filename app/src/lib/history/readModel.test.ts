import { describe, expect, it } from "vitest";
import {
	buildAnalysisPrompt,
	estimateTokenCount,
	groupHistoryByDate,
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

	it("estimates tokens defensively for empty and non-empty text", () => {
		expect(estimateTokenCount("   ")).toBe(0);
		expect(estimateTokenCount("12345")).toBe(2);
	});
});
