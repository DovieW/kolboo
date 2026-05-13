import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
	micPeakToDbfs,
	micPeakToMeterColor,
	micPeakToMeterLevel,
	toMicTestErrorMessage,
} from "../lib/audioDevices";
import { tauriAPI } from "../lib/tauri";
import { listenTyped } from "../lib/tauri/events";

type MicTestStatusTone = "dimmed" | "blue" | "green" | "yellow" | "red";

export function shouldRestartMicTestForSelectionChange(params: {
	desiredMicTesting: boolean;
	hasTrackedSelection: boolean;
	previousSelectedMicId: string | null;
	nextSelectedMicId: string | null;
	disabled: boolean;
	startInFlight: boolean;
	restartInFlight: boolean;
}) {
	return (
		params.desiredMicTesting &&
		params.hasTrackedSelection &&
		params.previousSelectedMicId !== params.nextSelectedMicId &&
		!params.disabled &&
		!params.startInFlight &&
		!params.restartInFlight
	);
}

function resetMicTestState(params: {
	setIsMicTesting: (value: boolean) => void;
	setMicPeak: (value: number) => void;
	sessionIdRef: React.MutableRefObject<number | null>;
	lastEventAtRef: React.MutableRefObject<number | null>;
	startedAtRef: React.MutableRefObject<number | null>;
	hasSignalRef: React.MutableRefObject<boolean>;
}) {
	params.setIsMicTesting(false);
	params.setMicPeak(0);
	params.sessionIdRef.current = null;
	params.lastEventAtRef.current = null;
	params.startedAtRef.current = null;
	params.hasSignalRef.current = false;
}

export function useMicTestMeter({
	selectedMicId,
	disabled = false,
}: {
	selectedMicId: string | null;
	disabled?: boolean;
}) {
	const [isMicTesting, setIsMicTesting] = useState(false);
	const [micPeak, setMicPeak] = useState(0);
	const [micTestError, setMicTestError] = useState<string | null>(null);
	const [statusTick, setStatusTick] = useState(0);

	const micTestSessionIdRef = useRef<number | null>(null);
	const micTestStartInFlightRef = useRef(false);
	const micTestRestartInFlightRef = useRef(false);
	const desiredMicTestingRef = useRef(false);
	const latestSelectedMicIdRef = useRef<string | null>(selectedMicId);
	const prevSelectedMicIdRef = useRef<string | null>(null);
	const hasTrackedSelectionRef = useRef(false);
	const lastEventAtRef = useRef<number | null>(null);
	const startedAtRef = useRef<number | null>(null);
	const hasSignalRef = useRef(false);

	latestSelectedMicIdRef.current = selectedMicId;

	const clearMicTestError = useCallback(() => {
		setMicTestError(null);
	}, []);

	const stopMicTest = useCallback(
		async (options?: { preserveDesired?: boolean }) => {
			const preserveDesired = options?.preserveDesired ?? false;
			if (!preserveDesired) {
				desiredMicTestingRef.current = false;
			}

			try {
				await tauriAPI.stopMicTestMeter();
			} catch (error) {
				console.warn("Failed to stop mic test meter:", error);
			} finally {
				resetMicTestState({
					setIsMicTesting,
					setMicPeak,
					sessionIdRef: micTestSessionIdRef,
					lastEventAtRef,
					startedAtRef,
					hasSignalRef,
				});
			}
		},
		[],
	);

	const startMicTest = useCallback(async () => {
		if (disabled) return;

		desiredMicTestingRef.current = true;

		while (desiredMicTestingRef.current && !disabled) {
			const micIdToStart = latestSelectedMicIdRef.current;

			setMicTestError(null);
			micTestStartInFlightRef.current = true;
			startedAtRef.current = Date.now();
			lastEventAtRef.current = null;
			hasSignalRef.current = false;
			micTestSessionIdRef.current = null;
			setIsMicTesting(true);

			try {
				await tauriAPI.startMicTestMeter(micIdToStart);
			} catch (error) {
				desiredMicTestingRef.current = false;
				setMicTestError(toMicTestErrorMessage(error));
				resetMicTestState({
					setIsMicTesting,
					setMicPeak,
					sessionIdRef: micTestSessionIdRef,
					lastEventAtRef,
					startedAtRef,
					hasSignalRef,
				});
				return;
			} finally {
				micTestStartInFlightRef.current = false;
			}

			if (latestSelectedMicIdRef.current === micIdToStart) {
				return;
			}

			await stopMicTest({ preserveDesired: true });
		}
	}, [disabled, stopMicTest]);

	const restartMicTestForSelectionChange = useCallback(async () => {
		if (disabled) return;
		if (!desiredMicTestingRef.current) return;
		if (micTestStartInFlightRef.current || micTestRestartInFlightRef.current) {
			return;
		}

		micTestRestartInFlightRef.current = true;

		try {
			await stopMicTest({ preserveDesired: true });
			if (!desiredMicTestingRef.current || disabled) return;
			await startMicTest();
		} finally {
			micTestRestartInFlightRef.current = false;
		}
	}, [disabled, startMicTest, stopMicTest]);

	const toggleMicTest = useCallback(async () => {
		if (isMicTesting) {
			await stopMicTest();
			return;
		}

		await startMicTest();
	}, [isMicTesting, startMicTest, stopMicTest]);

	useEffect(() => {
		let unlisten: (() => void) | null = null;

		const setup = async () => {
			try {
				unlisten = await listenTyped("mic-test-audio-level", (payload) => {
					const sessionId =
						typeof payload.session_id === "number" &&
						Number.isFinite(payload.session_id)
							? payload.session_id
							: null;

					const currentSessionId = micTestSessionIdRef.current;
					if (
						sessionId != null &&
						currentSessionId != null &&
						sessionId !== currentSessionId
					) {
						return;
					}

					if (payload.active) {
						setIsMicTesting(true);
						if (sessionId != null) {
							micTestSessionIdRef.current = sessionId;
						}
						lastEventAtRef.current = Date.now();
					} else {
						resetMicTestState({
							setIsMicTesting,
							setMicPeak,
							sessionIdRef: micTestSessionIdRef,
							lastEventAtRef,
							startedAtRef,
							hasSignalRef,
						});
						return;
					}

					const peak =
						typeof payload.peak === "number" && Number.isFinite(payload.peak)
							? Math.max(0, Math.min(1, payload.peak))
							: 0;

					if (micPeakToDbfs(peak) >= -45) {
						hasSignalRef.current = true;
					}

					// We intentionally keep a gentle decay so users can read the level
					// without needing DAW reflexes every time they say a word.
					setMicPeak((prev) => Math.max(peak, prev * 0.82));
				});
			} catch (error) {
				console.warn("Failed to listen to mic-test-audio-level:", error);
			}
		};

		void setup();

		return () => {
			try {
				unlisten?.();
			} catch {
				// ignore listener cleanup failures
			}
		};
	}, []);

	useEffect(() => {
		const prev = prevSelectedMicIdRef.current;
		const hasTrackedSelection = hasTrackedSelectionRef.current;
		prevSelectedMicIdRef.current = selectedMicId;
		hasTrackedSelectionRef.current = true;

		if (
			!shouldRestartMicTestForSelectionChange({
				desiredMicTesting: desiredMicTestingRef.current,
				hasTrackedSelection,
				previousSelectedMicId: prev,
				nextSelectedMicId: selectedMicId,
				disabled,
				startInFlight: micTestStartInFlightRef.current,
				restartInFlight: micTestRestartInFlightRef.current,
			})
		) {
			return;
		}

		void restartMicTestForSelectionChange();
	}, [disabled, restartMicTestForSelectionChange, selectedMicId]);

	useEffect(() => {
		if (!disabled) return;
		if (!isMicTesting && !desiredMicTestingRef.current) return;
		void stopMicTest();
	}, [disabled, isMicTesting, stopMicTest]);

	useEffect(() => {
		return () => {
			void stopMicTest();
		};
	}, [stopMicTest]);

	useEffect(() => {
		if (!isMicTesting) return;

		const interval = window.setInterval(() => {
			setStatusTick((tick) => tick + 1);
		}, 250);

		return () => {
			window.clearInterval(interval);
		};
	}, [isMicTesting]);

	const meterLevel = micPeakToMeterLevel(micPeak);
	const meterColor = micPeakToMeterColor(micPeak);

	const status = useMemo(() => {
		void statusTick;

		if (micTestError) {
			return {
				text: micTestError,
				tone: "red" as MicTestStatusTone,
				isStale: false,
			};
		}

		if (disabled) {
			return {
				text: "Choose a working microphone before testing.",
				tone: "dimmed" as MicTestStatusTone,
				isStale: false,
			};
		}

		if (!isMicTesting) {
			return {
				text: "Click Test, then speak normally.",
				tone: "dimmed" as MicTestStatusTone,
				isStale: false,
			};
		}

		const now = Date.now();
		const startedAt = startedAtRef.current ?? now;
		const lastEventAt = lastEventAtRef.current;
		const hasSignal = hasSignalRef.current;
		const peakDbfs = micPeakToDbfs(micPeak);
		const isClipping = peakDbfs >= -3;
		const isStale = lastEventAt != null && now - lastEventAt > 1200;
		const noSignalYet = !hasSignal && now - startedAt > 1500;

		if (isStale) {
			return {
				text: "Meter stopped receiving audio updates. Try Refresh or pick another mic.",
				tone: "yellow" as MicTestStatusTone,
				isStale: true,
			};
		}

		if (isClipping) {
			return {
				text: "Signal detected — you’re very hot. Back off the mic a bit.",
				tone: "red" as MicTestStatusTone,
				isStale: false,
			};
		}

		if (hasSignal) {
			return {
				text: "Signal detected — speaking level looks good.",
				tone: "green" as MicTestStatusTone,
				isStale: false,
			};
		}

		if (noSignalYet) {
			return {
				text: "No signal yet — check the selected mic, Windows input mute, or permissions.",
				tone: "yellow" as MicTestStatusTone,
				isStale: false,
			};
		}

		if (lastEventAt == null) {
			return {
				text: "Starting microphone test…",
				tone: "blue" as MicTestStatusTone,
				isStale: false,
			};
		}

		return {
			text: "Listening… speak normally.",
			tone: "blue" as MicTestStatusTone,
			isStale: false,
		};
	}, [disabled, isMicTesting, micPeak, micTestError, statusTick]);

	return {
		isMicTesting,
		meterLevel,
		meterColor,
		micTestError,
		statusText: status.text,
		statusTone: status.tone,
		isStreamStale: status.isStale,
		clearMicTestError,
		stopMicTest,
		toggleMicTest,
	};
}
