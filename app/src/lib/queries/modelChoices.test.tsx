import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, expect, it, vi } from "vitest";
import { type ModelPurpose, useModelChoices } from "./modelChoices";

const fixture = vi.hoisted(() => ({
	providers: {
		data: undefined,
		isPending: false,
		isError: false,
		refetch: vi.fn(),
	} as Record<string, unknown>,
	auth: {
		data: undefined,
		isPending: false,
		isError: false,
		refetch: vi.fn(),
	} as Record<string, unknown>,
	managed: {
		data: [],
		isPending: false,
		isError: false,
		isSuccess: true,
		refetch: vi.fn(),
	} as Record<string, unknown>,
	byok: { data: {} } as Record<string, unknown>,
	ollama: { data: [], refetch: vi.fn() } as Record<string, unknown>,
	whisper: { data: [], refetch: vi.fn() } as Record<string, unknown>,
	enabled: false,
	query: vi.fn(),
	managedHook: vi.fn(),
	byokHook: vi.fn(),
	ollamaHook: vi.fn(),
	whisperHook: vi.fn(),
}));
vi.mock("@tanstack/react-query", () => ({
	useQuery: (options: unknown) => {
		fixture.query(options);
		return fixture.providers;
	},
}));
vi.mock("../tauri", () => ({
	configAPI: { getAvailableProviders: vi.fn() },
	hasManagedInferenceAccess: () => fixture.enabled,
}));
vi.mock("./license", () => ({ useLicenseAuthContext: () => fixture.auth }));
vi.mock("./providers", () => ({
	useManagedModels: (enabled: boolean) => {
		fixture.managedHook(enabled);
		return fixture.managed;
	},
	useByokLlmModels: (enabled: boolean) => {
		fixture.byokHook(enabled);
		return fixture.byok;
	},
	useOllamaModels: (enabled: boolean) => {
		fixture.ollamaHook(enabled);
		return fixture.ollama;
	},
	useWhisperModels: (enabled: boolean) => {
		fixture.whisperHook(enabled);
		return fixture.whisper;
	},
}));
function read(purpose: ModelPurpose) {
	let result!: ReturnType<typeof useModelChoices>;
	function Reader() {
		result = useModelChoices(purpose);
		return null;
	}
	renderToStaticMarkup(<Reader />);
	return result;
}
beforeEach(() => {
	vi.clearAllMocks();
	fixture.providers.data = undefined;
	fixture.providers.isPending = false;
	fixture.providers.isError = false;
	fixture.enabled = false;
	fixture.auth.isPending = false;
	fixture.auth.isError = false;
	fixture.managed.data = [];
	fixture.managed.isPending = false;
	fixture.managed.isError = false;
	fixture.managed.isSuccess = true;
	fixture.byok.data = {};
	fixture.ollama.data = [];
	fixture.whisper.data = [];
});

it("presents ivritAI only as Managed Hebrew batch transcription without claiming free pricing", () => {
	fixture.enabled = true;
	fixture.managed.data = [
		{
			id: "ivrit-ai/whisper-large-v3-turbo",
			display_name: "ivritAI Hebrew Turbo",
			provider: "ivritai",
			capabilities: ["transcription"],
		},
	];
	for (const purpose of ["dictation", "file", "meeting"] as const) {
		const choices = read(purpose).choices;
		expect(choices).toHaveLength(1);
		const choice = choices[0];
		if (!choice) throw new Error("Missing ivritAI choice");
		expect(choice).toMatchObject({
			providerName: "ivritAI",
			displayName: "ivritAI Hebrew Turbo",
			selection: {
				source: "managed",
				provider_id: "ivritai",
				model_id: "ivrit-ai/whisper-large-v3-turbo",
			},
			capabilities: ["After recording"],
		});
		expect(choice.limitations).toContain("Auto uses Hebrew");
		expect(choice.limitations).toContain("host being online");
		expect(choice.pricing).toContain("not assumed free");
	}
	expect(read("llm").choices).toEqual([]);
	fixture.enabled = false;
	expect(read("dictation").choices).toEqual([]);
});

it("uses separate YiddishLabs key readiness and restricts Standard to uploaded files and meetings", () => {
	fixture.providers.data = {
		stt: [
			{
				value: "yiddishlabs",
				label: "YiddishLabs",
				configured: true,
				model_configured: {
					"yiddishlabs-3-turbo": true,
					"yiddishlabs-standard": false,
				},
			},
		],
	};
	fixture.enabled = true;
	fixture.managed.data = [
		{
			id: "yiddishlabs-3-turbo",
			display_name: "Flash Turbo",
			provider: "yiddishlabs",
			capabilities: ["transcription"],
		},
		{
			id: "yiddishlabs-standard",
			display_name: "Standard",
			provider: "yiddishlabs",
			capabilities: ["transcription_jobs"],
		},
	];
	const dictation = read("dictation").choices;
	expect(dictation.map((choice) => choice.selection.model_id)).toEqual([
		"yiddishlabs-3-turbo",
		"yiddishlabs-3-turbo",
	]);
	for (const purpose of ["file", "meeting"] as const) {
		const choices = read(purpose).choices;
		expect(choices).toHaveLength(4);
		const byokStandard = choices.find(
			(choice) =>
				choice.selection.source === "byok" &&
				choice.selection.model_id === "yiddishlabs-standard",
		)!;
		expect(byokStandard.ready).toBe(false);
		expect(byokStandard.capabilities).toContain("File/meeting job");
		expect(byokStandard.limitations).toContain(
			"stores submitted jobs and content",
		);
		expect(byokStandard.pricing).toBeUndefined();
		const managedStandard = choices.find(
			(choice) =>
				choice.selection.source === "managed" &&
				choice.selection.model_id === "yiddishlabs-standard",
		)!;
		expect(managedStandard.ready).toBe(true);
		expect(managedStandard.limitations).toContain(
			"does not cancel provider processing or charges",
		);
		expect(
			choices
				.filter((choice) => choice.selection.model_id === "yiddishlabs-3-turbo")
				.every((choice) =>
					choice.limitations?.includes("provider's retention description"),
				),
		).toBe(true);
	}
	fixture.providers.data = {
		llm: [],
		stt: [
			{
				value: "yiddishlabs",
				label: "YiddishLabs",
				configured: true,
				model_configured: { "yiddishlabs-standard": true },
			},
		],
	};
	const direct = read("file").choices.filter(
		(choice) => choice.selection.source === "byok",
	);
	expect(direct.map((choice) => choice.ready)).toEqual([false, true]);
	expect(read("llm").choices).toEqual([]);
});

it("lists keyless providers with setup guidance and filters only realtime-only models for files/meetings", async () => {
	const { configAPI } = await import("../tauri");
	fixture.providers.data = {
		stt: [
			{ value: "openai", label: "OpenAI", configured: false },
			{ value: "deepgram", label: "Deepgram", configured: true },
			{
				value: "custom_a",
				label: "Custom",
				configured: true,
				models: ["raw"],
				model_names: { raw: "Friendly" },
			},
			{ value: "unknown", label: "Unknown", configured: false },
		],
	};
	const dictation = read("dictation");
	const custom = dictation.choices.find(
		(choice) => choice.selection.provider_id === "custom_a",
	);
	expect(custom).toMatchObject({
		displayName: "Friendly",
		ready: true,
		selection: { source: "byok", model_id: "raw" },
	});
	expect(
		dictation.choices.some((choice) =>
			choice.capabilities.includes("Realtime input"),
		),
	).toBe(true);
	expect(
		dictation.choices
			.filter((choice) => choice.selection.provider_id === "openai")
			.every(
				(choice) =>
					!choice.ready && choice.reason?.includes("secure provider setup"),
			),
	).toBe(true);
	expect(
		dictation.choices.filter((choice) =>
			choice.limitations?.includes("Realtime-only"),
		).length,
	).toBeGreaterThan(0);
	for (const purpose of ["file", "meeting"] as const) {
		const batch = read(purpose);
		expect(
			batch.choices.every((choice) =>
				choice.capabilities.includes("After recording"),
			),
		).toBe(true);
		expect(
			batch.choices.some(
				(choice) =>
					choice.selection.provider_id === "deepgram" &&
					choice.capabilities.includes("Realtime input"),
			),
		).toBe(true);
		expect(batch.choices.length).toBeLessThan(dictation.choices.length);
	}
	const options = fixture.query.mock.calls[0]?.[0] as {
		queryKey: string[];
		queryFn: () => unknown;
	};
	expect(options.queryKey).toEqual(["availableProviders", "picker"]);
	await options.queryFn();
	expect(configAPI.getAvailableProviders).toHaveBeenCalledWith(true);
	expect(fixture.managedHook).toHaveBeenCalledWith(false);
	expect(fixture.byokHook).toHaveBeenCalledWith(false);
});

it("represents local downloads/endpoint readiness independently of Managed and never invents pricing", () => {
	fixture.providers.data = {
		stt: [
			{ value: "whisper", label: "Local Whisper", is_local: true },
			{ value: "local-whisper", label: "Local alias", is_local: true },
		],
		llm: [
			{ value: "ollama", label: "Ollama", configured: true },
			{ value: "missing", label: "Missing", configured: false },
		],
	};
	fixture.whisper.data = [
		{ id: "downloaded", name: "Downloaded", is_downloaded: true },
		{ id: "missing", name: "Missing", is_downloaded: false },
	];
	const local = read("dictation");
	expect(
		local.choices.map((choice) => [choice.selection.provider_id, choice.ready]),
	).toEqual([
		["local-whisper", true],
		["local-whisper", false],
		["local-whisper", true],
		["local-whisper", false],
	]);
	expect(local.choices[1]?.reason).toContain("Download");
	local.retry();
	expect(fixture.whisper.refetch).toHaveBeenCalledOnce();
	fixture.ollama.data = [{ value: "raw-local", label: "Local" }];
	fixture.byok.data = { missing: [{ value: "missing", label: "Missing" }] };
	const llm = read("llm");
	expect(llm.choices[0]).toMatchObject({
		displayName: "Local",
		ready: true,
		capabilities: ["Text generation"],
	});
	expect(llm.choices[1]?.ready).toBe(false);
	expect(llm.choices.every((choice) => choice.pricing === undefined)).toBe(
		true,
	);
	llm.retry();
	expect(fixture.ollama.refetch).toHaveBeenCalledOnce();
	expect(fixture.ollamaHook).toHaveBeenCalledWith(true);
	expect(fixture.whisperHook).toHaveBeenCalledWith(false);
	fixture.providers.data = {
		llm: [{ value: "ollama", label: "Ollama", configured: false }],
	};
	fixture.ollama.data = undefined;
	const unconfigured = read("llm");
	expect(unconfigured.setupProviders).toEqual([
		{ provider_id: "ollama", label: "Ollama" },
	]);
	expect(unconfigured.choices).toEqual([]);
	unconfigured.retry();
	expect(fixture.ollama.refetch).toHaveBeenCalledOnce();
	fixture.providers.data = { stt: [{ value: "whisper", label: "Whisper" }] };
	fixture.whisper.data = undefined;
	expect(read("dictation").setupProviders).toEqual([
		{ provider_id: "local-whisper", label: "Whisper" },
	]);
	fixture.providers.data = undefined;
	fixture.ollama.data = undefined;
	fixture.whisper.data = undefined;
	expect(read("llm").choices).toEqual([]);
	expect(read("dictation").choices).toEqual([]);
});

it("shows valid price estimates only and limits Managed models to the action's capabilities", () => {
	fixture.providers.data = {
		llm: [{ value: "prices", label: "Prices", configured: true }],
		stt: [],
	};
	fixture.byok.data = {
		prices: [
			{
				value: "known",
				label: "Known",
				pricing: { input_per_million: 1, output_per_million: 2 },
			},
			{
				value: "zero",
				label: "Known zero",
				pricing: { input_per_million: 0, output_per_million: 0 },
			},
			...[
				null,
				{},
				{ input_per_million: "1", output_per_million: 2 },
				{ input_per_million: NaN, output_per_million: 2 },
				{ input_per_million: -1, output_per_million: 2 },
				{ input_per_million: 1, output_per_million: "2" },
				{ input_per_million: 1, output_per_million: Infinity },
				{ input_per_million: 1, output_per_million: -2 },
			].map((pricing, index) => ({
				value: String(index),
				label: "Unknown",
				pricing,
			})),
		],
	};
	fixture.enabled = true;
	fixture.managed.data = undefined;
	expect(read("llm").managedMessage).toContain("No Managed models");
	fixture.managed.data = [
		{
			id: "text",
			provider: "openai",
			display_name: "Text",
			capabilities: ["chat_completions"],
		},
		{
			id: "response",
			provider: "openai",
			display_name: "Responses",
			capabilities: ["responses"],
		},
		{
			id: "speech",
			provider: "groq",
			display_name: "Speech",
			capabilities: ["transcription"],
		},
		{
			id: "gpt-4o-transcribe-diarize",
			provider: "openai",
			display_name: "Speakers",
			capabilities: ["transcription"],
		},
		{
			id: "embedding",
			provider: "openai",
			display_name: "Embedding",
			capabilities: ["embedding"],
		},
	];
	const llm = read("llm");
	expect(llm.choices[0]?.pricing).toContain("Input $1/1M");
	expect(llm.choices[1]?.pricing).toContain("Input $0/1M");
	expect(
		llm.choices.slice(2, 10).every((choice) => choice.pricing === undefined),
	).toBe(true);
	expect(
		llm.choices
			.filter((choice) => choice.selection.source === "managed")
			.map((choice) => choice.selection.model_id),
	).toEqual(["text"]);
	expect(llm.managedMessage).toBeUndefined();
	llm.retry();
	expect(fixture.managed.refetch).toHaveBeenCalledOnce();
	const stt = read("file");
	expect(stt.choices).toHaveLength(2);
	expect(stt.choices[1]?.capabilities).toContain("Speaker labels");
	expect(stt.choices[0]).toMatchObject({
		selection: { source: "managed", model_id: "speech" },
		capabilities: ["After recording"],
		ready: true,
	});
	expect(stt.choices[0]?.pricing).toContain("not assumed free");
});

it("keeps BYOK available across all Managed loading, sign-in and retry states", () => {
	fixture.providers.data = {
		stt: [
			{ value: "custom_a", label: "Custom", models: ["raw"], configured: true },
		],
		llm: [],
	};
	const states = [
		[() => (fixture.auth.isPending = true), "Checking account"],
		[
			() => {
				fixture.auth.isPending = false;
				fixture.auth.isError = true;
			},
			"Account access could not",
		],
		[() => (fixture.auth.isError = false), "Sign in"],
		[
			() => {
				fixture.enabled = true;
				fixture.managed.isPending = true;
				fixture.managed.isSuccess = false;
			},
			"Loading Managed",
		],
		[
			() => {
				fixture.managed.isPending = false;
				fixture.managed.isError = true;
			},
			"Managed models could not",
		],
		[
			() => {
				fixture.managed.isError = false;
				fixture.managed.isSuccess = true;
			},
			"No Managed models",
		],
	] as const;
	for (const [set, expected] of states) {
		set();
		const result = read("dictation");
		expect(result.managedMessage).toContain(expected);
		expect(result.choices[0]?.ready).toBe(true);
		result.retry();
	}
	expect(fixture.providers.refetch).toHaveBeenCalledTimes(6);
	expect(fixture.auth.refetch).toHaveBeenCalledTimes(6);
	expect(fixture.managed.refetch).toHaveBeenCalledTimes(3);
	fixture.providers.isError = true;
	fixture.providers.isPending = true;
	const result = read("dictation");
	expect(result.loading && result.error).toBe(true);
});
