// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { audioFilesAPI } from "../lib/tauri/audioFiles";
import { recordingControlsAPI } from "../lib/tauri/commands";
import type {
	FileImportResult,
	RecordingPreferences,
} from "../lib/tauri/types";
import { FileTranscription } from "./FileTranscription";
import styles from "./FileTranscription.module.css";

vi.mock("../lib/tauri/audioFiles", async (original) => {
	const actual = await original<typeof import("../lib/tauri/audioFiles")>();
	return {
		...actual,
		audioFilesAPI: { choose: vi.fn(), listenForDrop: vi.fn() },
	};
});
vi.mock("../lib/tauri/commands", () => ({
	recordingControlsAPI: {
		getPreferences: vi.fn(),
		getState: vi.fn(),
		importFile: vi.fn(),
		recover: vi.fn(),
		cancel: vi.fn(),
	},
}));
vi.mock("./FileModelPicker", () => ({
	FileModelPicker: ({
		value,
		onChange,
		onReadyChange,
		disabled,
	}: {
		value: RecordingPreferences["meeting_model"];
		onChange: (value: RecordingPreferences["meeting_model"]) => void;
		onReadyChange: (value: boolean) => void;
		disabled: boolean;
	}) => {
		useEffect(() => onReadyChange(value !== null), [value, onReadyChange]);
		return (
			<button
				type="button"
				disabled={disabled}
				onClick={() =>
					onChange({
						provider: "openai",
						model: "speaker-model",
						use_managed: false,
					})
				}
			>
				Select speaker model
			</button>
		);
	},
}));
let host: HTMLDivElement;
let root: Root;
let client: QueryClient;
let onDrop: ((paths: string[]) => void) | undefined;
const unlisten = vi.fn();
const history = vi.fn();
const prefs: RecordingPreferences = {
	mode: "dictation",
	meeting_model: {
		provider: "openai",
		model: "regular-model",
		use_managed: false,
	},
};
async function settle() {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(2);
	});
}
async function render(active = true) {
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<MantineProvider env="test">
					<FileTranscription active={active} onOpenHistory={history} />
				</MantineProvider>
			</QueryClientProvider>,
		),
	);
	await settle();
}
function button(label: string) {
	return [...host.querySelectorAll("button")].find(
		(b) => b.textContent?.trim() === label,
	) as HTMLButtonElement | undefined;
}
async function click(label: string) {
	expect(button(label)).toBeDefined();
	await act(async () => button(label)?.click());
	await settle();
}
async function drop(paths: string[]) {
	await act(async () => onDrop?.(paths));
	await settle();
}
beforeEach(async () => {
	vi.useFakeTimers();
	vi.clearAllMocks();
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	client = new QueryClient({
		defaultOptions: { queries: { retry: false, staleTime: Infinity } },
	});
	client.setQueryData(["recording-preferences"], prefs);
	client.setQueryData(["home-recording-state"], "idle");
	vi.mocked(recordingControlsAPI.getPreferences).mockResolvedValue(prefs);
	vi.mocked(recordingControlsAPI.getState).mockResolvedValue("idle");
	vi.mocked(recordingControlsAPI.cancel).mockResolvedValue();
	vi.mocked(audioFilesAPI.listenForDrop).mockImplementation(
		async (callback) => {
			onDrop = callback;
			return unlisten;
		},
	);
	vi.mocked(recordingControlsAPI.importFile).mockResolvedValue({
		transcription_complete: true,
		recovery_id: null,
		message: null,
	});
	vi.mocked(recordingControlsAPI.recover).mockResolvedValue({
		transcription_complete: true,
		recovery_id: null,
		message: null,
	});
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	await render();
});
afterEach(async () => {
	await act(async () => root.unmount());
	client.clear();
	host.remove();
	vi.useRealTimers();
});
describe("File transcription", () => {
	it("shows drag feedback only while the recorder can accept a file", async () => {
		const hover = vi.mocked(audioFilesAPI.listenForDrop).mock.calls.at(-1)?.[1];
		expect(hover).toBeDefined();
		await act(async () => hover?.(true));
		expect(host.querySelector(`.${styles.dropActive}`)).not.toBeNull();
		expect(host.querySelector(".mantine-Paper-root")).toBeNull();
		await act(async () =>
			client.setQueryData(["home-recording-state"], "recording"),
		);
		await settle();
		expect(host.querySelector(`.${styles.dropActive}`)).toBeNull();
		expect(button("Choose file")?.disabled).toBe(true);
		await act(async () => hover?.(false));
		expect(host.querySelector(`.${styles.drop}`)).not.toBeNull();
	});
	it("starts with the drop zone rather than a redundant page header", () => {
		expect(host.querySelector("h1")).toBeNull();
		expect(host.querySelector(".mantine-Paper-root")).toBeNull();
		expect(host.textContent).toContain("Drop an audio file here");
		expect(button("Choose file")).not.toBeNull();
		expect(button("Transcribe")).not.toBeNull();
	});
	it("removes the selected file without uploading or changing recorder settings", async () => {
		await drop(["/meeting.wav"]);
		await act(async () =>
			host
				.querySelector<HTMLButtonElement>('[aria-label="Remove selected file"]')
				?.click(),
		);
		expect(host.textContent).toContain("Drop an audio file here");
		expect(button("Transcribe")?.disabled).toBe(true);
		expect(recordingControlsAPI.importFile).not.toHaveBeenCalled();
		expect(client.getQueryData(["recording-preferences"])).toEqual(prefs);
	});
	it("explains initial loading and pins late-loaded options for an already selected file", async () => {
		await act(async () => root.unmount());
		client.clear();
		let loadPreferences!: (value: RecordingPreferences) => void;
		let loadState!: (value: "idle") => void;
		vi.mocked(recordingControlsAPI.getPreferences).mockImplementationOnce(
			() =>
				new Promise((resolve) => {
					loadPreferences = resolve;
				}),
		);
		vi.mocked(recordingControlsAPI.getState).mockImplementationOnce(
			() =>
				new Promise((resolve) => {
					loadState = resolve;
				}),
		);
		root = createRoot(host);
		await render();
		expect(host.textContent).toContain("Checking recorder");
		expect(host.querySelectorAll('[role="status"]')).toHaveLength(1);
		expect(host.textContent).not.toContain("Uses your dictation model");
		expect(button("Transcribe")?.disabled).toBe(true);
		await act(async () => loadState("idle"));
		await settle();
		expect(host.textContent).not.toContain("Checking recorder");
		expect(host.textContent).toContain("Loading transcription options");
		await drop(["/meeting.wav"]);
		expect(button("Transcribe")?.disabled).toBe(true);
		await act(async () => loadPreferences(prefs));
		await settle();
		expect(host.textContent).not.toContain("Loading transcription options");
		await act(async () =>
			client.setQueryData(["recording-preferences"], {
				mode: "meeting",
				meeting_model: null,
			}),
		);
		await settle();
		await click("Transcribe");
		expect(recordingControlsAPI.importFile).toHaveBeenCalledExactlyOnceWith(
			"/meeting.wav",
			{ ...prefs, mode: "meeting" },
		);
	});
	it("uses current recorder defaults until a file or its options are selected", async () => {
		await render(false);
		const meeting: RecordingPreferences = {
			mode: "meeting",
			meeting_model: {
				provider: "local-whisper",
				model: "base",
				use_managed: false,
			},
		};
		await act(async () =>
			client.setQueryData(["recording-preferences"], meeting),
		);
		await render(true);

		await drop(["/meeting.wav"]);
		await click("Transcribe");
		expect(recordingControlsAPI.importFile).toHaveBeenCalledExactlyOnceWith(
			"/meeting.wav",
			meeting,
		);
	});
	it("pins a selected file's options when recorder defaults change elsewhere", async () => {
		await drop(["/dictation.wav"]);
		await render(false);
		await act(async () =>
			client.setQueryData(["recording-preferences"], {
				mode: "meeting",
				meeting_model: null,
			}),
		);
		await render(true);

		await click("Transcribe");
		expect(recordingControlsAPI.importFile).toHaveBeenCalledExactlyOnceWith(
			"/dictation.wav",
			{ ...prefs, mode: "meeting" },
		);
	});
	it("preserves an explicit model choice before selecting a file", async () => {
		await click("Select speaker model");
		await render(false);
		await act(async () =>
			client.setQueryData(["recording-preferences"], { ...prefs }),
		);
		await render(true);
		await drop(["/meeting.wav"]);
		await click("Transcribe");
		expect(recordingControlsAPI.importFile).toHaveBeenCalledWith(
			"/meeting.wav",
			{
				mode: "meeting",
				meeting_model: {
					provider: "openai",
					model: "speaker-model",
					use_managed: false,
				},
			},
		);
	});
	it("selects without uploading and starts only on explicit Transcribe", async () => {
		expect(button("Transcribe")?.disabled).toBe(true);
		await drop(["/private/meeting.wav"]);
		expect(host.textContent).toContain("meeting.wav");
		expect(host.textContent).not.toContain("/private/");
		expect(recordingControlsAPI.importFile).not.toHaveBeenCalled();
		await click("Transcribe");
		expect(recordingControlsAPI.importFile).toHaveBeenCalledExactlyOnceWith(
			"/private/meeting.wav",
			{ ...prefs, mode: "meeting" },
		);
		expect(host.textContent).toContain("Saved to History");
		await click("Open History");
		expect(history).toHaveBeenCalledOnce();
	});
	it("rejects multiple files and unsupported types without starting work", async () => {
		await drop(["/one.wav", "/two.wav"]);
		expect(host.textContent).toContain("Choose one supported audio file");
		await drop(["/secret.txt"]);
		expect(button("Transcribe")?.disabled).toBe(true);
		expect(recordingControlsAPI.importFile).not.toHaveBeenCalled();
	});
	it("uses the native picker without automatically uploading", async () => {
		vi.mocked(audioFilesAPI.choose).mockResolvedValue("C:\\Audio\\meeting.MP3");
		await click("Choose file");
		expect(host.textContent).toContain("meeting.MP3");
		expect(recordingControlsAPI.importFile).not.toHaveBeenCalled();
	});
	it("retries durable saved audio rather than importing and paying for completed chunks again", async () => {
		vi.mocked(recordingControlsAPI.importFile).mockResolvedValue({
			transcription_complete: false,
			recovery_id: "saved-id",
			message: "Connection failed",
		});
		await drop(["/meeting.wav"]);
		await click("Transcribe");
		expect(button("Retry saved audio")).toBeDefined();
		await click("Retry saved audio");
		expect(recordingControlsAPI.recover).toHaveBeenCalledExactlyOnceWith(
			"saved-id",
		);
		expect(recordingControlsAPI.importFile).toHaveBeenCalledOnce();
		expect(host.textContent).toContain("Saved to History");
	});
	it("ignores drops while busy and keeps job state across navigation", async () => {
		let finish!: (value: FileImportResult) => void;
		vi.mocked(recordingControlsAPI.importFile).mockImplementation(
			() =>
				new Promise((resolve) => {
					finish = resolve;
				}),
		);
		await drop(["/meeting.wav"]);
		await click("Transcribe");
		await drop(["/replacement.wav"]);
		expect(host.textContent).not.toContain("replacement.wav");
		await render(false);
		expect(unlisten).toHaveBeenCalled();
		await render(true);
		expect(host.textContent).toContain("Processing audio");
		await act(async () =>
			finish({
				recovery_id: null,
				message: null,
				transcription_complete: true,
			}),
		);
		await settle();
		expect(host.textContent).toContain("Saved to History");
	});
	it("retains an incomplete recovery result without claiming the transcript was saved", async () => {
		const incomplete = {
			transcription_complete: false,
			recovery_id: "saved-id",
			message: "Connection failed",
		};
		vi.mocked(recordingControlsAPI.importFile).mockResolvedValue(incomplete);
		vi.mocked(recordingControlsAPI.recover).mockResolvedValue(incomplete);
		await drop(["/meeting.wav"]);
		await click("Transcribe");
		await click("Retry saved audio");
		expect(button("Retry saved audio")).toBeDefined();
		expect(host.textContent).not.toContain("Saved to History");
		expect(button("Open History")).toBeUndefined();
		expect(button("Retry saved audio")?.disabled).toBe(false);
		expect(recordingControlsAPI.importFile).toHaveBeenCalledOnce();
	});
	it("recognizes a transcript saved by retry even when its cleanup still needs attention", async () => {
		vi.mocked(recordingControlsAPI.importFile).mockResolvedValue({
			transcription_complete: false,
			recovery_id: "saved-id",
			message: "Connection failed",
		});
		vi.mocked(recordingControlsAPI.recover).mockResolvedValueOnce({
			transcription_complete: true,
			recovery_id: "saved-id",
			message: "Could not remove saved recording files",
		});
		await drop(["/meeting.wav"]);
		await click("Transcribe");
		await click("Retry saved audio");
		expect(host.textContent).toContain(
			"Could not remove saved recording files",
		);
		expect(host.textContent).toContain("Saved to History");
		expect(button("Retry saved audio")).toBeUndefined();
		expect(button("Open History")).toBeDefined();
		await click("Finish cleanup");
		expect(recordingControlsAPI.recover).toHaveBeenCalledTimes(2);
		expect(recordingControlsAPI.importFile).toHaveBeenCalledOnce();
		expect(button("Finish cleanup")).toBeUndefined();
	});
	it("lets the user cancel preparation and leaves original file selected", async () => {
		vi.mocked(recordingControlsAPI.importFile).mockImplementation(
			() => new Promise(() => {}),
		);
		vi.mocked(recordingControlsAPI.cancel).mockResolvedValue();
		await drop(["/meeting.wav"]);
		await click("Transcribe");
		await click("Cancel");
		expect(recordingControlsAPI.cancel).toHaveBeenCalledOnce();
		expect(host.textContent).toContain("meeting.wav");
	});
	it("separates successful transcription from retrying failed local cleanup", async () => {
		vi.mocked(recordingControlsAPI.importFile).mockResolvedValue({
			transcription_complete: true,
			recovery_id: "cleanup-id",
			message: "Could not remove saved recording files",
		});
		await drop(["/meeting.wav"]);
		await click("Transcribe");
		expect(host.textContent).toContain(
			"Could not remove saved recording files",
		);
		expect(button("Retry saved audio")).toBeUndefined();
		expect(button("Open History")).toBeDefined();
		await click("Finish cleanup");
		expect(recordingControlsAPI.recover).toHaveBeenCalledExactlyOnceWith(
			"cleanup-id",
		);
		expect(recordingControlsAPI.importFile).toHaveBeenCalledOnce();
		expect(button("Finish cleanup")).toBeUndefined();
	});
	it("blocks a new job until an outstanding cancellation has settled", async () => {
		let finishImport!: (value: FileImportResult) => void;
		let finishCancel!: () => void;
		vi.mocked(recordingControlsAPI.importFile).mockImplementation(
			() =>
				new Promise((resolve) => {
					finishImport = resolve;
				}),
		);
		vi.mocked(recordingControlsAPI.cancel).mockImplementation(
			() =>
				new Promise((resolve) => {
					finishCancel = resolve;
				}),
		);
		await drop(["/meeting.wav"]);
		await click("Transcribe");
		await click("Cancel");
		await act(async () =>
			finishImport({
				transcription_complete: false,
				recovery_id: "saved-id",
				message: "Cancelled",
			}),
		);
		await settle();
		expect(button("Retry saved audio")?.disabled).toBe(true);
		await drop(["/replacement.wav"]);
		expect(host.textContent).not.toContain("replacement.wav");
		await act(async () => finishCancel());
		await settle();
		expect(button("Retry saved audio")?.disabled).toBe(false);
	});
	it("requires a model and keeps file choices separate from recorder settings", async () => {
		await act(async () =>
			client.setQueryData(["recording-preferences"], {
				mode: "dictation",
				meeting_model: null,
			}),
		);
		await settle();
		await drop(["/meeting.wav"]);
		expect(button("Transcribe")?.disabled).toBe(true);
		await click("Select speaker model");
		await click("Transcribe");
		expect(recordingControlsAPI.importFile).toHaveBeenCalledWith(
			"/meeting.wav",
			{
				mode: "meeting",
				meeting_model: {
					provider: "openai",
					model: "speaker-model",
					use_managed: false,
				},
			},
		);
		expect(client.getQueryData(["recording-preferences"])).toEqual({
			mode: "dictation",
			meeting_model: null,
		});
	});
	it("shows a reconnect action instead of silently disabling the page after a state failure", async () => {
		vi.mocked(recordingControlsAPI.getState).mockRejectedValue(
			new Error("Disconnected"),
		);
		await act(async () => {
			await client.invalidateQueries({ queryKey: ["home-recording-state"] });
		});
		await settle();
		expect(host.textContent).toContain("Recording controls unavailable");
		expect(button("Transcribe")?.disabled).toBe(true);
		vi.mocked(recordingControlsAPI.getState).mockResolvedValue("idle");
		await click("Reconnect");
		expect(host.textContent).not.toContain("Recording controls unavailable");
	});
});
