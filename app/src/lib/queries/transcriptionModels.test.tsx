import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, expect, it, vi } from "vitest";
import { useTranscriptionModels } from "./transcriptionModels";

const queries = vi.hoisted(() => ({
	providers: {} as Record<string, unknown>,
	local: {} as Record<string, unknown>,
	catalog: {} as Record<string, unknown>,
	auth: {} as Record<string, unknown>,
}));
vi.mock("./providers", () => ({
	useAvailableProviders: () => queries.providers,
	useWhisperModels: () => queries.local,
	useManagedModels: () => queries.catalog,
}));
vi.mock("./license", () => ({ useLicenseAuthContext: () => queries.auth }));
function read() {
	let result!: ReturnType<typeof useTranscriptionModels>;
	function Reader() {
		result = useTranscriptionModels();
		return null;
	}
	renderToStaticMarkup(<Reader />);
	return result;
}
beforeEach(() => {
	queries.providers = { data: { stt: [] } };
	queries.local = {};
	queries.catalog = {};
	queries.auth = { data: { authenticated: false } };
});
it("offers only batch models and downloaded local models, deduplicating shared entries", () => {
	queries.providers = {
		data: {
			stt: [
				{
					value: "local-whisper",
					label: "Local",
					models: ["base"],
					is_local: true,
				},
				{
					value: "custom_host",
					label: "Local server",
					models: ["speech"],
					is_local: true,
				},
				{
					value: "openai",
					label: "OpenAI",
					models: ["whisper-1", "whisper-1", "gpt-4o-realtime-transcribe"],
				},
				{ value: "unknown", label: "Unknown" },
			],
		},
	};
	queries.local = {
		data: [
			{ id: "base", name: "Base", is_downloaded: true },
			{ id: "large", name: "Large", is_downloaded: false },
		],
	};
	const result = read();
	expect([...result.options.keys()]).toEqual([
		"local-whisper::base::byok",
		"custom_host::speech::byok",
		"openai::whisper-1::byok",
	]);
	expect(result.options.get("custom_host::speech::byok")?.label).toBe(
		"speech · Local server",
	);
	expect(result.loading).toBeFalsy();
});
it("does not use failed or absent managed catalogs as permission to enable bundled models", () => {
	queries.auth = {
		data: {
			authenticated: true,
			policy_status: "allow",
			entitlements: ["managed_inference"],
		},
	};
	queries.catalog = { isSuccess: true };
	expect(read().options.size).toBe(0);
	queries.catalog = { isPending: true };
	expect(read().loading).toBe(true);
	queries.catalog = {
		isSuccess: true,
		data: [
			{
				provider: "openai",
				id: "text",
				display_name: "Text",
				capabilities: ["chat_completions"],
			},
			{
				provider: "openai",
				id: "speech",
				display_name: "Speech",
				capabilities: ["transcription"],
			},
		],
	};
	expect([...read().options.keys()]).toEqual(["openai::speech::managed"]);
	queries.catalog = {
		isError: true,
		data: [
			{ provider: "openai", id: "speech", capabilities: ["transcription"] },
		],
	};
	expect(read().options.size).toBe(0);
	expect(read().failedSources.map((source) => source.label)).toEqual([
		"managed models",
	]);
});
it("handles missing provider/local data and surfaces only enabled failures", () => {
	queries.providers = { isPending: true };
	expect(read().loading).toBe(true);
	queries.providers = {
		data: { stt: [{ value: "whisper", label: "Whisper" }] },
	};
	queries.local = { isPending: true };
	expect(read().loading).toBe(true);
	expect(read().options.size).toBe(0);
	queries.local = { isError: true };
	queries.auth = { isError: true };
	queries.providers = { ...queries.providers, isError: true };
	expect(read().failedSources.map((source) => source.label)).toEqual([
		"provider list",
		"account access",
		"local models",
	]);
});
