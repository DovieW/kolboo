// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { RecordingPreferences } from "../lib/tauri/types";
import { MeetingModelDialog } from "./MeetingModelDialog";

const state = vi.hoisted(() => ({
	managed: false,
	openai: false,
	custom: false,
	loadingProviders: false,
	providerError: false,
	catalogError: false,
	refetchProviders: vi.fn(),
	refetchCatalog: vi.fn(),
}));
vi.mock("../lib/queries/modelChoices", () => ({
 useModelChoices: () => {
  const choices: import("../lib/modelSelection").ModelChoice[] = [];
  const add=(source:"byok"|"managed",provider:string,model:string,name:string)=>choices.push({selection:{source,provider_id:provider,model_id:model},displayName:name,providerName:provider === "custom_test" ? "My server" : provider === "local-whisper" ? "Local Whisper" : "OpenAI",ready:true,capabilities:["After recording",...(model === "gpt-4o-transcribe-diarize" ? ["Speaker labels"]:[])]});
  if (!state.loadingProviders && !state.providerError) {
   if (state.custom) add("byok","custom_test","my-transcriber","my-transcriber");
   else if (state.openai) {add("byok","openai","whisper-1","Whisper");add("byok","openai","gpt-4o-transcribe-diarize","GPT-4o Transcribe · speaker labels");}
   else if (!state.managed) {add("byok","local-whisper","base","Base");add("byok","local-whisper","small","Small");}
  }
  if (state.managed && state.openai && !state.catalogError) add("managed","openai","gpt-4o-transcribe-diarize","Speaker model");
  const failedSources=[{label:"provider list",query:{isError:state.providerError,isFetching:false,refetch:state.refetchProviders}},{label:"managed models",query:{isError:state.managed && state.catalogError,isFetching:false,refetch:state.refetchCatalog}}].filter(({query})=>query.isError);
  return {choices,loading:state.loadingProviders,failedSources,managedMessage:state.managed?undefined:"Sign in for Managed",retry:()=>{void state.refetchProviders(); if(state.managed) void state.refetchCatalog();}};
 },
}));
let host: HTMLDivElement;
let root: Root;
const save = vi.fn();
beforeEach(() => {
	state.managed = false;
	state.openai = false;
	state.custom = false;
	state.loadingProviders = false;
	state.providerError = false;
	state.catalogError = false;
	state.refetchProviders.mockReset().mockResolvedValue({});
	state.refetchCatalog.mockReset().mockResolvedValue({});
	save.mockClear();
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
});
afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
});
async function render(
	preferences: RecordingPreferences = { mode: "meeting", meeting_model: null },
	error?: string,
) {
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<MeetingModelDialog
					preferences={preferences}
					onSave={save}
					onClose={() => {}}
					saving={false}
					error={error}
				/>
			</MantineProvider>,
		),
	);
}
it("lets meetings select another downloaded local model without changing dictation", async () => {
	await render();
	await act(async () =>
		(
			document.querySelector('button[aria-label="Model"]') as HTMLElement
		).click(),
	);
	const options = [...document.querySelectorAll('[role="option"]')].filter(
		(option) => option.getAttribute("value")?.startsWith("["),
	);
	expect(
		options.map((option) => JSON.parse(option.getAttribute("value") ?? "null")),
	).toEqual([
		["byok", "local-whisper", "base"],
		["byok", "local-whisper", "small"],
	]);
	await act(async () => (options[1] as HTMLElement).click());
	await act(async () =>
		[...document.querySelectorAll("button")]
			.find((button) => button.textContent === "Save")
			?.click(),
	);
	expect(save).toHaveBeenCalledWith({
		mode: "meeting",
		language_hint: "auto",
		meeting_model: {
			provider: "local-whisper",
			model: "small",
			use_managed: false,
		},
	});
});

it("shows loading instead of claiming that an unfinished provider list is empty", async () => {
	state.loadingProviders = true;
	await render();
	expect(document.body.textContent).toContain("Loading models");
	expect(document.body.textContent).not.toContain("No models are available");
	expect(
		[...document.querySelectorAll("button")].find(
			(button) => button.textContent === "Save",
		)?.disabled,
	).toBe(true);
	expect(save).not.toHaveBeenCalled();
});

it("starts at auto and saves a selected language together with the model", async () => {
	await render({
		mode: "meeting",
		meeting_model: {
			provider: "local-whisper",
			model: "base",
			use_managed: false,
		},
	});
	const input = document.querySelector<HTMLInputElement>(
		'[aria-label="Meeting language hint"]',
	);
	expect(input?.value).toBe("Auto-detect");
	await act(async () => input?.click());
	const option = [
		...document.querySelectorAll<HTMLElement>('[role="option"]'),
	].find((element) => element.textContent === "Yiddish");
	expect(option).toBeDefined();
	await act(async () => option?.click());
	expect(input?.value).toBe("Yiddish");
	expect(save).not.toHaveBeenCalled();
	await act(async () =>
		[...document.querySelectorAll("button")]
			.find((b) => b.textContent === "Save")
			?.click(),
	);
	expect(save).toHaveBeenCalledWith({
		mode: "meeting",
		meeting_model: {
			provider: "local-whisper",
			model: "base",
			use_managed: false,
		},
		language_hint: "yi",
	});
});

it("retains an existing explicit meeting hint", async () => {
	await render({ mode: "meeting", meeting_model: null, language_hint: "he" });
	expect(
		document.querySelector<HTMLInputElement>(
			'[aria-label="Meeting language hint"]',
		)?.value,
	).toBe("Hebrew");
});

it("saves a custom meeting model as BYOK even with managed access", async () => {
	state.custom = true;
	state.managed = true;
	await render();
	await act(async () =>
		(
			document.querySelector('button[aria-label="Model"]') as HTMLElement
		).click(),
	);
	const option = document.querySelector('[role="option"]') as HTMLElement;
	expect(option.textContent).toContain("my-transcriber");
	expect(option.textContent).toContain("My server");
	await act(async () => option.click());
	await act(async () =>
		[...document.querySelectorAll("button")]
			.find((b) => b.textContent === "Save")
			?.click(),
	);
	expect(save).toHaveBeenCalledWith({
		mode: "meeting",
		language_hint: "auto",
		meeting_model: {
			provider: "custom_test",
			model: "my-transcriber",
			use_managed: false,
		},
	});
});

it("offers a targeted retry after a provider-list failure", async () => {
	state.providerError = true;
	await render();
	expect(document.body.textContent).toContain("Couldn't load provider list");
	expect(document.body.textContent).not.toContain("No models are available");
	await act(async () =>
		[...document.querySelectorAll("button")]
			.find((button) => button.textContent === "Retry loading models")
			?.click(),
	);
	expect(state.refetchProviders).toHaveBeenCalledOnce();
	expect(state.refetchCatalog).not.toHaveBeenCalled();
});

it("keeps BYOK models usable when the managed catalog fails, without falling back to bundled managed models", async () => {
	state.managed = true;
	state.openai = true;
	state.catalogError = true;
	await render();
	expect(document.body.textContent).toContain("Couldn't load managed models");
	await act(async () =>
		(
			document.querySelector('button[aria-label="Model"]') as HTMLElement
		).click(),
	);
	const options = [...document.querySelectorAll('[role="option"]')].filter(
		(option) => option.getAttribute("value")?.startsWith("["),
	);
	expect(
		options.some(
			(option) =>
				JSON.parse(option.getAttribute("value") ?? "null")?.[0] === "managed",
		),
	).toBe(false);
	const byok = options.find((option) =>
		option.textContent?.includes("speaker labels"),
	);
	expect(byok).toBeDefined();
	await act(async () => (byok as HTMLElement).click());
	await act(async () =>
		[...document.querySelectorAll("button")]
			.find((button) => button.textContent === "Save")
			?.click(),
	);
	expect(save).toHaveBeenCalledWith(
		expect.objectContaining({
			meeting_model: expect.objectContaining({ use_managed: false }),
		}),
	);
});

it("keeps an unavailable saved choice unchanged until another model is explicitly selected", async () => {
	state.managed = true;
	await render({
		mode: "meeting",
		meeting_model: {
			provider: "openai",
			model: "retired-model",
			use_managed: true,
		},
	});
	expect(document.body.textContent).toContain(
		"Your saved model is no longer available",
	);
	expect(
		[...document.querySelectorAll("button")].find(
			(button) => button.textContent === "Save",
		)?.disabled,
	).toBe(true);
	expect(save).not.toHaveBeenCalled();
});

it("keeps a save failure beside the model selector", async () => {
	await render(undefined, "Local settings could not be saved");
	expect(document.body.textContent).toContain("Could not save model");
	expect(document.body.textContent).toContain(
		"Local settings could not be saved",
	);
});
it("does not silently seed or enable a model when Edge publishes an empty managed catalog", async () => {
	state.managed = true;
	await render();
	expect(document.body.textContent).toContain("No models are available");
	expect(
		[...document.querySelectorAll("button")].find(
			(button) => button.textContent === "Save",
		)?.disabled,
	).toBe(true);
	expect(save).not.toHaveBeenCalled();
});

it("keeps managed and user-key choices distinct without depending on dictation settings", async () => {
	state.managed = true;
	state.openai = true;
	await render();
	const select = document.querySelector(
		'button[aria-label="Model"]',
	) as HTMLElement;
	await act(async () => select.click());
	await act(async () =>
		[...document.querySelectorAll<HTMLButtonElement>('[role="tab"]')]
			.find((el) => el.textContent === "Kolboo Managed")
			?.click(),
	);
	await act(async () =>
		(document.querySelector('[role="option"]') as HTMLElement).click(),
	);
	const submit = () =>
		[...document.querySelectorAll("button")]
			.find((button) => button.textContent === "Save")
			?.click();
	await act(async () => submit());
	expect(save).toHaveBeenLastCalledWith({
		mode: "meeting",
		language_hint: "auto",
		meeting_model: {
			provider: "openai",
			model: "gpt-4o-transcribe-diarize",
			use_managed: true,
		},
	});
	await act(async () => select.click());
	await act(async () =>
		[...document.querySelectorAll<HTMLButtonElement>('[role="tab"]')]
			.find((el) => el.textContent === "Your models")
			?.click(),
	);
	await act(async () =>
		(
			[...document.querySelectorAll('[role="option"]')].find((option) =>
				option.textContent?.includes("speaker labels"),
			) as HTMLElement
		).click(),
	);
	await act(async () => submit());
	expect(save).toHaveBeenLastCalledWith({
		mode: "meeting",
		language_hint: "auto",
		meeting_model: {
			provider: "openai",
			model: "gpt-4o-transcribe-diarize",
			use_managed: false,
		},
	});
});
