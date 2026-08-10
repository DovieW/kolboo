import { Loader, Select, Slider, Text } from "@mantine/core";
import { useEffect, useState } from "react";
import {
	LLM_MODELS,
	managedChatModelOptions,
	STT_MODELS,
} from "../../lib/modelOptions";
import {
	useAvailableProviders,
	useLicenseAuthContext,
	useManagedModels,
	useSettings,
	useUpdateGeminiThinkingBudget,
	useUpdateGeminiThinkingLevel,
	useUpdateLLMModel,
	useUpdateLLMProvider,
	useUpdateOpenAiReasoningEffort,
	useUpdateSTTModel,
	useUpdateSTTProvider,
	useUpdateSTTTimeout,
} from "../../lib/queries";
import {
	hasManagedInferenceAccess,
	type OpenAiReasoningEffort,
} from "../../lib/tauri";
import { HintSelectWithDefaultHint } from "../HintSelectWithDefaultHint";
import { SettingsRow } from "./SettingsRow";

// NOTE: This timeout is used by the Rust pipeline as a transcription request timeout.
// Keep this default aligned with backend fallbacks so "unset" settings don't lie.
const DEFAULT_STT_TIMEOUT = 10;

/** @lintignore */
export function ProvidersSettings() {
	const { data: settings, isLoading: isLoadingSettings } = useSettings();
	const { data: availableProviders, isLoading: isLoadingProviders } =
		useAvailableProviders();
	const { data: licenseAuthContext } = useLicenseAuthContext();
	const managedAccessEnabled = hasManagedInferenceAccess(licenseAuthContext);
	const managedModelsQuery = useManagedModels(managedAccessEnabled);
	const managedModels = managedModelsQuery.data ?? [];
	const managedProviderReady = managedAccessEnabled && managedModels.length > 0;
	const getLlmModelsForProvider = (provider: string) => {
		if (provider === "managed") {
			return managedChatModelOptions(managedModels);
		}
		return LLM_MODELS[provider] ?? [];
	};

	// Wait for settings (source of truth) and provider list (for options)
	const isLoadingProviderData =
		isLoadingSettings ||
		isLoadingProviders ||
		(managedAccessEnabled && managedModelsQuery.isLoading);
	const updateSTTProvider = useUpdateSTTProvider();
	const updateSTTModel = useUpdateSTTModel();
	const updateLLMProvider = useUpdateLLMProvider();
	const updateLLMModel = useUpdateLLMModel();
	const updateOpenAiReasoningEffort = useUpdateOpenAiReasoningEffort();
	const updateGeminiThinkingBudget = useUpdateGeminiThinkingBudget();
	const updateGeminiThinkingLevel = useUpdateGeminiThinkingLevel();
	const updateSTTTimeout = useUpdateSTTTimeout();

	const handleSTTProviderChange = (value: string | null) => {
		if (!value) return;
		// Save to local settings (Tauri) then notify overlay window to sync to server
		updateSTTProvider.mutate(value, {
			onSuccess: () => {
				// Reset model to first available when provider changes
				const models = STT_MODELS[value];
				const firstModel = models?.[0];
				if (firstModel) {
					updateSTTModel.mutate(firstModel.value);
				}
			},
		});
	};

	const handleSTTModelChange = (value: string | null) => {
		if (!value) return;
		updateSTTModel.mutate(value, {
			onSuccess: () => {},
		});
	};

	const handleLLMProviderChange = (value: string | null) => {
		if (!value) return;
		// Save to local settings (Tauri) then notify overlay window to sync to server
		updateLLMProvider.mutate(value, {
			onSuccess: () => {
				// Reset model to first available when provider changes
				const models = getLlmModelsForProvider(value);
				const firstModel = models?.[0];
				if (firstModel) {
					updateLLMModel.mutate(firstModel.value);
				}
			},
		});
	};

	const handleLLMModelChange = (value: string | null) => {
		if (!value) return;
		updateLLMModel.mutate(value, {
			onSuccess: () => {},
		});
	};

	const handleSTTTimeoutChange = (value: number) => {
		// Save to local settings (Tauri) then notify overlay window to sync to server
		updateSTTTimeout.mutate(value, {
			onSuccess: () => {},
		});
	};

	// Get the current timeout value from settings, falling back to default
	const currentTimeout = settings?.stt_timeout_seconds ?? DEFAULT_STT_TIMEOUT;

	// Local state for smooth slider dragging
	const [sliderValue, setSliderValue] = useState(currentTimeout);
	// Sync local state when server value changes
	useEffect(() => {
		setSliderValue(currentTimeout);
	}, [currentTimeout]);

	// Group providers by cloud/local for dropdown display
	const sttCloudProviderOrder = [
		"groq",
		"openai",
		"fireworks",
		"aquavoice",
		"assemblyai",
		"speechmatics",
		"elevenlabs",
		"deepgram",
	];
	const sttCloudProviderOrderIndex = new Map(
		sttCloudProviderOrder.map((value, index) => [value, index]),
	);
	const sttCloudProviders =
		availableProviders?.stt
			.filter((p) => !p.is_local)
			.map((p) => ({ value: p.value, label: p.label }))
			.sort((a, b) => {
				const aIndex = sttCloudProviderOrderIndex.get(a.value) ?? 999;
				const bIndex = sttCloudProviderOrderIndex.get(b.value) ?? 999;
				if (aIndex !== bIndex) return aIndex - bIndex;
				return a.label.localeCompare(b.label);
			}) ?? [];
	const sttLocalProviders =
		availableProviders?.stt
			.filter((p) => p.is_local)
			.map((p) => ({ value: p.value, label: p.label })) ?? [];
	const sttProviderOptions = [
		{ group: "Cloud", items: sttCloudProviders },
		{ group: "Local", items: sttLocalProviders },
	];

	const configuredLlmCloudProviders =
		availableProviders?.llm
			.filter((p) => !p.is_local)
			.map((p) => ({ value: p.value, label: p.label })) ?? [];
	const llmCloudProviders = [
		...(managedProviderReady
			? [{ value: "managed", label: "Kolboo Managed" }]
			: []),
		...configuredLlmCloudProviders.filter(
			(provider) => provider.value !== "managed",
		),
	];
	const llmLocalProviders =
		availableProviders?.llm
			.filter((p) => p.is_local)
			.map((p) => ({ value: p.value, label: p.label })) ?? [];
	const llmProviderOptions = [
		{ group: "Cloud", items: llmCloudProviders },
		{ group: "Local", items: llmLocalProviders },
	];

	// Get available models for the selected providers
	const sttModelOptions = settings?.stt_provider
		? (STT_MODELS[settings.stt_provider] ?? [])
		: [];
	const llmModelOptions = settings?.llm_provider
		? getLlmModelsForProvider(settings.llm_provider)
		: [];

	const effectiveLlmProvider = settings?.llm_provider ?? null;
	const effectiveLlmModel =
		settings?.llm_model ?? llmModelOptions[0]?.value ?? null;

	const supportsOpenAiReasoningEffort =
		effectiveLlmProvider === "openai" &&
		!!effectiveLlmModel &&
		(effectiveLlmModel.startsWith("gpt-5") ||
			effectiveLlmModel.startsWith("o"));

	const supportsGeminiThinkingLevel =
		effectiveLlmProvider === "gemini" &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-3");

	const supportsGeminiThinkingBudget =
		effectiveLlmProvider === "gemini" &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-2.5") &&
		!effectiveLlmModel.includes("flash-lite");

	// Mantine Select requires option values to be strings.
	const SELECT_DEFAULT = "default";

	const openAiThinkingEffortsForModel = (
		model: string,
	): OpenAiReasoningEffort[] => {
		// OpenAI docs (2025-12):
		// - gpt-5.1 supports: none, low, medium, high
		// - models before gpt-5.1 do not support `none`
		// - gpt-5-pro defaults to and only supports `high`
		if (model.startsWith("gpt-5-pro")) {
			return ["high"];
		}
		if (model.startsWith("gpt-5.2") || model.startsWith("gpt-5.1")) {
			return ["none", "low", "medium", "high"];
		}
		if (model.startsWith("gpt-5")) {
			return ["low", "medium", "high"];
		}
		if (model.startsWith("o")) {
			return ["low", "medium", "high"];
		}
		return [];
	};

	const openAiDefaultReasoningEffortForModel = (
		model: string,
	): OpenAiReasoningEffort => {
		// OpenAI docs (2025-12):
		// - gpt-5.1 defaults to `none`
		// - models before gpt-5.1 default to `medium`
		// - gpt-5-pro defaults to `high`
		if (model.startsWith("gpt-5-pro")) return "high";
		if (model.startsWith("gpt-5.2") || model.startsWith("gpt-5.1"))
			return "none";
		return "medium";
	};

	const openAiThinkingOptions =
		!supportsOpenAiReasoningEffort || !effectiveLlmModel
			? []
			: [
					{ value: SELECT_DEFAULT, label: "Default" },
					...openAiThinkingEffortsForModel(effectiveLlmModel).map((v) => ({
						value: v,
						label:
							v === "none" ? "None" : v.charAt(0).toUpperCase() + v.slice(1),
					})),
				];

	const isGemini3Flash =
		supportsGeminiThinkingLevel &&
		effectiveLlmModel?.includes("gemini-3-flash");
	const isGemini3Pro =
		supportsGeminiThinkingLevel && effectiveLlmModel?.includes("gemini-3-pro");

	const geminiThinkingLevelOptions = isGemini3Flash
		? [
				{ value: SELECT_DEFAULT, label: "Default" },
				{ value: "minimal", label: "Minimal" },
				{ value: "low", label: "Low" },
				{ value: "medium", label: "Medium" },
				{ value: "high", label: "High" },
			]
		: [
				{ value: SELECT_DEFAULT, label: "Default" },
				{ value: "low", label: "Low" },
				{ value: "high", label: "High" },
			];

	const canDisableGemini25Thinking =
		supportsGeminiThinkingBudget &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-2.5-flash") &&
		!effectiveLlmModel.includes("gemini-2.5-pro");

	const isGemini25Pro =
		supportsGeminiThinkingBudget &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-2.5-pro");

	const gemini25MaxBudget = isGemini25Pro ? 32768 : 24576;
	const gemini25MinBudget = isGemini25Pro ? 128 : 0;

	const geminiThinkingBudgetOptions: Array<{ value: string; label: string }> = [
		{ value: SELECT_DEFAULT, label: "Default" },
		{ value: "-1", label: "Dynamic (-1)" },
		...(canDisableGemini25Thinking ? [{ value: "0", label: "Off (0)" }] : []),
		...(isGemini25Pro
			? [{ value: String(gemini25MinBudget), label: "Minimal (128)" }]
			: []),
		{ value: "1024", label: "Light (1024)" },
		{ value: "4096", label: "Medium (4096)" },
		{ value: "16384", label: "High (16384)" },
		...(gemini25MaxBudget > 16384
			? [
					{
						value: String(gemini25MaxBudget),
						label: `Max (${gemini25MaxBudget})`,
					},
				]
			: []),
	];

	const handleOpenAiReasoningEffortChange = (value: string | null) => {
		if (value == null || value === SELECT_DEFAULT) {
			updateOpenAiReasoningEffort.mutate(null);
			return;
		}

		// Select values are strings; narrow to the allowed union before mutating.
		const v: OpenAiReasoningEffort | null =
			value === "none" ||
			value === "minimal" ||
			value === "low" ||
			value === "medium" ||
			value === "high" ||
			value === "xhigh"
				? (value as OpenAiReasoningEffort)
				: null;
		if (!v) return;

		updateOpenAiReasoningEffort.mutate(v);
	};

	const handleGeminiThinkingLevelChange = (value: string | null) => {
		const v =
			value === "minimal" ||
			value === "low" ||
			value === "medium" ||
			value === "high"
				? value
				: null;
		updateGeminiThinkingLevel.mutate(v);
	};

	const handleGeminiThinkingBudgetChange = (value: string | null) => {
		if (value == null || value === SELECT_DEFAULT) {
			updateGeminiThinkingBudget.mutate(null);
			return;
		}

		const parsed = Number(value);
		if (!Number.isFinite(parsed)) return;
		updateGeminiThinkingBudget.mutate(parsed);
	};

	return (
		<>
			{/* STT Provider */}
			<SettingsRow
				label="Speech-to-Text Provider"
				description="Service for transcribing audio"
				right={
					isLoadingProviderData ? (
						<Loader size="sm" color="orange" />
					) : (
						<Select
							data={sttProviderOptions}
							value={settings?.stt_provider ?? null}
							onChange={handleSTTProviderChange}
							placeholder="Select provider"
							withCheckIcon={false}
							disabled={
								sttCloudProviders.length === 0 && sttLocalProviders.length === 0
							}
							styles={{
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
									minWidth: 200,
								},
							}}
						/>
					)
				}
			/>

			{/* STT Model - only show if provider has models */}
			{sttModelOptions.length > 0 && (
				<SettingsRow
					label="STT Model"
					description="Model to use for transcription"
					right={
						<Select
							data={sttModelOptions}
							value={settings?.stt_model ?? sttModelOptions[0]?.value ?? null}
							onChange={handleSTTModelChange}
							placeholder="Select model"
							withCheckIcon={false}
							styles={{
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
									minWidth: 200,
								},
							}}
						/>
					}
				/>
			)}

			{/* LLM Provider */}
			<SettingsRow
				label="Language Model Provider"
				description="AI service for text formatting"
				right={
					isLoadingProviderData ? (
						<Loader size="sm" color="orange" />
					) : (
						<Select
							data={llmProviderOptions}
							value={settings?.llm_provider ?? null}
							onChange={handleLLMProviderChange}
							placeholder="Select provider"
							withCheckIcon={false}
							disabled={
								llmCloudProviders.length === 0 && llmLocalProviders.length === 0
							}
							styles={{
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
									minWidth: 200,
								},
							}}
						/>
					)
				}
			/>

			{/* LLM Model - only show if provider has models */}
			{llmModelOptions.length > 0 && (
				<SettingsRow
					label="Rewrite LLM Model"
					description="LLM Model used to rewrite the transcription."
					right={
						<Select
							data={llmModelOptions}
							value={settings?.llm_model ?? llmModelOptions[0]?.value ?? null}
							onChange={handleLLMModelChange}
							placeholder="Select model"
							withCheckIcon={false}
							styles={{
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
									minWidth: 200,
								},
							}}
						/>
					}
				/>
			)}

			{/* OpenAI thinking (gpt-5 / o-series) */}
			{supportsOpenAiReasoningEffort && (
				<SettingsRow
					label="Thinking"
					description="Set the reasoning effort for this model."
					right={
						<HintSelectWithDefaultHint
							data={openAiThinkingOptions}
							value={settings?.openai_reasoning_effort ?? SELECT_DEFAULT}
							onChange={handleOpenAiReasoningEffortChange}
							placeholder="Default"
							defaultValue={SELECT_DEFAULT}
							defaultHint={
								effectiveLlmModel
									? openAiDefaultReasoningEffortForModel(effectiveLlmModel)
									: "medium"
							}
							inputStyle={{
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							}}
						/>
					}
				/>
			)}

			{/* Gemini thinking level (Gemini 3) */}
			{supportsGeminiThinkingLevel && (
				<SettingsRow
					label="Thinking Level"
					description={
						isGemini3Pro
							? "Gemini 3 Pro supports low/high (default high)."
							: "Gemini 3 Flash supports minimal/low/medium/high (default high)."
					}
					right={
						<HintSelectWithDefaultHint
							data={geminiThinkingLevelOptions}
							value={settings?.gemini_thinking_level ?? SELECT_DEFAULT}
							onChange={handleGeminiThinkingLevelChange}
							placeholder="Default"
							defaultValue={SELECT_DEFAULT}
							defaultHint="high"
							inputStyle={{
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							}}
						/>
					}
				/>
			)}

			{/* Gemini thinking budget (Gemini 2.5) */}
			{supportsGeminiThinkingBudget && (
				<SettingsRow
					label="Thinking Budget"
					description="Token budget for Gemini 2.5 thinking."
					right={
						<HintSelectWithDefaultHint
							data={geminiThinkingBudgetOptions}
							value={
								settings?.gemini_thinking_budget == null
									? SELECT_DEFAULT
									: String(settings.gemini_thinking_budget)
							}
							onChange={handleGeminiThinkingBudgetChange}
							placeholder="Default"
							defaultValue={SELECT_DEFAULT}
							defaultHint="dynamic"
							inputStyle={{
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							}}
						/>
					}
				/>
			)}

			{/* STT Timeout */}
			<SettingsRow
				label="STT Timeout"
				description="Increase if nothing is getting transcribed"
				right={
					<div style={{ minWidth: 320 }}>
						<div
							style={{
								marginTop: 12,
								display: "flex",
								alignItems: "center",
								gap: 12,
							}}
						>
							<Slider
								value={sliderValue}
								onChange={setSliderValue}
								onChangeEnd={handleSTTTimeoutChange}
								min={5}
								max={120}
								step={5}
								marks={[
									{ value: 5, label: "5s" },
									{ value: 30, label: "30s" },
									{ value: 60, label: "60s" },
									{ value: 120, label: "120s" },
								]}
								styles={{
									root: { flex: 1 },
									track: { backgroundColor: "var(--bg-elevated)" },
									bar: { backgroundColor: "var(--accent-primary)" },
									thumb: { borderColor: "var(--accent-primary)" },
									markLabel: { color: "var(--text-secondary)", fontSize: 10 },
								}}
							/>
							<Text size="xs" c="dimmed" style={{ minWidth: 32 }}>
								{Math.round(sliderValue)}s
							</Text>
						</div>
					</div>
				}
			/>
		</>
	);
}
