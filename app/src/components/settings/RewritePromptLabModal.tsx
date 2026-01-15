import {
	Button,
	Divider,
	Group,
	Modal,
	SegmentedControl,
	Select,
	SimpleGrid,
	Stack,
	Text,
	Textarea,
} from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { llmAPI } from "../../lib/tauri";

function errorToMessage(err: unknown): string {
	if (err instanceof Error) return err.message;
	if (typeof err === "string") return err;
	try {
		return JSON.stringify(err);
	} catch {
		return String(err);
	}
}

export function RewritePromptLabModal(props: {
	opened: boolean;
	onClose: () => void;

	profileId: string;
	profileLabel: string;

	// Seed values when opening the modal.
	initialTranscript?: string;
	initialProblemOutput?: string;

	// Computed prompt (edited elsewhere).
	currentPrompt: string;

	// Optional initial model selection (for convenience when opening the modal).
	initialLlmProvider?: string | null;
	initialLlmModel?: string | null;

	// Actions
	onIteratePrompt: (params: {
		profileId: string;
		mode: "fixed" | "new";
		transcript: string;
		problemOutput: string;
		desiredOutput?: string;
		currentPrompt: string;

		llmProvider?: string;
		llmModel?: string;
		openAiReasoningEffort?: "none" | "low" | "medium" | "high" | null;
		geminiThinkingLevel?: "minimal" | "low" | "medium" | "high" | null;
		geminiThinkingBudget?: number | null;
		anthropicThinkingBudget?: number | null;
	}) => Promise<{
		improvedPrompt: string;
		providerUsed: string;
		modelUsed: string;
	}>;

	onSetPrompt: (prompt: string) => void;

	onTestPrompt: (params: {
		profileId: string;
		transcript: string;
		prompt: string;
	}) => Promise<{ output: string; providerUsed: string; modelUsed: string }>;
}) {
	const { data: llmProviders } = useQuery({
		queryKey: ["llmProviders"],
		queryFn: () => llmAPI.getLlmProviders(),
		staleTime: 60_000,
		refetchOnWindowFocus: false,
	});

	const [selectedProvider, setSelectedProvider] = useState<string | null>(null);
	const [selectedModel, setSelectedModel] = useState<string | null>(null);

	// One dropdown, but maps to different backend knobs depending on provider/model.
	const [thinkingValue, setThinkingValue] = useState<string>("default");

	const [mode, setMode] = useState<"fixed" | "new">("fixed");

	const [transcript, setTranscript] = useState("");
	const [problemOutput, setProblemOutput] = useState("");
	const [promptGoal, setPromptGoal] = useState("");
	const [desiredOutput, setDesiredOutput] = useState("");

	const [improvedPrompt, setImprovedPrompt] = useState("");
	const [testedOutput, setTestedOutput] = useState("");

	const [improveError, setImproveError] = useState<string>("");
	const [testError, setTestError] = useState<string>("");

	const [improveMeta, setImproveMeta] = useState<string>("");
	const [testMeta, setTestMeta] = useState<string>("");

	const [isImproving, setIsImproving] = useState(false);
	const [isTesting, setIsTesting] = useState(false);

	// If providers load and we don't have a selection yet, default to initial or first provider.
	useEffect(() => {
		if (!props.opened) return;
		if (!llmProviders || llmProviders.length === 0) return;

		if (!selectedProvider) {
			const preferred = props.initialLlmProvider ?? null;
			const exists = preferred
				? llmProviders.some((p) => p.id === preferred)
				: false;
			setSelectedProvider(exists ? preferred : (llmProviders[0]?.id ?? null));
		}
	}, [llmProviders, props.initialLlmProvider, props.opened, selectedProvider]);

	// When provider changes, ensure model is valid for that provider.
	useEffect(() => {
		if (!props.opened) return;
		if (!selectedProvider) return;
		const provider = (llmProviders ?? []).find(
			(p) => p.id === selectedProvider,
		);
		const models = provider?.models ?? [];

		if (models.length === 0) {
			setSelectedModel(null);
			return;
		}

		if (!selectedModel || !models.includes(selectedModel)) {
			const preferred = props.initialLlmModel ?? null;
			const usePreferred = preferred && models.includes(preferred);
			setSelectedModel(usePreferred ? preferred : (models[0] ?? null));
		}
	}, [
		llmProviders,
		props.initialLlmModel,
		props.opened,
		selectedModel,
		selectedProvider,
	]);

	// Seed values on open.
	useEffect(() => {
		if (!props.opened) return;

		setMode("fixed");
		setTranscript(props.initialTranscript ?? "");
		setProblemOutput(props.initialProblemOutput ?? "");
		setPromptGoal("");
		setDesiredOutput("");
		setImprovedPrompt("");
		setTestedOutput("");
		setImproveError("");
		setTestError("");
		setImproveMeta("");
		setTestMeta("");

		setSelectedProvider(props.initialLlmProvider ?? null);
		setSelectedModel(props.initialLlmModel ?? null);
		setThinkingValue("default");
	}, [props.opened, props.initialTranscript, props.initialProblemOutput]);

	const supportsOpenAiReasoningEffort =
		selectedProvider === "openai" &&
		!!selectedModel &&
		(selectedModel.startsWith("gpt-5") || selectedModel.startsWith("o"));

	const supportsGeminiThinkingLevel =
		selectedProvider === "gemini" &&
		!!selectedModel &&
		selectedModel.includes("gemini-3");

	const supportsGeminiThinkingBudget =
		selectedProvider === "gemini" &&
		!!selectedModel &&
		selectedModel.includes("gemini-2.5") &&
		!selectedModel.includes("flash-lite");

	const supportsAnthropicThinkingBudget =
		selectedProvider === "anthropic" &&
		!!selectedModel &&
		(selectedModel.includes("claude-3-7") ||
			selectedModel.includes("claude-4") ||
			selectedModel.includes("-4-"));

	const openAiThinkingEffortsForModel = (model: string): string[] => {
		// Keep aligned with ProvidersSettings.
		if (model.startsWith("gpt-5-pro")) return ["high"];
		if (model.startsWith("gpt-5.2") || model.startsWith("gpt-5.1"))
			return ["none", "low", "medium", "high"];
		if (model.startsWith("gpt-5")) return ["low", "medium", "high"];
		if (model.startsWith("o")) return ["low", "medium", "high"];
		return [];
	};

	const thinkingSelectKind:
		| "none"
		| "openai"
		| "geminiLevel"
		| "geminiBudget"
		| "anthropicBudget" = supportsOpenAiReasoningEffort
		? "openai"
		: supportsGeminiThinkingLevel
			? "geminiLevel"
			: supportsGeminiThinkingBudget
				? "geminiBudget"
				: supportsAnthropicThinkingBudget
					? "anthropicBudget"
					: "none";

	const thinkingOptions = useMemo((): Array<{
		value: string;
		label: string;
	}> => {
		if (thinkingSelectKind === "openai") {
			const model = selectedModel ?? "";
			return [
				{ value: "default", label: "Default" },
				...openAiThinkingEffortsForModel(model).map((v) => ({
					value: v,
					label: v === "none" ? "None" : v.charAt(0).toUpperCase() + v.slice(1),
				})),
			];
		}

		if (thinkingSelectKind === "geminiLevel") {
			const isFlash = (selectedModel ?? "").includes("gemini-3-flash");
			const base = [{ value: "default", label: "Default" }];
			return isFlash
				? base.concat([
						{ value: "minimal", label: "Minimal" },
						{ value: "low", label: "Low" },
						{ value: "medium", label: "Medium" },
						{ value: "high", label: "High" },
					])
				: base.concat([
						{ value: "low", label: "Low" },
						{ value: "high", label: "High" },
					]);
		}

		if (thinkingSelectKind === "geminiBudget") {
			const model = selectedModel ?? "";
			const isPro = model.includes("gemini-2.5-pro");
			const max = isPro ? 32768 : 24576;
			const canDisable = model.includes("gemini-2.5-flash") && !isPro;

			return [
				{ value: "default", label: "Default" },
				{ value: "-1", label: "Dynamic (-1)" },
				...(canDisable ? [{ value: "0", label: "Off (0)" }] : []),
				...(isPro ? [{ value: "128", label: "Minimal (128)" }] : []),
				{ value: "1024", label: "Light (1024)" },
				{ value: "4096", label: "Medium (4096)" },
				{ value: "8192", label: "High (8192)" },
				{ value: "16384", label: "Very High (16384)" },
				...(max > 16384 ? [{ value: String(max), label: `Max (${max})` }] : []),
			];
		}

		if (thinkingSelectKind === "anthropicBudget") {
			return [
				{ value: "default", label: "Default" },
				{ value: "0", label: "Off" },
				{ value: "2000", label: "Low" },
				{ value: "4000", label: "Medium" },
				{ value: "8000", label: "High" },
				{ value: "32000", label: "Max" },
			];
		}

		return [{ value: "default", label: "Not supported" }];
	}, [selectedModel, thinkingSelectKind]);

	// Switching modes changes the meaning of inputs, so clear derived outputs.
	useEffect(() => {
		setImprovedPrompt("");
		setTestedOutput("");
		setImproveError("");
		setTestError("");
		setImproveMeta("");
		setTestMeta("");
	}, [mode]);

	const canImprove = useMemo(() => {
		const hasModeSpecificInputs =
			mode === "fixed"
				? problemOutput.trim().length > 0 &&
					props.currentPrompt.trim().length > 0
				: // New prompt: allow either goal/description OR (transcript + desired output)
					promptGoal.trim().length > 0 ||
					(transcript.trim().length > 0 && desiredOutput.trim().length > 0);

		const hasCoreInputs =
			mode === "fixed"
				? transcript.trim().length > 0 && desiredOutput.trim().length > 0
				: // New prompt mode doesn't require transcript/output when a goal is provided.
					true;

		return hasCoreInputs && hasModeSpecificInputs && !isImproving && !isTesting;
	}, [
		desiredOutput,
		isImproving,
		isTesting,
		mode,
		problemOutput,
		promptGoal,
		props.currentPrompt,
		transcript,
	]);

	const canTest = useMemo(() => {
		return (
			transcript.trim().length > 0 &&
			improvedPrompt.trim().length > 0 &&
			!isTesting
		);
	}, [improvedPrompt, isTesting, transcript]);

	const monospaceStyles = {
		input: {
			backgroundColor: "var(--bg-elevated)",
			borderColor: "var(--border-default)",
			color: "var(--text-primary)",
			fontFamily: "monospace",
			fontSize: "13px",
		},
	} as const;

	return (
		<Modal
			opened={props.opened}
			onClose={() => {
				if (isImproving || isTesting) return;
				props.onClose();
			}}
			title={`Prompt Lab · ${props.profileLabel}`}
			centered
			size="90%"
			styles={{
				body: { paddingTop: 8 },
				content: { maxWidth: 1400 },
			}}
		>
			<Stack gap="sm">
				<Group grow gap="sm" align="flex-end">
					<Select
						label="Provider"
						data={(llmProviders ?? []).map((p) => ({
							value: p.id,
							label: p.name,
						}))}
						value={selectedProvider}
						onChange={(v) => {
							setSelectedProvider(v);
							setThinkingValue("default");
						}}
						placeholder="Select provider"
						withCheckIcon={false}
						disabled={
							isImproving || isTesting || (llmProviders?.length ?? 0) === 0
						}
					/>

					<Select
						label="Model"
						data={(
							(llmProviders ?? []).find((p) => p.id === selectedProvider)
								?.models ?? []
						).map((m) => ({ value: m, label: m }))}
						value={selectedModel}
						onChange={(v) => {
							setSelectedModel(v);
							setThinkingValue("default");
						}}
						placeholder="Select model"
						withCheckIcon={false}
						disabled={isImproving || isTesting || !selectedProvider}
						searchable
					/>

					<Select
						label="Thinking"
						data={thinkingOptions}
						value={thinkingValue}
						onChange={(v) => setThinkingValue(v ?? "default")}
						placeholder={
							thinkingSelectKind === "none" ? "Not supported" : "Default"
						}
						withCheckIcon={false}
						disabled={isImproving || isTesting || thinkingSelectKind === "none"}
					/>
				</Group>

				<Group justify="space-between" align="center" gap="sm">
					<SegmentedControl
						value={mode}
						onChange={(v) => setMode(v as "fixed" | "new")}
						data={[
							{ label: "Fix prompt", value: "fixed" },
							{ label: "New prompt", value: "new" },
						]}
						disabled={isImproving || isTesting}
					/>
					<Text size="xs" c="dimmed">
						{mode === "fixed"
							? "Use before/after outputs to improve the existing prompt."
							: "Describe your goal and generate a fresh prompt from scratch."}
					</Text>
				</Group>

				<Divider
					label="Inputs"
					labelPosition="left"
					styles={{
						root: { borderColor: "var(--border-subtle)" },
						label: {
							color: "var(--text-primary)",
							fontSize: 11,
							fontWeight: 600,
							letterSpacing: "0.08em",
							textTransform: "uppercase",
						},
					}}
				/>

				<SimpleGrid cols={2} spacing="md" verticalSpacing="md">
					<Textarea
						label="Transcript (input)"
						value={transcript}
						onChange={(e) => setTranscript(e.currentTarget.value)}
						rows={6}
						styles={monospaceStyles}
					/>

					{mode === "fixed" ? (
						<Textarea
							label="Current prompt (read-only)"
							value={props.currentPrompt}
							readOnly
							rows={6}
							styles={monospaceStyles}
						/>
					) : (
						<Textarea
							label="Prompt goal / description"
							value={promptGoal}
							onChange={(e) => setPromptGoal(e.currentTarget.value)}
							autosize
							minRows={6}
							placeholder="Describe what the new prompt should accomplish and any rules/constraints to follow."
							styles={monospaceStyles}
						/>
					)}

					{mode === "fixed" ? (
						<Textarea
							label="Problem output (what you got)"
							value={problemOutput}
							onChange={(e) => setProblemOutput(e.currentTarget.value)}
							autosize
							minRows={6}
							styles={monospaceStyles}
						/>
					) : (
						<Textarea
							label="Existing prompt (reference)"
							value={props.currentPrompt}
							readOnly
							rows={6}
							styles={monospaceStyles}
						/>
					)}

					{mode === "new" ? (
						<Textarea
							label="Desired output"
							value={desiredOutput}
							onChange={(e) => setDesiredOutput(e.currentTarget.value)}
							rows={6}
							styles={monospaceStyles}
						/>
					) : (
						<Textarea
							label="Desired output (what you want)"
							value={desiredOutput}
							onChange={(e) => setDesiredOutput(e.currentTarget.value)}
							autosize
							minRows={6}
							styles={monospaceStyles}
						/>
					)}
				</SimpleGrid>

				<Group justify="flex-end" gap="sm">
					<Button
						variant="light"
						color="gray"
						onClick={() => {
							setDesiredOutput("");
							setImprovedPrompt("");
							setTestedOutput("");
							setImproveError("");
							setTestError("");
							setImproveMeta("");
							setTestMeta("");
						}}
						disabled={isImproving || isTesting}
					>
						Clear outputs
					</Button>

					<Button
						color="gray"
						loading={isImproving}
						disabled={!canImprove}
						onClick={async () => {
							setImproveError("");
							setImproveMeta("");
							setImprovedPrompt("");
							setTestedOutput("");
							setTestError("");
							setTestMeta("");

							setIsImproving(true);
							try {
								const selectedThinking =
									thinkingValue && thinkingValue !== "default"
										? thinkingValue
										: null;

								const res = await props.onIteratePrompt({
									profileId: props.profileId,
									mode,
									transcript,
									problemOutput: mode === "fixed" ? problemOutput : promptGoal,
									desiredOutput: desiredOutput.trim().length
										? desiredOutput
										: undefined,
									currentPrompt: props.currentPrompt,

									llmProvider: selectedProvider ?? undefined,
									llmModel: selectedModel ?? undefined,
									openAiReasoningEffort:
										thinkingSelectKind === "openai"
											? (selectedThinking as any)
											: null,
									geminiThinkingLevel:
										thinkingSelectKind === "geminiLevel"
											? (selectedThinking as any)
											: null,
									geminiThinkingBudget:
										thinkingSelectKind === "geminiBudget" && selectedThinking
											? Number(selectedThinking)
											: null,
									anthropicThinkingBudget:
										thinkingSelectKind === "anthropicBudget" && selectedThinking
											? Number(selectedThinking)
											: null,
								});
								setImprovedPrompt(res.improvedPrompt);
								setImproveMeta(
									`${res.providerUsed}${
										res.modelUsed ? ` / ${res.modelUsed}` : ""
									}`,
								);
							} catch (e) {
								setImproveError(errorToMessage(e));
							} finally {
								setIsImproving(false);
							}
						}}
					>
						{mode === "fixed" ? "Improve prompt" : "Create prompt"}
					</Button>

					<Button
						color="gray"
						loading={isTesting}
						disabled={!canTest}
						onClick={async () => {
							setTestError("");
							setTestMeta("");
							setTestedOutput("");

							setIsTesting(true);
							try {
								const res = await props.onTestPrompt({
									profileId: props.profileId,
									transcript,
									prompt: improvedPrompt,
								});
								setTestedOutput(res.output);
								setTestMeta(
									`${res.providerUsed}${
										res.modelUsed ? ` / ${res.modelUsed}` : ""
									}`,
								);
							} catch (e) {
								setTestError(errorToMessage(e));
							} finally {
								setIsTesting(false);
							}
						}}
					>
						Test prompt
					</Button>

					<Button
						color="gray"
						variant="light"
						disabled={
							isImproving ||
							isTesting ||
							improvedPrompt.trim().length === 0 ||
							testedOutput.trim().length === 0
						}
						onClick={() => {
							const next = improvedPrompt.trim();
							if (!next) return;
							props.onSetPrompt(next);
							props.onClose();
						}}
					>
						Set prompt
					</Button>
				</Group>

				{improveError ? (
					<Text size="sm" c="red">
						{improveError}
					</Text>
				) : null}

				{improveMeta ? (
					<Text size="xs" c="dimmed">
						Improved prompt generated with: {improveMeta}
					</Text>
				) : null}

				<Divider
					label="Outputs"
					labelPosition="left"
					styles={{
						root: { borderColor: "var(--border-subtle)" },
						label: {
							color: "var(--text-primary)",
							fontSize: 11,
							fontWeight: 600,
							letterSpacing: "0.08em",
							textTransform: "uppercase",
						},
					}}
				/>

				<SimpleGrid cols={2} spacing="md" verticalSpacing="md">
					<Textarea
						label={mode === "fixed" ? "Improved prompt" : "Generated prompt"}
						value={improvedPrompt}
						readOnly
						placeholder={
							mode === "fixed"
								? "Click “Improve prompt” to generate a candidate prompt."
								: "Click “Create prompt” to generate a candidate prompt."
						}
						autosize
						minRows={8}
						styles={monospaceStyles}
					/>

					<Textarea
						label="Output from improved prompt"
						value={testedOutput}
						readOnly
						placeholder="Click “Test prompt” to run the improved prompt on the transcript."
						autosize
						minRows={8}
						styles={monospaceStyles}
					/>
				</SimpleGrid>

				{testError ? (
					<Text size="sm" c="red">
						{testError}
					</Text>
				) : null}

				{testMeta ? (
					<Text size="xs" c="dimmed">
						Tested with: {testMeta}
					</Text>
				) : null}
			</Stack>
		</Modal>
	);
}
