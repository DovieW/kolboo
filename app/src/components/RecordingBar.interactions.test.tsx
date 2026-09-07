// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { RecordingPreferences } from "../lib/tauri/types";
import { RecordingBar } from "./RecordingBar";

const mock = vi.hoisted(() => ({
	state: "idle",
	historyOnly: false,
	saved: [] as string[],
	onCancelled: null as (() => void) | null,
	stopListening: vi.fn(),
	listen: vi.fn(),
	api: {
		getPreferences: vi.fn(),
		setPreferences: vi.fn(),
		getSeconds: vi.fn(),
		canPause: vi.fn(),
		computerAudioAvailable: vi.fn(),
		getState: vi.fn(),
		listRecovery: vi.fn(),
		recover: vi.fn(),
		discardRecovery: vi.fn(),
		getPaused: vi.fn(),
		setPaused: vi.fn(),
		start: vi.fn(),
		stop: vi.fn(),
		cancel: vi.fn(),
	},
}));
vi.mock("./MeetingModelDialog", () => ({
	MeetingModelDialog: ({
		preferences,
		onSave,
		onClose,
		error,
	}: {
		preferences: RecordingPreferences;
		onSave: (value: RecordingPreferences) => void;
		onClose: () => void;
		error?: string | null;
	}) => (
		<div role="dialog" aria-label="Meeting model">
			{error && <p>{error}</p>}
			<button
				type="button"
				onClick={() =>
					onSave({
						...preferences,
						meeting_model: {
							provider: "openai",
							model: "speaker-model",
							use_managed: false,
						},
					})
				}
			>
				Save meeting model
			</button>
			<button type="button" onClick={onClose}>
				Close meeting model
			</button>
		</div>
	),
}));
vi.mock("../lib/tauri/commands", () => ({ recordingControlsAPI: mock.api }));
vi.mock("../lib/tauri/events", () => ({ listenTyped: mock.listen }));
let client: QueryClient;
let host: HTMLDivElement;
let root: Root;
let unmounted = false;
beforeEach(() => {
	vi.useFakeTimers();
	vi.clearAllMocks();
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	mock.state = "idle";
	mock.historyOnly = false;
	mock.saved = [];
	mock.onCancelled = null;
	mock.listen.mockImplementation((_name: string, callback: () => void) => {
		mock.onCancelled = callback;
		return Promise.resolve(mock.stopListening);
	});
	mock.api.getPreferences.mockResolvedValue({
		mode: "dictation",
		meeting_model: null,
	});
	mock.api.setPreferences.mockResolvedValue(undefined);
	mock.api.getSeconds.mockResolvedValue(42);
	mock.api.canPause.mockImplementation(
		async () => mock.historyOnly && mock.state === "recording",
	);
	mock.api.computerAudioAvailable.mockResolvedValue(false);
	mock.api.getState.mockImplementation(async () => mock.state);
	mock.api.listRecovery.mockImplementation(async () => mock.saved);
	mock.api.getPaused.mockResolvedValue(false);
	mock.api.cancel.mockResolvedValue(undefined);
	mock.api.recover.mockResolvedValue({
		transcription_complete: true,
		recovery_id: null,
		message: null,
	});
	mock.api.discardRecovery.mockImplementation(async (id: string) => {
		mock.saved = mock.saved.filter((entry) => entry !== id);
	});
	client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	unmounted = false;
});
afterEach(async () => {
	if (!unmounted) await act(async () => root.unmount());
	client.clear();
	host.remove();
	vi.useRealTimers();
});
async function flush() {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(0);
	});
}
async function render() {
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<QueryClientProvider client={client}>
					<RecordingBar />
				</QueryClientProvider>
			</MantineProvider>,
		),
	);
	await flush();
}
function button(text: string) {
	const result = [...document.querySelectorAll("button")].find(
		(element) => element.textContent === text,
	);
	if (!result) throw new Error(`Missing button: ${text}`);
	return result;
}
async function openSaved() {
	await act(async () =>
		(
			host.querySelector(
				'button[aria-label^="Recording options"]',
			) as HTMLButtonElement
		).click(),
	);
	await act(async () =>
		button(`Saved recordings (${mock.saved.length})`).click(),
	);
}
it("refreshes saved recordings immediately after Escape without claiming success from the event alone", async () => {
	await render();
	expect(mock.listen).toHaveBeenCalledWith(
		"pipeline-cancelled",
		expect.any(Function),
	);
	mock.saved = ["fixture-recording"];
	await act(async () => mock.onCancelled?.());
	await flush();
	expect(
		host.querySelector(
			'button[aria-label="Recording options: 1 saved recording"]',
		),
	).not.toBeNull();
	expect(document.querySelector('[role="dialog"]')).toBeNull();
	expect(document.body.textContent).not.toContain("Saved successfully");
});

it("does not overwrite saved mode/model while recording preferences are still loading", async () => {
	mock.api.getPreferences.mockImplementationOnce(() => new Promise(() => {}));
	await render();
	const record = host.querySelector(
		'button[aria-label="Record"]',
	) as HTMLButtonElement;
	expect(record.disabled).toBe(true);
	await act(async () =>
		(
			host.querySelector(
				'button[aria-label^="Recording options"]',
			) as HTMLButtonElement
		).click(),
	);
	const modes = [
		...document.querySelectorAll<HTMLInputElement>('input[type="radio"]'),
	];
	expect(modes).toHaveLength(2);
	expect(modes.every((mode) => mode.disabled)).toBe(true);
	await act(async () => {
		record.click();
		modes[1]?.click();
	});
	expect(mock.api.start).not.toHaveBeenCalled();
	expect(mock.api.setPreferences).not.toHaveBeenCalled();
});

it("blocks recording after a preference failure and offers a retry that restores the saved mode", async () => {
	mock.api.getPreferences.mockRejectedValueOnce(
		new Error("Recording preferences unavailable"),
	);
	await render();
	expect(
		(host.querySelector('button[aria-label="Record"]') as HTMLButtonElement)
			.disabled,
	).toBe(true);
	expect(document.body.textContent).toContain(
		"Recording preferences unavailable",
	);
	mock.api.getPreferences.mockResolvedValue({
		mode: "meeting",
		meeting_model: {
			provider: "openai",
			model: "speaker-model",
			use_managed: false,
		},
	});
	await act(async () => button("Retry recording preferences").click());
	await flush();
	expect(
		(host.querySelector('button[aria-label="Record"]') as HTMLButtonElement)
			.disabled,
	).toBe(false);
	expect(client.getQueryData(["recording-preferences"])).toEqual({
		mode: "meeting",
		meeting_model: {
			provider: "openai",
			model: "speaker-model",
			use_managed: false,
		},
	});
	expect(mock.api.setPreferences).not.toHaveBeenCalled();
});
it("keeps a save-for-later failure visible without claiming that audio was retained", async () => {
	mock.state = "recording";
	mock.historyOnly = true;
	mock.api.cancel.mockRejectedValueOnce(
		new Error("Local recording could not be saved"),
	);
	await render();
	const save = host.querySelector(
		'button[aria-label="Stop & save for later"]',
	) as HTMLButtonElement;
	expect(save).not.toBeNull();
	await act(async () => save.click());
	await flush();
	expect(document.body.textContent).toContain(
		"Local recording could not be saved",
	);
	expect(document.body.textContent).toContain("No saved recordings found");
	expect(document.body.textContent).not.toContain(
		"Audio is saved locally until",
	);
});

it("keeps a model-save failure in its dialog without opening Saved recordings on top", async () => {
	mock.api.getPreferences.mockResolvedValue({
		mode: "meeting",
		meeting_model: null,
	});
	mock.api.setPreferences.mockRejectedValueOnce(
		new Error("Local settings could not be saved"),
	);
	await render();
	await act(async () =>
		(
			host.querySelector(
				'button[aria-label^="Recording options"]',
			) as HTMLButtonElement
		).click(),
	);
	await act(async () => button("Meeting model").click());
	await act(async () => button("Save meeting model").click());
	await flush();
	expect(document.body.textContent).toContain(
		"Local settings could not be saved",
	);
	expect(document.querySelectorAll('[role="dialog"]')).toHaveLength(1);
	expect(
		document.querySelector('[role="dialog"]')?.getAttribute("aria-label"),
	).toBe("Meeting model");
	await act(async () => button("Close meeting model").click());
	await flush();
	expect(document.querySelector('[role="dialog"]')).toBeNull();
});

it("does not reset a model save in flight or allow recording before it finishes", async () => {
	mock.api.getPreferences.mockResolvedValue({
		mode: "meeting",
		meeting_model: null,
	});
	let finish: () => void = () => {};
	mock.api.setPreferences.mockImplementationOnce(
		() =>
			new Promise<void>((resolve) => {
				finish = resolve;
			}),
	);
	await render();
	await act(async () =>
		(
			host.querySelector(
				'button[aria-label^="Recording options"]',
			) as HTMLButtonElement
		).click(),
	);
	await act(async () => button("Meeting model").click());
	await act(async () => button("Save meeting model").click());
	await flush();
	await act(async () => button("Close meeting model").click());
	expect(
		document.querySelector('[role="dialog"][aria-label="Meeting model"]'),
	).not.toBeNull();
	expect(
		(host.querySelector('button[aria-label="Record"]') as HTMLButtonElement)
			.disabled,
	).toBe(true);
	await act(async () => button("Save meeting model").click());
	expect(mock.api.setPreferences).toHaveBeenCalledOnce();
	await act(async () => finish());
	await flush();
	expect(document.querySelector('[role="dialog"]')).toBeNull();
	expect(
		(host.querySelector('button[aria-label="Record"]') as HTMLButtonElement)
			.disabled,
	).toBe(false);
});

it("disables Stop while a save-for-later request is pending", async () => {
	mock.state = "recording";
	mock.historyOnly = true;
	let finish: () => void = () => {};
	mock.api.cancel.mockImplementationOnce(
		() =>
			new Promise<void>((resolve) => {
				finish = resolve;
			}),
	);
	await render();
	await act(async () =>
		(
			host.querySelector(
				'button[aria-label="Stop & save for later"]',
			) as HTMLButtonElement
		).click(),
	);
	await flush();
	expect(
		(
			host.querySelector(
				'button[aria-label="Stop & transcribe"]',
			) as HTMLButtonElement
		).disabled,
	).toBe(true);
	expect(mock.api.stop).not.toHaveBeenCalled();
	await act(async () => finish());
	await flush();
});
it("shows an incomplete recovery result without treating a resolved command as completion", async () => {
	mock.saved = ["fixture-recording"];
	mock.api.recover.mockResolvedValueOnce({
		transcription_complete: false,
		recovery_id: "fixture-recording",
		message: "Transcription could not finish. Audio is retained.",
	});
	await render();
	await openSaved();
	await act(async () => button("Transcribe").click());
	await flush();
	expect(document.body.textContent).toContain("Recovery needs attention");
	expect(document.body.textContent).toContain("Transcription could not finish");
	expect(document.body.textContent).not.toContain("Transcript saved");
	expect(button("Transcribe").disabled).toBe(false);
});
it("keeps a cleanup-only recovery handle and labels it without offering another transcription", async () => {
	mock.saved = ["fixture-recording"];
	mock.api.recover.mockImplementationOnce(async () => {
		mock.saved = [];
		return {
			transcription_complete: true,
			recovery_id: "fixture-recording",
			message: "Could not remove temporary files.",
		};
	});
	await render();
	await openSaved();
	await act(async () => button("Transcribe").click());
	await flush();
	expect(document.body.textContent).toContain(
		"Transcript saved; cleanup needs attention",
	);
	expect(document.body.textContent).not.toContain("No saved recordings found");
	expect(() => button("Transcribe")).toThrow();
	mock.api.recover.mockRejectedValueOnce(
		new Error("Recording pipeline is busy"),
	);
	await act(async () => button("Finish cleanup").click());
	await flush();
	expect(button("Finish cleanup")).toBeDefined();
	await act(async () => button("Finish cleanup").click());
	await flush();
	expect(document.body.textContent).not.toContain(
		"Could not remove temporary files",
	);
	expect(document.body.textContent).toContain("No saved recordings found");
	expect(mock.api.recover).toHaveBeenCalledTimes(3);
});
it("requires explicit confirmation before discarding a recoverable recording", async () => {
	mock.saved = ["fixture-recording"];
	await render();
	await openSaved();
	await act(async () => button("Discard").click());
	expect(mock.api.discardRecovery).not.toHaveBeenCalled();
	expect(document.body.textContent).toContain("This cannot be undone");
	await act(async () => button("Keep recording").click());
	expect(mock.api.discardRecovery).not.toHaveBeenCalled();
	await act(async () => button("Discard").click());
	await act(async () => button("Discard recording").click());
	await flush();
	expect(mock.api.discardRecovery).toHaveBeenCalledWith(
		"fixture-recording",
		expect.anything(),
	);
	expect(document.body.textContent).toContain("No saved recordings found");
});
it("preserves the recovery entry and error when confirmed discard fails", async () => {
	mock.saved = ["fixture-recording"];
	mock.api.discardRecovery.mockRejectedValueOnce(
		new Error("Could not remove saved recording"),
	);
	await render();
	await openSaved();
	await act(async () => button("Discard").click());
	await act(async () => button("Discard recording").click());
	await flush();
	expect(document.body.textContent).toContain(
		"Could not remove saved recording",
	);
	expect(document.body.textContent).toContain("Saved recording 1");
	expect(document.body.textContent).not.toContain("No saved recordings found");
});
it("unregisters a listener whose setup finishes after unmount", async () => {
	let finish: (stop: () => void) => void = () => {};
	mock.listen.mockImplementation((_name: string, callback: () => void) => {
		mock.onCancelled = callback;
		return new Promise<() => void>((resolve) => {
			finish = resolve;
		});
	});
	await render();
	await act(async () => root.unmount());
	unmounted = true;
	await act(async () => finish(mock.stopListening));
	expect(mock.stopListening).toHaveBeenCalledOnce();
	const calls = mock.api.listRecovery.mock.calls.length;
	await act(async () => mock.onCancelled?.());
	expect(mock.api.listRecovery.mock.calls).toHaveLength(calls);
});
