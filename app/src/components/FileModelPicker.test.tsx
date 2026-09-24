// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { RecordingPreferences } from "../lib/tauri/types";
import { FileModelPicker } from "./FileModelPicker";

const source = vi.hoisted(() => ({
	loading: false,
	fail: false,
	empty: false,
	refetch: vi.fn(),
}));
vi.mock("../lib/queries/transcriptionModels", () => ({
	useTranscriptionModels: () => ({
		loading: source.loading,
		failedSources: source.fail ? [{ query: { refetch: source.refetch } }] : [],
		options: new Map(
			(source.empty
				? []
				: [
						{
							value: "openai::whisper-1::byok",
							label: "Whisper · OpenAI · Your key",
							provider: "openai",
							model: "whisper-1",
							use_managed: false,
						},
						{
							value: "openai::gpt-4o-transcribe-diarize::managed",
							label: "Speakers · OpenAI · Managed",
							provider: "openai",
							model: "gpt-4o-transcribe-diarize",
							use_managed: true,
						},
					]
			).map((model) => [model.value, model]),
		),
	}),
}));
let host: HTMLDivElement;
let root: Root;
const ready = vi.fn(),
	change = vi.fn();
function Harness({
	initial = null,
	disabled = false,
}: {
	initial?: RecordingPreferences["meeting_model"];
	disabled?: boolean;
}) {
	const [value, setValue] = useState(initial);
	return (
		<FileModelPicker
			value={value}
			disabled={disabled}
			onReadyChange={ready}
			onChange={(next) => {
				change(next);
				setValue(next);
			}}
		/>
	);
}
async function render(
	initial: RecordingPreferences["meeting_model"] = null,
	disabled = false,
) {
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<Harness initial={initial} disabled={disabled} />
			</MantineProvider>,
		),
	);
}
async function open() {
	await act(async () =>
		host
			.querySelector<HTMLInputElement>(
				'input[aria-label="Transcription model"]',
			)
			?.click(),
	);
}
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	source.loading = false;
	source.empty = false;
	source.fail = false;
	source.refetch.mockReset();
	ready.mockClear();
	change.mockClear();
});
afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
});
it("requires explicit choices, filters speaker models, and preserves managed routing", async () => {
	await render();
	expect(ready).toHaveBeenLastCalledWith(false);
	await open();
	expect(
		[...document.querySelectorAll('[role="option"]')].map(
			(el) => el.textContent,
		),
	).toEqual(["Whisper · OpenAI · Your key"]);
	await act(async () =>
		document.querySelector<HTMLElement>('[role="option"]')?.click(),
	);
	expect(change).toHaveBeenLastCalledWith({
		provider: "openai",
		model: "whisper-1",
		use_managed: false,
	});
	expect(ready).toHaveBeenLastCalledWith(true);
	await act(async () =>
		host
			.querySelector<HTMLButtonElement>(
				'[aria-label="Clear transcription model"]',
			)
			?.click(),
	);
	expect(change).toHaveBeenLastCalledWith(null);
	expect(ready).toHaveBeenLastCalledWith(false);
	await act(async () =>
		host.querySelector<HTMLInputElement>('input[type="checkbox"]')?.click(),
	);
	expect(change).toHaveBeenLastCalledWith(null);
	expect(ready).toHaveBeenLastCalledWith(false);
	await open();
	expect(
		[...document.querySelectorAll('[role="option"]')].map(
			(el) => el.textContent,
		),
	).toEqual(["Speakers · OpenAI · Managed"]);
	await act(async () =>
		document.querySelector<HTMLElement>('[role="option"]')?.click(),
	);
	expect(change).toHaveBeenLastCalledWith({
		provider: "openai",
		model: "gpt-4o-transcribe-diarize",
		use_managed: true,
	});
	expect(ready).toHaveBeenLastCalledWith(true);
});
it("marks a retired selection unavailable without substituting another model", async () => {
	await render({ provider: "openai", model: "retired", use_managed: true });
	expect(ready).toHaveBeenLastCalledWith(false);
	expect(host.textContent).toContain("no longer available");
	expect(change).not.toHaveBeenCalled();
});
it("distinguishes loading, empty and failed sources and offers retry", async () => {
	source.loading = true;
	source.empty = true;
	await render();
	expect(
		host.querySelector('input[placeholder="Loading models…"]'),
	).not.toBeNull();
	expect(host.textContent).not.toContain("Add a provider");
	source.loading = false;
	await render();
	expect(host.textContent).toContain("Add a provider");
	await act(async () =>
		host.querySelector<HTMLInputElement>('input[type="checkbox"]')?.click(),
	);
	expect(host.textContent).toContain("No speaker-label model");
	source.fail = true;
	await render();
	expect(host.textContent).toContain("couldn’t load");
	await act(async () =>
		[...host.querySelectorAll("button")]
			.find((el) => el.textContent === "Retry")
			?.click(),
	);
	expect(source.refetch).toHaveBeenCalledOnce();
});
it("reflects a saved diarization choice and locks options during processing", async () => {
	await render(
		{
			provider: "openai",
			model: "gpt-4o-transcribe-diarize",
			use_managed: true,
		},
		true,
	);
	expect(
		host.querySelector<HTMLInputElement>('input[type="checkbox"]')?.checked,
	).toBe(true);
	expect(
		host.querySelector<HTMLInputElement>('input[type="checkbox"]')?.disabled,
	).toBe(true);
	expect(
		host.querySelector<HTMLInputElement>(
			'input[aria-label="Transcription model"]',
		)?.disabled,
	).toBe(true);
});
