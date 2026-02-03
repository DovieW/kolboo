import { useCallback } from "react";
import { STT_MODELS } from "../../../lib/modelOptions";
import type {
	AppSettings,
	RewriteProgramPromptProfile,
} from "../../../lib/tauri";

const DEFAULT_STT_TIMEOUT = 10;

type UseSttSettingsHandlersOptions = {
	isDefaultScope: boolean;
	settings: AppSettings | undefined;
	activeProfile: RewriteProgramPromptProfile | null;
	whisperServerModelDraft: string;
	localProfileSttTimeout: string | number;
	localProfileSttLanguage: string;
	// State setters
	setSttProviderInheriting: (v: boolean) => void;
	setSttModelInheriting: (v: boolean) => void;
	setSttLanguageInheriting: (v: boolean) => void;
	setSttTimeoutInheriting: (v: boolean) => void;
	setLocalProfileSttProvider: (v: string | null) => void;
	setLocalProfileSttModel: (v: string | null) => void;
	setLocalProfileSttLanguage: (v: string) => void;
	setLocalProfileSttTimeout: (v: string | number) => void;
	// Mutations
	updateSTTProvider: {
		mutate: (v: string, opts?: { onSuccess?: () => void }) => void;
	};
	updateSTTModel: {
		mutate: (v: string | null, opts?: { onSuccess?: () => void }) => void;
	};
	updateSTTLanguage: {
		mutate: (v: string, opts?: { onSuccess?: () => void }) => void;
	};
	updateSTTTimeout: {
		mutate: (v: number, opts?: { onSuccess?: () => void }) => void;
	};
	// Helpers
	saveProfileMetadata: (updates: Partial<RewriteProgramPromptProfile>) => void;
	openDisableOverrideDialog: (opts: {
		title: string;
		onConfirm: () => void;
	}) => void;
};

export type SttSettingsHandlers = {
	handleWhisperServerModelDraftBlur: () => void;
	handleSttProviderChange: (value: string | null) => void;
	handleSttModelChange: (value: string | null) => void;
	handleSttLanguageChange: (value: string | null) => void;
	handleSttTimeoutChange: (value: number | string) => void;
	handleSttTimeoutBlur: () => void;
	handleDisableSttProviderOverride: () => void;
	handleDisableSttModelOverride: () => void;
	handleDisableSttLanguageOverride: () => void;
	handleDisableSttTimeoutOverride: () => void;
};

/**
 * Hook that provides handlers for STT (speech-to-text) settings changes.
 * Encapsulates logic for both default scope and profile-specific overrides.
 */
export function useSttSettingsHandlers({
	isDefaultScope,
	settings,
	activeProfile,
	whisperServerModelDraft,
	localProfileSttTimeout,
	localProfileSttLanguage,
	setSttProviderInheriting,
	setSttModelInheriting,
	setSttLanguageInheriting,
	setSttTimeoutInheriting,
	setLocalProfileSttProvider,
	setLocalProfileSttModel,
	setLocalProfileSttLanguage,
	setLocalProfileSttTimeout,
	updateSTTProvider,
	updateSTTModel,
	updateSTTLanguage,
	updateSTTTimeout,
	saveProfileMetadata,
	openDisableOverrideDialog,
}: UseSttSettingsHandlersOptions): SttSettingsHandlers {
	// ─────────────────────────────────────────────────────────────────────────
	// Default scope handlers
	// ─────────────────────────────────────────────────────────────────────────

	const handleDefaultSTTProviderChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			updateSTTProvider.mutate(value, {
				onSuccess: () => {
					const models = STT_MODELS[value];
					const firstModel = models?.[0];
					if (firstModel) {
						updateSTTModel.mutate(firstModel.value);
					}
				},
			});
		},
		[updateSTTModel, updateSTTProvider],
	);

	const handleDefaultSTTModelChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			updateSTTModel.mutate(value);
		},
		[updateSTTModel],
	);

	const handleDefaultSTTLanguageChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			updateSTTLanguage.mutate(value);
		},
		[updateSTTLanguage],
	);

	const handleDefaultSTTTimeoutChange = useCallback(
		(value: number) => {
			updateSTTTimeout.mutate(value);
		},
		[updateSTTTimeout],
	);

	// ─────────────────────────────────────────────────────────────────────────
	// Profile-specific handlers
	// ─────────────────────────────────────────────────────────────────────────

	const handleWhisperServerModelDraftBlur = useCallback(() => {
		const trimmed = whisperServerModelDraft.trim();
		const toStore = trimmed.length > 0 ? trimmed : null;

		if (isDefaultScope) {
			const stored = settings?.stt_model?.trim() || null;
			if (toStore === stored) return;
			updateSTTModel.mutate(toStore);
			return;
		}

		setSttModelInheriting(false);
		setLocalProfileSttModel(toStore);
		saveProfileMetadata({ stt_model: toStore });
	}, [
		isDefaultScope,
		saveProfileMetadata,
		setLocalProfileSttModel,
		setSttModelInheriting,
		settings?.stt_model,
		updateSTTModel,
		whisperServerModelDraft,
	]);

	const handleSttProviderChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			if (isDefaultScope) {
				handleDefaultSTTProviderChange(value);
				return;
			}

			setSttProviderInheriting(false);
			setSttModelInheriting(false);
			setLocalProfileSttProvider(value);
			const models = STT_MODELS[value] ?? [];
			const firstModel = models[0]?.value ?? null;
			setLocalProfileSttModel(firstModel);
			saveProfileMetadata({
				stt_provider: value,
				stt_model: firstModel,
			});
		},
		[
			handleDefaultSTTProviderChange,
			isDefaultScope,
			saveProfileMetadata,
			setLocalProfileSttModel,
			setLocalProfileSttProvider,
			setSttModelInheriting,
			setSttProviderInheriting,
		],
	);

	const handleSttModelChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			if (isDefaultScope) {
				handleDefaultSTTModelChange(value);
				return;
			}

			setSttModelInheriting(false);
			setLocalProfileSttModel(value);
			saveProfileMetadata({ stt_model: value });
		},
		[
			handleDefaultSTTModelChange,
			isDefaultScope,
			saveProfileMetadata,
			setLocalProfileSttModel,
			setSttModelInheriting,
		],
	);

	const handleSttLanguageChange = useCallback(
		(value: string | null) => {
			if (!value) return;
			if (isDefaultScope) {
				handleDefaultSTTLanguageChange(value);
				return;
			}

			setSttLanguageInheriting(false);
			setLocalProfileSttLanguage(value);
			saveProfileMetadata({ stt_language: value });
		},
		[
			handleDefaultSTTLanguageChange,
			isDefaultScope,
			saveProfileMetadata,
			setLocalProfileSttLanguage,
			setSttLanguageInheriting,
		],
	);

	const handleSttTimeoutChange = useCallback(
		(value: number | string) => {
			// Keep local state permissive so typing feels natural (e.g., allow clearing the field
			// or temporarily typing an out-of-range intermediate value like "1" before "10").
			setLocalProfileSttTimeout(value);

			// Only persist when the value is a valid in-range number.
			if (typeof value !== "number" || Number.isNaN(value)) return;
			if (value < 5 || value > 120) return;

			if (isDefaultScope) {
				handleDefaultSTTTimeoutChange(value);
				return;
			}

			setSttTimeoutInheriting(false);
			saveProfileMetadata({ stt_timeout_seconds: value });
		},
		[
			handleDefaultSTTTimeoutChange,
			isDefaultScope,
			saveProfileMetadata,
			setLocalProfileSttTimeout,
			setSttTimeoutInheriting,
		],
	);

	const handleSttTimeoutBlur = useCallback(() => {
		// On blur, clamp and normalize.
		// If the user cleared the input, revert to the effective value without saving.
		if (localProfileSttTimeout === "") {
			const fallback = isDefaultScope
				? (settings?.stt_timeout_seconds ?? DEFAULT_STT_TIMEOUT)
				: (activeProfile?.stt_timeout_seconds ??
					settings?.stt_timeout_seconds ??
					DEFAULT_STT_TIMEOUT);
			setLocalProfileSttTimeout(fallback);
			return;
		}

		if (
			typeof localProfileSttTimeout !== "number" ||
			Number.isNaN(localProfileSttTimeout)
		) {
			return;
		}

		const clamped = Math.max(5, Math.min(120, localProfileSttTimeout));
		if (clamped !== localProfileSttTimeout) {
			setLocalProfileSttTimeout(clamped);
		}

		if (isDefaultScope) {
			handleDefaultSTTTimeoutChange(clamped);
			return;
		}

		setSttTimeoutInheriting(false);
		saveProfileMetadata({ stt_timeout_seconds: clamped });
	}, [
		activeProfile?.stt_timeout_seconds,
		handleDefaultSTTTimeoutChange,
		isDefaultScope,
		localProfileSttTimeout,
		saveProfileMetadata,
		setLocalProfileSttTimeout,
		setSttTimeoutInheriting,
		settings?.stt_timeout_seconds,
	]);

	// ─────────────────────────────────────────────────────────────────────────
	// Override disable handlers
	// ─────────────────────────────────────────────────────────────────────────

	const handleDisableSttProviderOverride = useCallback(() => {
		openDisableOverrideDialog({
			title: "Disable Speech-to-Text Provider override?",
			onConfirm: () => {
				setSttProviderInheriting(true);
				setSttModelInheriting(true);
				setLocalProfileSttProvider(settings?.stt_provider ?? null);
				setLocalProfileSttModel(settings?.stt_model ?? null);
				saveProfileMetadata({
					stt_provider: null,
					stt_model: null,
				});
			},
		});
	}, [
		openDisableOverrideDialog,
		saveProfileMetadata,
		setLocalProfileSttModel,
		setLocalProfileSttProvider,
		setSttModelInheriting,
		setSttProviderInheriting,
		settings?.stt_model,
		settings?.stt_provider,
	]);

	const handleDisableSttModelOverride = useCallback(() => {
		openDisableOverrideDialog({
			title: "Disable STT Model override?",
			onConfirm: () => {
				setSttModelInheriting(true);
				setLocalProfileSttModel(settings?.stt_model ?? null);
				saveProfileMetadata({ stt_model: null });
			},
		});
	}, [
		openDisableOverrideDialog,
		saveProfileMetadata,
		setLocalProfileSttModel,
		setSttModelInheriting,
		settings?.stt_model,
	]);

	const handleDisableSttLanguageOverride = useCallback(() => {
		openDisableOverrideDialog({
			title: "Disable STT Language override?",
			onConfirm: () => {
				setSttLanguageInheriting(true);
				setLocalProfileSttLanguage(
					settings?.stt_language ?? localProfileSttLanguage,
				);
				saveProfileMetadata({ stt_language: null });
			},
		});
	}, [
		localProfileSttLanguage,
		openDisableOverrideDialog,
		saveProfileMetadata,
		setLocalProfileSttLanguage,
		setSttLanguageInheriting,
		settings?.stt_language,
	]);

	const handleDisableSttTimeoutOverride = useCallback(() => {
		openDisableOverrideDialog({
			title: "Disable STT Timeout override?",
			onConfirm: () => {
				setSttTimeoutInheriting(true);
				setLocalProfileSttTimeout(
					settings?.stt_timeout_seconds ?? DEFAULT_STT_TIMEOUT,
				);
				saveProfileMetadata({ stt_timeout_seconds: null });
			},
		});
	}, [
		openDisableOverrideDialog,
		saveProfileMetadata,
		setLocalProfileSttTimeout,
		setSttTimeoutInheriting,
		settings?.stt_timeout_seconds,
	]);

	return {
		handleWhisperServerModelDraftBlur,
		handleSttProviderChange,
		handleSttModelChange,
		handleSttLanguageChange,
		handleSttTimeoutChange,
		handleSttTimeoutBlur,
		handleDisableSttProviderOverride,
		handleDisableSttModelOverride,
		handleDisableSttLanguageOverride,
		handleDisableSttTimeoutOverride,
	};
}
