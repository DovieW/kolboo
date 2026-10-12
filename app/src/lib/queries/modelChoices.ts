import { useQuery } from "@tanstack/react-query";
import {
	configuredProviderModelOptions,
	isRealtimeSttModel,
	isStreamingOnlySttModel,
	STT_MODELS,
} from "../modelOptions";
import type { ModelChoice } from "../modelSelection";
import { configAPI, hasManagedInferenceAccess } from "../tauri";
import { useLicenseAuthContext } from "./license";
import {
	useByokLlmModels,
	useManagedModels,
	useOllamaModels,
	useWhisperModels,
} from "./providers";

export type ModelPurpose = "dictation" | "file" | "meeting" | "llm";

/** Catalog readiness is separate from account readiness: BYOK never waits for Managed. */
export function useModelChoices(purpose: ModelPurpose) {
	const providers = useQuery({
		queryKey: ["availableProviders", "picker"],
		queryFn: () => configAPI.getAvailableProviders(true),
	});
	const auth = useLicenseAuthContext();
	const managedEnabled = hasManagedInferenceAccess(auth.data);
	const managed = useManagedModels(managedEnabled);
	const llm = purpose === "llm";
	const byok = useByokLlmModels(llm);
	const ollamaEnabled =
		llm &&
		providers.data?.llm.some(
			(provider) => provider.value === "ollama" && provider.configured === true,
		) === true;
	const ollama = useOllamaModels(ollamaEnabled);
	const whisperEnabled =
		!llm &&
		providers.data?.stt.some((provider) => provider.value === "whisper") ===
			true;
	const whisper = useWhisperModels(whisperEnabled);
	const choices: ModelChoice[] = [];
	const setupProviders: { provider_id: string; label: string }[] = [];
	for (const provider of (llm ? providers.data?.llm : providers.data?.stt) ??
		[]) {
		const isWhisper =
			provider.value === "whisper" || provider.value === "local-whisper";
		const models =
			provider.value === "ollama"
				? (ollama.data ?? [])
				: isWhisper
					? (whisper.data ?? []).map((model) => ({
							value: model.id,
							label: model.name,
						}))
					: (configuredProviderModelOptions(provider) ??
						(llm ? byok.data[provider.value] : STT_MODELS[provider.value]) ??
						[]);
		if (models.length === 0)
			setupProviders.push({ provider_id: isWhisper ? "local-whisper" : provider.value, label: provider.label });
		for (const model of models) {
			const providerId = isWhisper ? "local-whisper" : provider.value;
			if (
				!llm &&
				purpose !== "dictation" &&
				isStreamingOnlySttModel(providerId, model.value)
			)
				continue;
			const downloaded =
				!isWhisper ||
				whisper.data?.some(
					(local) => local.id === model.value && local.is_downloaded,
				) === true;
			const ready = provider.configured !== false && downloaded;
			const realtime = !llm && isRealtimeSttModel(providerId, model.value);
			const batch = llm || !isStreamingOnlySttModel(providerId, model.value);
			const price = "pricing" in model ? model.pricing : undefined;
			choices.push({
				selection: {
					source: "byok",
					provider_id: providerId,
					model_id: model.value,
				},
				displayName: model.label,
				providerName: provider.label,
				ready,
				capabilities: llm
					? ["Text generation"]
					: [
							...(realtime ? ["Realtime input"] : []),
							...(batch ? ["After recording"] : []),
							...(model.value === "gpt-4o-transcribe-diarize"
								? ["Speaker labels"]
								: []),
						],
				reason: ready
					? undefined
					: isWhisper
						? "Download this model in provider settings"
						: "The provider key or endpoint is unavailable. Open secure provider setup",
				limitations:
					!llm && !batch
						? "Realtime-only: After recording is unavailable"
						: undefined,
				pricing:
					price &&
					typeof price === "object" &&
					"input_per_million" in price &&
					"output_per_million" in price &&
					typeof price.input_per_million === "number" &&
					Number.isFinite(price.input_per_million) &&
					price.input_per_million >= 0 &&
					typeof price.output_per_million === "number" &&
					Number.isFinite(price.output_per_million) &&
					price.output_per_million >= 0
						? `Input $${price.input_per_million}/1M tokens; output $${price.output_per_million}/1M tokens (models.dev estimate; verify provider billing)`
						: undefined,
			});
		}
	}
	if (managedEnabled && managed.isSuccess)
		for (const model of managed.data ?? []) {
			if (
				llm
					? !model.capabilities.includes("chat_completions")
					: !model.capabilities.includes("transcription")
			)
				continue;
			choices.push({
				selection: {
					source: "managed",
					provider_id: model.provider,
					model_id: model.id,
				},
				displayName: model.display_name,
				providerName: model.provider,
				ready: true,
				capabilities: llm
					? ["Text generation"]
					: [
							"After recording",
							...(model.id === "gpt-4o-transcribe-diarize"
								? ["Speaker labels"]
								: []),
						],
				pricing:
					"Uses your Kolboo allowance. Vendor pricing is unavailable, not assumed free.",
			});
		}
	const managedMessage = auth.isPending
		? "Checking account access…"
		: auth.isError
			? "Account access could not be loaded. Your models remain available."
			: !managedEnabled
				? "Sign in or view account access to use Kolboo Managed. Your models do not require an account."
				: managed.isPending
					? "Loading Managed models…"
					: managed.isError
						? "Managed models could not be loaded. Retry without changing your saved selection."
						: (managed.data ?? []).length === 0
							? "No Managed models are published for this account yet."
							: undefined;
	const retry = () => {
		void providers.refetch();
		void auth.refetch();
		if (managedEnabled) void managed.refetch();
		if (ollamaEnabled) void ollama.refetch();
		if (whisperEnabled) void whisper.refetch();
	};
	const failedSources = [
		{ label: "provider list", query: providers, enabled: true },
		{ label: "account access", query: auth, enabled: true },
		{ label: "managed models", query: managed, enabled: managedEnabled },
		{ label: "local models", query: whisper, enabled: whisperEnabled },
		{ label: "local endpoints", query: ollama, enabled: ollamaEnabled },
	].filter(({ enabled, query }) => enabled && query.isError);
	return {
		choices,
		setupProviders,
		managedMessage,
		loading: providers.isPending,
		retry,
		error: providers.isError,
		failedSources,
	};
}
