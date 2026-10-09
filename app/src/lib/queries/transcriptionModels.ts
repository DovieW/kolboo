import {
	configuredProviderModelOptions,
	isRealtimeSttModel,
	STT_MODELS,
} from "../modelOptions";
import { modelSelectionKey } from "../modelSelection";
import { hasManagedInferenceAccess } from "../tauri/managedInference";
import { useLicenseAuthContext } from "./license";
import {
	useAvailableProviders,
	useManagedModels,
	useWhisperModels,
} from "./providers";

/** Batch-capable models shared by meeting recordings and file imports. */
export function useTranscriptionModels() {
	const providers = useAvailableProviders();
	const localEnabled =
		providers.data?.stt.some(
			(provider) =>
				provider.value === "whisper" || provider.value === "local-whisper",
		) ?? false;
	const localModels = useWhisperModels(localEnabled);
	const auth = useLicenseAuthContext();
	const managed = hasManagedInferenceAccess(auth.data);
	const catalog = useManagedModels(managed);
	const loading =
		providers.isPending ||
		auth.isPending ||
		(localEnabled && localModels.isPending) ||
		(managed && catalog.isPending);
	const failedSources = [
		{ label: "provider list", query: providers, enabled: true },
		{ label: "account access", query: auth, enabled: true },
		{ label: "local models", query: localModels, enabled: localEnabled },
		{ label: "managed models", query: catalog, enabled: managed },
	].filter(({ query, enabled }) => enabled && query.isError);
	const options = new Map<
		string,
		{
			value: string;
			label: string;
			provider: string;
			model: string;
			use_managed: boolean;
			displayName?: string;
			providerName?: string;
		}
	>();
	if (managed)
		for (const model of catalog.isSuccess ? (catalog.data ?? []) : []) {
			if (!model.capabilities.includes("transcription")) continue;
			const value = modelSelectionKey({
				source: "managed",
				provider_id: model.provider,
				model_id: model.id,
			});
			options.set(value, {
				value,
				label: `${model.display_name} · ${model.provider} · Managed`,
				provider: model.provider,
				model: model.id,
				use_managed: true,
				displayName: model.display_name,
				providerName: model.provider,
			});
		}
	for (const provider of providers.data?.stt ?? []) {
		if (provider.value === "whisper" || provider.value === "local-whisper") {
			for (const model of localModels.data ?? []) {
				if (!model.is_downloaded) continue;
				const value = modelSelectionKey({
					source: "byok",
					provider_id: "local-whisper",
					model_id: model.id,
				});
				options.set(value, {
					value,
					label: `${model.name} · Local Whisper`,
					provider: "local-whisper",
					model: model.id,
					use_managed: false,
					displayName: model.name,
					providerName: "Local Whisper",
				});
			}
		}
		for (const model of configuredProviderModelOptions(provider) ??
			STT_MODELS[provider.value] ??
			[]) {
			if (isRealtimeSttModel(provider.value, model.value)) continue;
			const value = modelSelectionKey({
				source: "byok",
				provider_id: provider.value,
				model_id: model.value,
			});
			if (!options.has(value))
				options.set(value, {
					value,
					label: `${model.label} · ${provider.label}${provider.is_local ? "" : " · Your key"}`,
					provider: provider.value,
					model: model.value,
					use_managed: false,
					displayName: model.label,
					providerName: provider.label,
				});
		}
	}

	return { options, loading, failedSources };
}
