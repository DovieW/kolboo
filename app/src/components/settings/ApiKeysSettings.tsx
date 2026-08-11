import {
	ActionIcon,
	Alert,
	Badge,
	Button,
	Card,
	Collapse,
	Divider,
	Group,
	PasswordInput,
	Progress,
	SegmentedControl,
	Select,
	SimpleGrid,
	Stack,
	Switch,
	Text,
	TextInput,
	Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { Link as LinkIcon } from "lucide-react";
import type { ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import {
	API_KEYS,
	type ApiKeyConfig,
	type ApiKeyMutationIntent,
	resolveApiKeyMutationIntent,
} from "../../lib/apiKeys";
import { formatErrorMessage } from "../../lib/formatError";
import { EMBEDDING_MODELS, STT_MODELS } from "../../lib/modelOptions";
import type { ByokLlmModelCatalog } from "../../lib/modelsDev";
import {
	useByokLlmModels,
	useCancelWhisperModelDownload,
	useDeleteWhisperModel,
	useDownloadWhisperModel,
	useIsLocalWhisperAvailable,
	useIsLocalWhisperModelLoaded,
	useLoadLocalWhisperModel,
	useLocalWhisperBackendStatus,
	useSettings,
	useUnloadLocalWhisperModel,
	useUpdateAssemblyAiFreeTier,
	useUpdateCerebrasFreeTier,
	useUpdateCohereFreeTier,
	useUpdateGroqFreeTier,
	useUpdateLocalWhisperLoadMode,
	useUpdateLocalWhisperModelId,
	useUpdateOllamaUrl,
	useUpdateSpeechmaticsFreeTier,
	useUpdateWhisperServerBaseUrl,
	useValidateWhisperModel,
	useWhisperModels,
	useWhisperModelsDir,
} from "../../lib/queries";
import type {
	LocalWhisperLoadMode,
	LocalWhisperModelLoadEvent,
	WhisperModelDownloadProgress,
	WhisperModelInfo,
} from "../../lib/tauri";
import { tauriAPI } from "../../lib/tauri";
import { OcrProviderSettings } from "./OcrProviderSettings";
import { SettingsRow } from "./SettingsRow";

const GLOBAL_ONLY_TOOLTIP =
	"This setting can only be changed in the Default profile";

function formatProviderModelCounts(
	providerId: string,
	llmModels: ByokLlmModelCatalog,
): string | null {
	const sttCount = STT_MODELS[providerId]?.length ?? 0;
	const llmCount = llmModels[providerId]?.length ?? 0;
	const embedCount = EMBEDDING_MODELS[providerId]?.length ?? 0;

	const parts: string[] = [];
	if (sttCount > 0) parts.push(`${sttCount} STT`);
	if (embedCount > 0) parts.push(`${embedCount} Embed`);
	if (llmCount > 0) parts.push(`${llmCount} LLM`);
	if (parts.length === 0) return null;
	return parts.join(" / ");
}

function formatProviderModelsTooltip(
	providerId: string,
	llmModels: ByokLlmModelCatalog,
): ReactNode | null {
	const embed = EMBEDDING_MODELS[providerId] ?? [];
	const stt = STT_MODELS[providerId] ?? [];
	const llm = llmModels[providerId] ?? [];

	if (embed.length === 0 && stt.length === 0 && llm.length === 0) return null;

	const formatList = (items: Array<{ label: string; value: string }>) =>
		items.map((m) => m.label || m.value).join(", ");

	return (
		<div style={{ maxWidth: 420 }}>
			{embed.length > 0 ? (
				<Text size="xs" fw={600}>
					Embed
				</Text>
			) : null}
			{embed.length > 0 ? (
				<Text size="xs" c="dimmed" style={{ lineHeight: 1.35 }}>
					{formatList(embed)}
				</Text>
			) : null}

			{stt.length > 0 ? (
				<Text size="xs" fw={600} mt={embed.length > 0 ? 8 : 0}>
					STT
				</Text>
			) : null}
			{stt.length > 0 ? (
				<Text size="xs" c="dimmed" style={{ lineHeight: 1.35 }}>
					{formatList(stt)}
				</Text>
			) : null}

			{llm.length > 0 ? (
				<Text
					size="xs"
					fw={600}
					mt={embed.length > 0 || stt.length > 0 ? 8 : 0}
				>
					LLM
				</Text>
			) : null}
			{llm.length > 0 ? (
				<Text size="xs" c="dimmed" style={{ lineHeight: 1.35 }}>
					{formatList(llm)}
				</Text>
			) : null}
		</div>
	);
}

function ApiKeyInput({
	config,
	llmModels,
}: {
	config: ApiKeyConfig;
	llmModels: ByokLlmModelCatalog;
}) {
	const queryClient = useQueryClient();
	const [value, setValue] = useState("");
	const [isPrefilling, _setIsPrefilling] = useState(false);
	const hasHydratedRef = useRef(false);

	const { data: settings } = useSettings();
	const updateGroqFreeTier = useUpdateGroqFreeTier();
	const updateCerebrasFreeTier = useUpdateCerebrasFreeTier();
	const updateAssemblyAiFreeTier = useUpdateAssemblyAiFreeTier();
	const updateSpeechmaticsFreeTier = useUpdateSpeechmaticsFreeTier();
	const updateCohereFreeTier = useUpdateCohereFreeTier();

	const { data: savedKeyValue } = useQuery({
		queryKey: ["apiKeyValue", config.storeKey],
		queryFn: () => tauriAPI.getApiKey(config.storeKey),
		staleTime: 0,
	});

	useEffect(() => {
		if (hasHydratedRef.current) return;
		if (!savedKeyValue) return;

		// Mirror the setup guide: if a key exists, show it in the PasswordInput
		// (hidden by default), so Show/Hide reveals something useful.
		setValue(savedKeyValue);
		hasHydratedRef.current = true;
	}, [savedKeyValue]);

	const saveKey = useMutation({
		mutationFn: async (intent: ApiKeyMutationIntent) => {
			if (intent.kind === "clear") {
				await tauriAPI.clearApiKey(config.storeKey);
				return "";
			}

			await tauriAPI.setApiKey(config.storeKey, intent.value);
			return intent.value;
		},
		onSuccess: async (normalizedValue) => {
			await Promise.all([
				queryClient.invalidateQueries({
					queryKey: ["apiKey", config.storeKey],
				}),
				queryClient.invalidateQueries({
					queryKey: ["apiKeyValue", config.storeKey],
				}),
				queryClient.invalidateQueries({ queryKey: ["availableProviders"] }),
			]);

			// Keep the normalized value in the field so blur/Enter stays idempotent
			// and a cleared key looks cleared immediately, even before the query
			// refetch resolves.
			setValue(normalizedValue);
			hasHydratedRef.current = true;
		},
		onError: (error, intent) => {
			notifications.show({
				title:
					intent.kind === "clear"
						? `Unable to clear ${config.label} API key`
						: `Unable to save ${config.label} API key`,
				message: formatErrorMessage(error),
				color: "red",
			});
		},
	});

	const handleCommit = () => {
		if (saveKey.isPending) return;

		const intent = resolveApiKeyMutationIntent({
			draftValue: value,
			savedValue: savedKeyValue,
		});

		if (!intent) return;

		saveKey.mutate(intent);
	};

	const modelCountsLabel = formatProviderModelCounts(config.id, llmModels);
	const modelsTooltip = formatProviderModelsTooltip(config.id, llmModels);

	return (
		<SettingsRow
			className="api-keys-row"
			left={
				<div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
					<p className="settings-label">{config.label}</p>
					<Text
						size="xs"
						c="var(--text-muted)"
						className="settings-description--single-line"
						title="Stored securely in your OS credential vault. Leave the field blank to remove the saved key."
					>
						Stored securely. Leave blank to clear.
					</Text>
					{config.id === "groq" && (
						<Group gap={10} align="center" wrap="nowrap" mt={2}>
							<Switch
								size="sm"
								checked={settings?.groq_free_tier ?? true}
								onChange={(e) =>
									updateGroqFreeTier.mutate(e.currentTarget.checked)
								}
								aria-label="Groq free tier"
							/>
							<Text size="xs" c="var(--text-secondary)" fw={600}>
								Free tier
							</Text>
							<Text
								size="xs"
								c="var(--text-muted)"
								className="settings-description--single-line"
								style={{ flex: 1 }}
								title="Assume Groq calls cost $0 for stats"
							>
								Assume Groq calls cost $0 for stats
							</Text>
						</Group>
					)}
					{config.id === "cerebras" && (
						<Group gap={10} align="center" wrap="nowrap" mt={2}>
							<Switch
								size="sm"
								checked={settings?.cerebras_free_tier ?? true}
								onChange={(e) =>
									updateCerebrasFreeTier.mutate(e.currentTarget.checked)
								}
								aria-label="Cerebras free tier"
							/>
							<Text size="xs" c="var(--text-secondary)" fw={600}>
								Free tier
							</Text>
							<Text
								size="xs"
								c="var(--text-muted)"
								className="settings-description--single-line"
								style={{ flex: 1 }}
								title="Assume Cerebras calls cost $0 for stats"
							>
								Assume Cerebras calls cost $0 for stats
							</Text>
						</Group>
					)}
					{config.id === "assemblyai" && (
						<Group gap={10} align="center" wrap="nowrap" mt={2}>
							<Switch
								size="sm"
								checked={settings?.assemblyai_free_tier ?? true}
								onChange={(e) =>
									updateAssemblyAiFreeTier.mutate(e.currentTarget.checked)
								}
								aria-label="AssemblyAI free tier"
							/>
							<Text size="xs" c="var(--text-secondary)" fw={600}>
								Free tier
							</Text>
							<Text
								size="xs"
								c="var(--text-muted)"
								className="settings-description--single-line"
								style={{ flex: 1 }}
								title="Assume AssemblyAI calls cost $0 for stats"
							>
								Assume AssemblyAI calls cost $0 for stats
							</Text>
						</Group>
					)}
					{config.id === "speechmatics" && (
						<Group gap={10} align="center" wrap="nowrap" mt={2}>
							<Switch
								size="sm"
								checked={settings?.speechmatics_free_tier ?? true}
								onChange={(e) =>
									updateSpeechmaticsFreeTier.mutate(e.currentTarget.checked)
								}
								aria-label="Speechmatics free tier"
							/>
							<Text size="xs" c="var(--text-secondary)" fw={600}>
								Free tier
							</Text>
							<Text
								size="xs"
								c="var(--text-muted)"
								className="settings-description--single-line"
								style={{ flex: 1 }}
								title="Assume Speechmatics calls cost $0 for stats"
							>
								Assume Speechmatics calls cost $0 for stats
							</Text>
						</Group>
					)}
					{config.id === "cohere" && (
						<Group gap={10} align="center" wrap="nowrap" mt={2}>
							<Switch
								size="sm"
								checked={settings?.cohere_free_tier ?? true}
								onChange={(e) =>
									updateCohereFreeTier.mutate(e.currentTarget.checked)
								}
								aria-label="Cohere free tier"
							/>
							<Text size="xs" c="var(--text-secondary)" fw={600}>
								Free tier
							</Text>
							<Text
								size="xs"
								c="var(--text-muted)"
								className="settings-description--single-line"
								style={{ flex: 1 }}
								title="Assume Cohere calls cost $0 for stats"
							>
								Assume Cohere calls cost $0 for stats
							</Text>
						</Group>
					)}
				</div>
			}
			right={
				<>
					{modelCountsLabel && (
						<Tooltip
							label={modelsTooltip ?? ""}
							withArrow
							multiline
							disabled={!modelsTooltip}
							position="bottom"
							styles={{
								tooltip: {
									backgroundColor: "var(--bg-elevated)",
									color: "var(--text-primary)",
									border: "1px solid var(--border-default)",
								},
							}}
						>
							<Text
								size="xs"
								c="var(--text-muted)"
								style={{ alignSelf: "center", whiteSpace: "nowrap" }}
							>
								{modelCountsLabel}
							</Text>
						</Tooltip>
					)}
					<Tooltip label="Get key" withArrow>
						<ActionIcon
							component="a"
							href={config.getKeyUrl}
							target="_blank"
							rel="noreferrer"
							variant="subtle"
							color="gray"
							size={36}
						>
							<LinkIcon size={16} />
						</ActionIcon>
					</Tooltip>
					<PasswordInput
						value={value}
						onChange={(e) => setValue(e.currentTarget.value)}
						onBlur={handleCommit}
						placeholder={config.placeholder}
						size="sm"
						disabled={isPrefilling || saveKey.isPending}
						styles={{
							input: {
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								height: 36,
								width: 200,
							},
						}}
						onKeyDown={(e) => {
							if (e.key === "Enter") {
								e.preventDefault();
								e.currentTarget.blur();
							}
						}}
					/>
				</>
			}
		/>
	);
}

const WHISPER_MODEL_DOWNLOAD_PROGRESS_EVENT = "whisper-model-download-progress";
const LOCAL_WHISPER_MODEL_LOAD_EVENT = "local-whisper-model-load";

function LocalWhisperModelsCard() {
	const queryClient = useQueryClient();
	const { data: settings } = useSettings();

	const { data: isAvailable } = useIsLocalWhisperAvailable();
	const localWhisperBackendStatus = useLocalWhisperBackendStatus(!!isAvailable);
	const { data: modelsDir } = useWhisperModelsDir();
	const whisperModels = useWhisperModels(!!isAvailable);

	const downloadModel = useDownloadWhisperModel();
	const cancelDownload = useCancelWhisperModelDownload();
	const deleteModel = useDeleteWhisperModel();
	const validateModel = useValidateWhisperModel();
	const updateLocalWhisperModelId = useUpdateLocalWhisperModelId();
	const updateLocalWhisperLoadMode = useUpdateLocalWhisperLoadMode();

	const localWhisperLoaded = useIsLocalWhisperModelLoaded(!!isAvailable);
	const loadLocalWhisperModel = useLoadLocalWhisperModel();
	const unloadLocalWhisperModel = useUnloadLocalWhisperModel();

	const [progressById, setProgressById] = useState<
		Record<string, WhisperModelDownloadProgress>
	>({});
	const [validateResultById, setValidateResultById] = useState<
		Record<string, boolean | null>
	>({});
	const [errorById, setErrorById] = useState<Record<string, string | null>>({});
	const [validatingModelId, setValidatingModelId] = useState<string | null>(
		null,
	);
	const [deletingModelId, setDeletingModelId] = useState<string | null>(null);

	useEffect(() => {
		let disposed = false;
		let unlisten: (() => void) | null = null;

		const setup = async () => {
			try {
				const nextUnlisten = await listen<WhisperModelDownloadProgress>(
					WHISPER_MODEL_DOWNLOAD_PROGRESS_EVENT,
					(event) => {
						const payload = event.payload;

						// Normalize nulls (Rust Option -> null)
						const normalized: WhisperModelDownloadProgress = {
							...payload,
							total_bytes: payload.total_bytes ?? null,
							percent: payload.percent ?? null,
							message: payload.message ?? null,
						};

						setProgressById((prev) => ({
							...prev,
							[normalized.model_id]: normalized,
						}));

						if (normalized.status === "error") {
							setErrorById((prev) => ({
								...prev,
								[normalized.model_id]: normalized.message ?? "Download failed",
							}));
						}

						if (
							normalized.status === "completed" ||
							normalized.status === "cancelled" ||
							normalized.status === "error"
						) {
							queryClient.invalidateQueries({ queryKey: ["whisperModels"] });
						}
					},
				);

				// React StrictMode (dev) mounts effects twice; if we were disposed before
				// `listen()` resolves, immediately unregister to avoid leaked duplicate listeners.
				if (disposed) {
					nextUnlisten();
					return;
				}

				unlisten = nextUnlisten;
			} catch (e) {
				console.warn(
					"Failed to listen for whisper model download progress events:",
					e,
				);
			}
		};

		void setup();

		return () => {
			disposed = true;
			unlisten?.();
		};
	}, [queryClient]);

	const [isLocalWhisperLoading, setIsLocalWhisperLoading] = useState(false);
	const [showDiagnostics, setShowDiagnostics] = useState(false);

	useEffect(() => {
		let disposed = false;
		let unlisten: (() => void) | null = null;

		const setup = async () => {
			try {
				const nextUnlisten = await listen<LocalWhisperModelLoadEvent>(
					LOCAL_WHISPER_MODEL_LOAD_EVENT,
					(event) => {
						const payload = event.payload;
						const status = payload.status;

						if (status === "started") {
							setIsLocalWhisperLoading(true);
							notifications.show({
								title: "Local Whisper",
								message: "Loading model…",
								color: "orange",
							});
							return;
						}

						if (status === "completed") {
							setIsLocalWhisperLoading(false);
							notifications.show({
								title: "Local Whisper",
								message: "Model loaded.",
								color: "green",
							});
							queryClient.invalidateQueries({
								queryKey: ["localWhisperModelLoaded"],
							});
							queryClient.invalidateQueries({
								queryKey: ["localWhisperBackendStatus"],
							});
							return;
						}

						// error
						setIsLocalWhisperLoading(false);
						notifications.show({
							title: "Local Whisper",
							message: payload.message ?? "Model load failed",
							color: "red",
						});
						queryClient.invalidateQueries({
							queryKey: ["localWhisperModelLoaded"],
						});
						queryClient.invalidateQueries({
							queryKey: ["localWhisperBackendStatus"],
						});
					},
				);

				// React StrictMode (dev) mounts effects twice; if we were disposed before
				// `listen()` resolves, immediately unregister to avoid leaked duplicate listeners.
				if (disposed) {
					nextUnlisten();
					return;
				}

				unlisten = nextUnlisten;
			} catch (e) {
				console.warn(
					"Failed to listen for local whisper model load events:",
					e,
				);
			}
		};

		void setup();

		return () => {
			disposed = true;
			unlisten?.();
		};
	}, [queryClient]);

	const contentDisabled = !isAvailable;

	const models: WhisperModelInfo[] = whisperModels.data ?? [];
	const modelsSortedBySizeDesc = [...models].sort(
		(a, b) => b.size_bytes - a.size_bytes,
	);

	const storedActiveModelId = settings?.local_whisper_model_id ?? null;
	const activeModelId = (storedActiveModelId ?? "base").toLowerCase();
	const activeModel = models.find((m) => m.id === activeModelId) ?? null;
	const localWhisperLoadMode = settings?.local_whisper_load_mode ?? "manual";
	const sttProvider = settings?.stt_provider ?? null;
	const isSttProviderLocalWhisper =
		sttProvider === "whisper" || sttProvider === "local-whisper";

	const isModelLoaded = localWhisperLoaded.data ?? false;

	const backend = localWhisperBackendStatus.data ?? null;
	const compute = backend?.compute ?? null;
	const computeLabel =
		compute === "cuda" ? "GPU (CUDA)" : compute === "cpu" ? "CPU" : "Unknown";
	const computeColor = compute === "cuda" ? "green" : "gray";
	const observed = backend?.observed ?? null;
	const observedLabel = (() => {
		if (!observed) return "Observed: Unknown";
		if (!observed.nvidia_smi_available)
			return "Observed: nvidia-smi unavailable";
		if (observed.cuda_process_present) return "Observed: CUDA active";
		return "Observed: not seen";
	})();
	const observedColor = (() => {
		if (!observed) return "gray";
		if (!observed.nvidia_smi_available) return "gray";
		if (observed.cuda_process_present) return "green";
		return "gray";
	})();

	const downloadedModelOptions = modelsSortedBySizeDesc
		.filter((m) => m.is_downloaded)
		.map((m) => ({ value: m.id, label: m.name }));

	const selectedModelValue = downloadedModelOptions.some(
		(o) => o.value === activeModelId,
	)
		? activeModelId
		: null;

	const renderModels = () => {
		if (!isAvailable) {
			return (
				<Text size="sm" c="dimmed">
					This build of Kolboo was compiled without Local Whisper. Download the
					“local-whisper” build variant to enable offline transcription.
				</Text>
			);
		}

		if (whisperModels.isLoading) {
			return (
				<Text size="sm" c="dimmed">
					Loading models…
				</Text>
			);
		}

		if (whisperModels.isError) {
			return (
				<Text size="sm" c="dimmed">
					Unable to load models.
				</Text>
			);
		}

		return (
			<Stack gap={10}>
				<Card
					withBorder
					radius="md"
					padding="md"
					style={{
						background: "var(--bg-elevated)",
						borderColor: "var(--border-default)",
					}}
				>
					<Text size="sm" c="dimmed" mb={10}>
						Local Whisper runs when your STT provider is set to Local Whisper.
						{isSttProviderLocalWhisper
							? ""
							: ` Currently selected: ${sttProvider ?? "(none)"}.`}
					</Text>

					<SimpleGrid cols={{ base: 1, sm: 2 }} spacing={10}>
						<div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
							<Text size="sm" fw={600}>
								Active model
							</Text>
							<Text size="xs" c="dimmed">
								{activeModel?.is_downloaded
									? "Changing the model unloads the current model. The new model won’t auto-load."
									: "Download the active model (or pick another downloaded model)."}
							</Text>
						</div>

						<Select
							data={downloadedModelOptions}
							value={selectedModelValue}
							placeholder={
								downloadedModelOptions.length > 0
									? "Choose a downloaded model"
									: "Download a model to select"
							}
							disabled={!isAvailable || downloadedModelOptions.length === 0}
							onChange={(value) => {
								if (!value) return;

								const update = () => {
									updateLocalWhisperModelId.mutate(value);
								};

								// Model selection should NOT auto-load the new model.
								// If a model is currently loaded, unload it first.
								if (isModelLoaded) {
									unloadLocalWhisperModel.mutate(undefined, {
										onSettled: () => {
											update();
										},
									});
								} else {
									update();
								}
							}}
							styles={{
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
								},
							}}
						/>
					</SimpleGrid>

					<Divider my={10} />

					<Group justify="space-between" align="center" wrap="wrap" gap={10}>
						<div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
							<Text size="sm" fw={600}>
								Model load
							</Text>
							<Text size="xs" c="dimmed">
								Manual prevents auto-loading during transcription.
							</Text>
						</div>

						<Group gap={8} wrap="wrap">
							<SegmentedControl
								size="sm"
								value={localWhisperLoadMode}
								disabled={!isAvailable}
								data={[
									{ value: "manual", label: "Manual" },
									{ value: "on_transcribe", label: "On transcribe" },
									{ value: "on_launch", label: "On launch" },
								]}
								onChange={(value) => {
									const nextMode: LocalWhisperLoadMode | null =
										value === "manual" ||
										value === "on_transcribe" ||
										value === "on_launch"
											? value
											: null;

									if (!nextMode) return;

									updateLocalWhisperLoadMode.mutate(nextMode);
								}}
							/>

							<Button
								size="sm"
								color={isModelLoaded ? "gray" : "orange"}
								variant={isModelLoaded ? "light" : "filled"}
								disabled={!isAvailable || !activeModel?.is_downloaded}
								loading={
									isLocalWhisperLoading ||
									loadLocalWhisperModel.isPending ||
									unloadLocalWhisperModel.isPending
								}
								onClick={() => {
									if (isModelLoaded) {
										unloadLocalWhisperModel.mutate();
										return;
									}

									loadLocalWhisperModel.mutate();
								}}
							>
								{isModelLoaded ? "Unload model" : "Load model"}
							</Button>
						</Group>
					</Group>

					<Group gap={8} wrap="wrap" mt={10}>
						<Badge
							size="sm"
							color={isModelLoaded ? "green" : "gray"}
							variant="light"
						>
							{isModelLoaded ? "Loaded" : "Not loaded"}
						</Badge>

						<Tooltip
							label={backend?.reason ?? null}
							withArrow
							disabled={!backend?.reason}
						>
							<Badge size="sm" color={computeColor} variant="light">
								Compute: {computeLabel}
							</Badge>
						</Tooltip>

						<Badge size="sm" color={observedColor} variant="light">
							{observedLabel}
						</Badge>

						<Button
							size="xs"
							variant="subtle"
							color="gray"
							onClick={() => setShowDiagnostics((v) => !v)}
						>
							{showDiagnostics ? "Hide diagnostics" : "Show diagnostics"}
						</Button>
					</Group>

					<Text size="xs" c="dimmed" mt={6}>
						Compute shows what Kolboo thinks it can use (availability/request).
						Observed shows what nvidia-smi reports for this process.
					</Text>

					<Collapse expanded={showDiagnostics}>
						<Stack gap={6} mt={10}>
							{observed ? (
								<Alert
									color="gray"
									variant="light"
									title="Observed (nvidia-smi)"
								>
									<Text size="xs" c="dimmed">
										PID: {observed.pid}
									</Text>
									{observed.nvidia_smi_available ? (
										<Text size="xs" c="dimmed">
											Used GPU memory (MB):{" "}
											{observed.used_gpu_memory_mb ?? "unknown"}
										</Text>
									) : (
										<Text size="xs" c="dimmed">
											nvidia-smi error: {observed.error ?? "unknown"}
										</Text>
									)}
								</Alert>
							) : null}

							{backend &&
							backend.compute === "cpu" &&
							backend.build_has_cuda ? (
								<Alert color="yellow" variant="light" title="CUDA unavailable">
									<Text size="xs" c="dimmed">
										{backend.reason ?? "Unknown reason"}
									</Text>
									{backend.missing_dlls?.length ? (
										<Text
											size="xs"
											c="dimmed"
											style={{ fontFamily: "monospace" }}
										>
											Missing: {backend.missing_dlls.join(", ")}
										</Text>
									) : null}
								</Alert>
							) : null}
						</Stack>
					</Collapse>

					{storedActiveModelId && !activeModel?.is_downloaded ? (
						<Alert
							color="red"
							variant="light"
							mt={10}
							title="Active model missing"
						>
							<Text size="xs" c="dimmed">
								Active model “{activeModelId}” isn’t downloaded yet.
							</Text>
						</Alert>
					) : null}

					{isAvailable &&
					activeModel?.is_downloaded &&
					!isModelLoaded &&
					localWhisperLoadMode === "manual" ? (
						<Alert
							color="gray"
							variant="light"
							mt={10}
							title="Manual load enabled"
						>
							<Text size="xs" c="dimmed">
								Click “Load model” before transcribing.
							</Text>
						</Alert>
					) : null}
				</Card>

				{modelsSortedBySizeDesc.map((m) => {
					const progress = progressById[m.id];
					const isDownloading =
						progress &&
						progress.status !== "completed" &&
						progress.status !== "cancelled" &&
						progress.status !== "error";

					const validation = validateResultById[m.id];
					const error = errorById[m.id];

					return (
						<Card
							key={m.id}
							withBorder
							radius="md"
							padding="md"
							style={{
								background: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
							}}
						>
							<Group
								justify="space-between"
								align="center"
								wrap="wrap"
								gap={10}
							>
								<div
									style={{ display: "flex", flexDirection: "column", gap: 2 }}
								>
									<Group gap={8} align="center" wrap="wrap">
										<Text fw={600}>{m.name}</Text>
										{m.id === activeModelId ? (
											<Badge size="sm" color="orange" variant="light">
												Active
											</Badge>
										) : null}
										{m.is_downloaded ? (
											<Badge size="sm" color="green" variant="light">
												Downloaded
											</Badge>
										) : null}
									</Group>
									<Text size="xs" c="dimmed">
										{m.filename} • {m.size_display}
									</Text>
								</div>

								<Group gap={8}>
									{!m.is_downloaded ? (
										<>
											<Button
												size="sm"
												color="green"
												onClick={() => {
													setErrorById((prev) => ({ ...prev, [m.id]: null }));
													// Optimistic UI: show queued immediately so the user sees
													// something even before the first progress event arrives.
													setProgressById((prev) => ({
														...prev,
														[m.id]: {
															model_id: m.id,
															status: "queued",
															downloaded_bytes: 0,
															total_bytes: null,
															percent: null,
															message: null,
														},
													}));

													downloadModel.mutate(m.id, {
														onError: (err) => {
															const msg =
																err instanceof Error
																	? err.message
																	: String(err);

															setErrorById((prev) => ({
																...prev,
																[m.id]: msg,
															}));

															setProgressById((prev) => ({
																...prev,
																[m.id]: {
																	model_id: m.id,
																	status: "error",
																	downloaded_bytes: 0,
																	total_bytes: null,
																	percent: null,
																	message: msg,
																},
															}));
														},
													});
												}}
												disabled={isDownloading}
											>
												Download
											</Button>
											{isDownloading ? (
												<Button
													size="sm"
													variant="default"
													onClick={() => cancelDownload.mutate(m.id)}
												>
													Cancel
												</Button>
											) : null}
										</>
									) : (
										<>
											<Button
												size="sm"
												variant="default"
												onClick={() => {
													setValidateResultById((prev) => ({
														...prev,
														[m.id]: null,
													}));

													setValidatingModelId(m.id);

													validateModel.mutate(m.id, {
														onSuccess: (ok) => {
															setValidateResultById((prev) => ({
																...prev,
																[m.id]: ok,
															}));
														},
														onSettled: () => {
															setValidatingModelId((current) =>
																current === m.id ? null : current,
															);
														},
													});
												}}
												loading={validatingModelId === m.id}
												disabled={
													validatingModelId != null &&
													validatingModelId !== m.id
												}
											>
												Validate
											</Button>
											<Button
												size="sm"
												color="red"
												variant="light"
												onClick={() => {
													setDeletingModelId(m.id);
													deleteModel.mutate(m.id, {
														onSettled: () => {
															setDeletingModelId((current) =>
																current === m.id ? null : current,
															);
														},
													});
												}}
												loading={deletingModelId === m.id}
												disabled={
													deletingModelId != null && deletingModelId !== m.id
												}
											>
												Delete
											</Button>
										</>
									)}
								</Group>
							</Group>

							{isDownloading ? (
								<div style={{ marginTop: 10 }}>
									<Text size="xs" c="dimmed" mb={6}>
										{progress.status === "queued" ? "Queued" : null}
										{progress.status === "downloading" ? "Downloading" : null}
										{progress.status === "verifying" ? "Verifying" : null}
										{progress.message ? ` • ${progress.message}` : null}
									</Text>
									<Progress
										value={progress.percent ?? 0}
										animated
										color="orange"
									/>
								</div>
							) : null}

							{validation != null ? (
								<Text size="xs" mt={8} c={validation ? "green" : "red"}>
									{validation
										? "Model file verified (SHA-256)."
										: "Model file looks invalid/corrupt. Try re-downloading."}
								</Text>
							) : null}

							{error ? (
								<Text size="xs" mt={8} c="red">
									{error}
								</Text>
							) : null}
						</Card>
					);
				})}
			</Stack>
		);
	};

	return (
		<Card
			withBorder
			mt={18}
			padding="md"
			style={{
				background: "var(--bg-elevated)",
				borderColor: "var(--border-default)",
			}}
		>
			<Group justify="space-between" align="center" mb={6}>
				<Group gap={10} align="center">
					<Text fw={700}>Local Whisper models</Text>
					{!isAvailable ? (
						<Badge size="sm" variant="light" color="gray">
							Unavailable
						</Badge>
					) : (
						<Badge size="sm" variant="light" color="green">
							Offline
						</Badge>
					)}
				</Group>

				<Tooltip label={modelsDir ?? ""} withArrow disabled={!modelsDir}>
					<Text
						size="xs"
						c="dimmed"
						style={{ fontFamily: "monospace", maxWidth: 420 }}
						lineClamp={1}
					>
						{modelsDir ?? ""}
					</Text>
				</Tooltip>
			</Group>

			<Text size="sm" c="dimmed" mb={12}>
				Download and manage offline Whisper models used by the Local Whisper STT
				provider. Downloads are verified with SHA-256.
			</Text>

			{contentDisabled ? (
				<div style={{ opacity: 0.55 }}>{renderModels()}</div>
			) : (
				renderModels()
			)}
		</Card>
	);
}

export function ApiKeysSettings({
	editingProfileId,
}: {
	editingProfileId?: string;
}) {
	const isProfileScope = editingProfileId && editingProfileId !== "default";

	const { data: settings } = useSettings();
	const byokLlmModelsQuery = useByokLlmModels(true);
	const updateOllamaUrl = useUpdateOllamaUrl();
	const updateWhisperServerBaseUrl = useUpdateWhisperServerBaseUrl();

	const [ollamaUrlDraft, setOllamaUrlDraft] = useState(
		settings?.ollama_url ?? "",
	);
	const [whisperServerBaseUrlDraft, setWhisperServerBaseUrlDraft] = useState(
		settings?.whisper_server_base_url ?? "",
	);

	useEffect(() => {
		setOllamaUrlDraft(settings?.ollama_url ?? "");
		setWhisperServerBaseUrlDraft(settings?.whisper_server_base_url ?? "");
	}, [settings?.ollama_url, settings?.whisper_server_base_url]);

	const content = (
		<>
			{API_KEYS.map((config) => (
				<ApiKeyInput
					key={config.id}
					config={config}
					llmModels={byokLlmModelsQuery.data}
				/>
			))}

			<SettingsRow
				label="Ollama server URL"
				description={
					<>
						Base URL for your local Ollama server (e.g. http://localhost:11434).
						<br />
						Models are discovered automatically.
					</>
				}
				right={
					<TextInput
						value={ollamaUrlDraft}
						onChange={(e) => setOllamaUrlDraft(e.currentTarget.value)}
						onBlur={() => {
							const trimmed = ollamaUrlDraft.trim();
							const normalized = trimmed ? trimmed : null;
							updateOllamaUrl.mutate(normalized);
						}}
						placeholder="http://localhost:11434"
						styles={{
							input: {
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 280,
							},
						}}
					/>
				}
			/>

			<SettingsRow
				label="Whisper server URL"
				description={
					"Base URL for an OpenAI-compatible transcription API (e.g. http://localhost:8000/v1)"
				}
				right={
					<TextInput
						value={whisperServerBaseUrlDraft}
						onChange={(e) =>
							setWhisperServerBaseUrlDraft(e.currentTarget.value)
						}
						onBlur={() => {
							const trimmed = whisperServerBaseUrlDraft.trim();
							const normalized = trimmed ? trimmed : null;
							updateWhisperServerBaseUrl.mutate(normalized);
						}}
						placeholder="http://localhost:8000/v1"
						styles={{
							input: {
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 280,
							},
						}}
					/>
				}
			/>

			<OcrProviderSettings editingProfileId={editingProfileId} />

			<LocalWhisperModelsCard />
		</>
	);

	if (isProfileScope) {
		return (
			<Tooltip label={GLOBAL_ONLY_TOOLTIP} withArrow position="top-start">
				<div style={{ opacity: 0.5, cursor: "not-allowed" }}>
					<div style={{ pointerEvents: "none" }}>{content}</div>
				</div>
			</Tooltip>
		);
	}

	return content;
}
