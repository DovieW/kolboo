// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { MicrophoneControl } from "./MicrophoneControl";

const mocks = vi.hoisted(() => ({
	settings: { selected_mic_id: null as string | null },
	devices: {
		data: {
			devices: [{ id: "mic-1", name: "USB Mic" }],
			defaultDeviceName: "USB Mic",
		},
		isLoading: false,
		isError: false,
		isFetching: false,
		refetch: vi.fn(),
	},
	update: { mutate: vi.fn(), isPending: false },
	micTest: {
		isMicTesting: false,
		meterLevel: 0,
		meterColor: "#22c55e",
		micTestError: null as string | null,
		statusText: "Click Test, then speak normally.",
		statusTone: "dimmed",
		clearMicTestError: vi.fn(),
		stopMicTest: vi.fn(async () => {}),
		toggleMicTest: vi.fn(async () => {}),
	},
}));
vi.mock("../lib/queries", () => ({
	useSettings: () => ({ data: mocks.settings, isLoading: false }),
	useAudioInputDevices: () => mocks.devices,
	useUpdateSelectedMic: () => mocks.update,
}));
vi.mock("../hooks/useMicTestMeter", () => ({
	useMicTestMeter: () => mocks.micTest,
}));

const host = document.createElement("div");
let root = createRoot(host);
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	mocks.micTest.isMicTesting = false;
	mocks.micTest.micTestError = null;
	mocks.micTest.statusText = "Click Test, then speak normally.";
	mocks.micTest.statusTone = "dimmed";
});
afterEach(async () => {
	await act(async () => root.unmount());
	root = createRoot(host);
	vi.clearAllMocks();
});

it("keeps the idle mic row quiet and shows test feedback only while testing", async () => {
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<MicrophoneControl variant="home" />
			</MantineProvider>,
		),
	);
	expect(host.querySelector(".microphone-test-feedback")).toBeNull();
	mocks.micTest.isMicTesting = true;
	mocks.micTest.statusText = "Signal detected — speaking level looks good.";
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<MicrophoneControl variant="home" />
			</MantineProvider>,
		),
	);
	expect(host.querySelector('[role="status"]')?.textContent).toContain(
		"Signal detected",
	);
	mocks.micTest.statusText = "No microphone signal detected.";
	mocks.micTest.statusTone = "yellow";
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<MicrophoneControl variant="home" />
			</MantineProvider>,
		),
	);
	expect(host.querySelector('[role="status"]')?.textContent).toContain(
		"No microphone signal detected",
	);
});

it("shows microphone-test failures after the test stops", async () => {
	mocks.micTest.micTestError = "Microphone could not start.";
	mocks.micTest.statusText = mocks.micTest.micTestError;
	mocks.micTest.statusTone = "red";
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<MicrophoneControl variant="settings" />
			</MantineProvider>,
		),
	);
	expect(host.querySelector('[role="status"]')?.textContent).toBe(
		"Microphone could not start.",
	);
});
