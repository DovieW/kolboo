import { invoke } from "@tauri-apps/api/core";
import { expect, it, vi } from "vitest";
import { tauriAPI } from "./commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
it("requests a lightweight activity summary for the selected timeframe", async () => {
	const result = {
		totals: {
			recordings: 0,
			words: 0,
			audio_seconds: 0,
			timed_recordings: 0,
			meetings: 0,
		},
		days: [],
		models: [],
	};
	vi.mocked(invoke).mockResolvedValue(result);
	expect(await tauriAPI.getHistoryActivity("7d")).toEqual(result);
	expect(invoke).toHaveBeenCalledExactlyOnceWith("get_history_activity", {
		timeframe: "7d",
	});
	vi.mocked(invoke).mockRejectedValue(new Error("Unavailable"));
	await expect(tauriAPI.getHistoryActivity("all")).rejects.toThrow(
		"Unavailable",
	);
});
