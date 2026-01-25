import { describe, expect, it } from "vitest";

import { applyAnimatedHideGate } from "./overlayHideGate";

describe("applyAnimatedHideGate", () => {
	it("accepts first request", () => {
		const res = applyAnimatedHideGate({
			now: 1000,
			animState: "visible",
			state: { lastRequestAt: null },
			cooldownMs: 350,
		});
		expect(res.accept).toBe(true);
		expect(res.nextState.lastRequestAt).toBe(1000);
	});

	it("rejects if already exiting", () => {
		const res = applyAnimatedHideGate({
			now: 1000,
			animState: "exit",
			state: { lastRequestAt: null },
			cooldownMs: 350,
		});
		expect(res.accept).toBe(false);
	});

	it("rejects repeated requests inside cooldown", () => {
		const first = applyAnimatedHideGate({
			now: 1000,
			animState: "visible",
			state: { lastRequestAt: null },
			cooldownMs: 350,
		});
		expect(first.accept).toBe(true);

		const second = applyAnimatedHideGate({
			now: 1200,
			animState: "visible",
			state: first.nextState,
			cooldownMs: 350,
		});
		expect(second.accept).toBe(false);
	});

	it("accepts after cooldown", () => {
		const first = applyAnimatedHideGate({
			now: 1000,
			animState: "visible",
			state: { lastRequestAt: null },
			cooldownMs: 350,
		});
		expect(first.accept).toBe(true);

		const second = applyAnimatedHideGate({
			now: 1500,
			animState: "visible",
			state: first.nextState,
			cooldownMs: 350,
		});
		expect(second.accept).toBe(true);
		expect(second.nextState.lastRequestAt).toBe(1500);
	});
});
