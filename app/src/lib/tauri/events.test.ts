import { beforeEach, describe, expect, it, vi } from "vitest";

const emitMock = vi.fn();
const listenMock = vi.fn(
	async (_name: string, handler: (event: { payload: unknown }) => void) => {
		handler({ payload: null });
		return () => {
			// no-op
		};
	},
);

vi.mock("@tauri-apps/api/event", () => ({
	emit: emitMock,
	listen: listenMock,
}));

describe("typed tauri events", () => {
	beforeEach(() => {
		emitMock.mockReset();
		listenMock.mockReset();
		listenMock.mockImplementation(async (_name, handler) => {
			handler({ payload: null });
			return () => {
				// no-op
			};
		});
	});

	it("emitTyped forwards name and payload", async () => {
		const { emitTyped } = await import("./events");

		await emitTyped("stats-changed", null);

		expect(emitMock).toHaveBeenCalledWith("stats-changed", null);
	});

	it("listenTyped unwraps payload", async () => {
		const { listenTyped } = await import("./events");
		const handler = vi.fn();

		await listenTyped("stats-changed", handler);

		expect(listenMock).toHaveBeenCalledWith(
			"stats-changed",
			expect.any(Function),
		);
		expect(handler).toHaveBeenCalledWith(null);
	});

	it("listenTyped forwards settings-changed policy payload", async () => {
		listenMock.mockImplementationOnce(async (_name, handler) => {
			handler({
				payload: {
					policy_normalized: true,
					policy_constraints_applied: true,
					policy_violations: [{ path: "request_logs_privacy_mode" }],
				},
			});
			return () => {
				// no-op
			};
		});

		const { listenTyped } = await import("./events");
		const handler = vi.fn();

		await listenTyped("settings-changed", handler);

		expect(handler).toHaveBeenCalledWith(
			expect.objectContaining({
				policy_normalized: true,
				policy_constraints_applied: true,
			}),
		);
	});

	it("EVENT_NAMES contains key events", async () => {
		const { EVENT_NAMES } = await import("./events");

		expect(EVENT_NAMES).toContain("settings-changed");
		expect(EVENT_NAMES).toContain("pipeline-state-changed");
		expect(new Set(EVENT_NAMES).size).toBe(EVENT_NAMES.length);
	});
});
