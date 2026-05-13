import { describe, expect, it } from "vitest";
import { shouldRestartMicTestForSelectionChange } from "./useMicTestMeter";

describe("shouldRestartMicTestForSelectionChange", () => {
	it("restarts when a tracked explicit microphone changes while testing is desired", () => {
		expect(
			shouldRestartMicTestForSelectionChange({
				desiredMicTesting: true,
				hasTrackedSelection: true,
				previousSelectedMicId: "mic-1",
				nextSelectedMicId: "mic-2",
				disabled: false,
				startInFlight: false,
				restartInFlight: false,
			}),
		).toBe(true);
	});

	it("restarts when switching between system default and an explicit microphone", () => {
		expect(
			shouldRestartMicTestForSelectionChange({
				desiredMicTesting: true,
				hasTrackedSelection: true,
				previousSelectedMicId: null,
				nextSelectedMicId: "mic-2",
				disabled: false,
				startInFlight: false,
				restartInFlight: false,
			}),
		).toBe(true);

		expect(
			shouldRestartMicTestForSelectionChange({
				desiredMicTesting: true,
				hasTrackedSelection: true,
				previousSelectedMicId: "mic-2",
				nextSelectedMicId: null,
				disabled: false,
				startInFlight: false,
				restartInFlight: false,
			}),
		).toBe(true);
	});

	it("does not restart on the initial tracked selection snapshot", () => {
		expect(
			shouldRestartMicTestForSelectionChange({
				desiredMicTesting: true,
				hasTrackedSelection: false,
				previousSelectedMicId: null,
				nextSelectedMicId: "mic-2",
				disabled: false,
				startInFlight: false,
				restartInFlight: false,
			}),
		).toBe(false);
	});

	it("does not restart when testing is blocked or already syncing", () => {
		expect(
			shouldRestartMicTestForSelectionChange({
				desiredMicTesting: true,
				hasTrackedSelection: true,
				previousSelectedMicId: "mic-1",
				nextSelectedMicId: "mic-2",
				disabled: true,
				startInFlight: false,
				restartInFlight: false,
			}),
		).toBe(false);

		expect(
			shouldRestartMicTestForSelectionChange({
				desiredMicTesting: true,
				hasTrackedSelection: true,
				previousSelectedMicId: "mic-1",
				nextSelectedMicId: "mic-2",
				disabled: false,
				startInFlight: true,
				restartInFlight: false,
			}),
		).toBe(false);

		expect(
			shouldRestartMicTestForSelectionChange({
				desiredMicTesting: true,
				hasTrackedSelection: true,
				previousSelectedMicId: "mic-1",
				nextSelectedMicId: "mic-2",
				disabled: false,
				startInFlight: false,
				restartInFlight: true,
			}),
		).toBe(false);
	});
});
