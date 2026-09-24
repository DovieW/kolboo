import { beforeEach, expect, it, vi } from "vitest";
const calls = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: calls.invoke }));
import { logsAPI } from "./commands";
beforeEach(() => calls.invoke.mockReset());
it("requests identifiers only for History availability", async () => {
	calls.invoke.mockResolvedValue(["first", "second"]);
	expect(await logsAPI.getRequestLogIds(25)).toEqual(["first", "second"]);
	expect(calls.invoke).toHaveBeenCalledExactlyOnceWith("get_request_log_ids", {
		limit: 25,
	});
});
