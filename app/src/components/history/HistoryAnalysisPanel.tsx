import {
	ActionIcon,
	Badge,
	Box,
	Button,
	Drawer,
	Group,
	Modal,
	NumberInput,
	SegmentedControl,
	Select,
	Stack,
	Text,
	Textarea,
	Tooltip,
} from "@mantine/core";
import { Copy, Send } from "lucide-react";
import type { AnalysisPromptStyle } from "../../lib/history/readModel";
import { analysisStyleLabel } from "../../lib/history/readModel";
import type { LlmProviderInfo } from "../../lib/tauri";

export function HistoryAnalysisPanel({
	analysisOpened,
	onCloseAnalysis,
	analysisIncludedCount,
	analysisEstimatedTokens,
	analysisAvailableTranscriptsCount,
	analysisIncludeFromLastHoursInput,
	onAnalysisIncludeFromLastHoursInputChange,
	analysisPromptStyle,
	onAnalysisPromptStyleChange,
	onGenerateAnalysisPrompt,
	isAnalysisLoading,
	analysisPrompt,
	onAnalysisPromptChange,
	onCopyAnalysisPrompt,
	hasAnyLlmProviders,
	onOpenSendDrawer,
	sendDrawerOpened,
	onCloseSendDrawer,
	isNarrow,
	llmProviders,
	sendProvider,
	onSendProviderChange,
	sendModel,
	onSendModelChange,
	sendProviderUsed,
	sendModelUsed,
	onGenerateSendOutput,
	isSendPending,
	sendOutput,
	onSendOutputChange,
	onCopySendOutput,
}: {
	analysisOpened: boolean;
	onCloseAnalysis: () => void;
	analysisIncludedCount: number;
	analysisEstimatedTokens: number;
	analysisAvailableTranscriptsCount: number;
	analysisIncludeFromLastHoursInput: string | number;
	onAnalysisIncludeFromLastHoursInputChange: (value: string | number) => void;
	analysisPromptStyle: AnalysisPromptStyle;
	onAnalysisPromptStyleChange: (style: AnalysisPromptStyle) => void;
	onGenerateAnalysisPrompt: () => void;
	isAnalysisLoading: boolean;
	analysisPrompt: string;
	onAnalysisPromptChange: (value: string) => void;
	onCopyAnalysisPrompt: () => void;
	hasAnyLlmProviders: boolean;
	onOpenSendDrawer: () => void;
	sendDrawerOpened: boolean;
	onCloseSendDrawer: () => void;
	isNarrow: boolean;
	llmProviders: LlmProviderInfo[] | undefined;
	sendProvider: string | null;
	onSendProviderChange: (providerId: string | null) => void;
	sendModel: string | null;
	onSendModelChange: (modelId: string | null) => void;
	sendProviderUsed: string;
	sendModelUsed: string;
	onGenerateSendOutput: () => void;
	isSendPending: boolean;
	sendOutput: string;
	onSendOutputChange: (value: string) => void;
	onCopySendOutput: () => void;
}) {
	const selectedProvider = (llmProviders ?? []).find(
		(provider) => provider.id === sendProvider,
	);

	return (
		<>
			<AnalysisPromptModal
				analysisOpened={analysisOpened}
				onCloseAnalysis={onCloseAnalysis}
				analysisIncludedCount={analysisIncludedCount}
				analysisEstimatedTokens={analysisEstimatedTokens}
				analysisAvailableTranscriptsCount={analysisAvailableTranscriptsCount}
				analysisIncludeFromLastHoursInput={analysisIncludeFromLastHoursInput}
				onAnalysisIncludeFromLastHoursInputChange={
					onAnalysisIncludeFromLastHoursInputChange
				}
				analysisPromptStyle={analysisPromptStyle}
				onAnalysisPromptStyleChange={onAnalysisPromptStyleChange}
				onGenerateAnalysisPrompt={onGenerateAnalysisPrompt}
				isAnalysisLoading={isAnalysisLoading}
				analysisPrompt={analysisPrompt}
				onAnalysisPromptChange={onAnalysisPromptChange}
				onCopyAnalysisPrompt={onCopyAnalysisPrompt}
				hasAnyLlmProviders={hasAnyLlmProviders}
				onOpenSendDrawer={onOpenSendDrawer}
			/>

			<Drawer
				opened={sendDrawerOpened}
				onClose={onCloseSendDrawer}
				title="Send to LLM"
				position={isNarrow ? "bottom" : "right"}
				size={isNarrow ? "70%" : 460}
			>
				<Stack gap="sm">
					<Group grow>
						<Select
							label="Provider"
							placeholder="Select provider"
							data={(llmProviders ?? []).map((provider) => ({
								value: provider.id,
								label: provider.name,
							}))}
							value={sendProvider}
							onChange={onSendProviderChange}
							renderOption={({ option }) => option.label}
							styles={{
								input: {
									backgroundColor: "transparent",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
								},
								dropdown: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
								},
							}}
						/>

						<Select
							label="Model"
							placeholder="Select model"
							data={(selectedProvider?.models ?? []).map((model) => ({
								value: model,
								label: model,
							}))}
							value={sendModel}
							onChange={onSendModelChange}
							searchable
							renderOption={({ option }) => option.label}
							styles={{
								input: {
									backgroundColor: "transparent",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
								},
								dropdown: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
								},
							}}
						/>
					</Group>

					<Group justify="space-between" align="center">
						<Text size="xs" c="dimmed">
							{sendProviderUsed && sendModelUsed
								? `Used: ${sendProviderUsed} • ${sendModelUsed}`
								: ""}
						</Text>

						<Group gap={8}>
							<Button
								variant="light"
								color="gray"
								loading={isSendPending}
								onClick={onGenerateSendOutput}
							>
								Generate
							</Button>

							<Button
								variant="subtle"
								color="gray"
								leftSection={<Copy size={14} />}
								onClick={onCopySendOutput}
								disabled={sendOutput.trim().length === 0}
							>
								Copy
							</Button>
						</Group>
					</Group>

					<Textarea
						value={sendOutput}
						onChange={(event) => onSendOutputChange(event.currentTarget.value)}
						placeholder="LLM output will appear here…"
						styles={{
							input: {
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								fontFamily: "monospace",
								fontSize: "13px",
								height: 300,
								overflowY: "auto",
								resize: "none",
							},
						}}
					/>
				</Stack>
			</Drawer>
		</>
	);
}

function AnalysisPromptModal({
	analysisOpened,
	onCloseAnalysis,
	analysisIncludedCount,
	analysisEstimatedTokens,
	analysisAvailableTranscriptsCount,
	analysisIncludeFromLastHoursInput,
	onAnalysisIncludeFromLastHoursInputChange,
	analysisPromptStyle,
	onAnalysisPromptStyleChange,
	onGenerateAnalysisPrompt,
	isAnalysisLoading,
	analysisPrompt,
	onAnalysisPromptChange,
	onCopyAnalysisPrompt,
	hasAnyLlmProviders,
	onOpenSendDrawer,
}: {
	analysisOpened: boolean;
	onCloseAnalysis: () => void;
	analysisIncludedCount: number;
	analysisEstimatedTokens: number;
	analysisAvailableTranscriptsCount: number;
	analysisIncludeFromLastHoursInput: string | number;
	onAnalysisIncludeFromLastHoursInputChange: (value: string | number) => void;
	analysisPromptStyle: AnalysisPromptStyle;
	onAnalysisPromptStyleChange: (style: AnalysisPromptStyle) => void;
	onGenerateAnalysisPrompt: () => void;
	isAnalysisLoading: boolean;
	analysisPrompt: string;
	onAnalysisPromptChange: (value: string) => void;
	onCopyAnalysisPrompt: () => void;
	hasAnyLlmProviders: boolean;
	onOpenSendDrawer: () => void;
}) {
	return (
		<Modal
			opened={analysisOpened}
			onClose={onCloseAnalysis}
			title="Analyze transcripts"
			centered
			size="lg"
		>
			<Text size="sm" c="dimmed" mb="sm">
				Build a prompt from your saved transcripts, then copy it or send it to a
				provider.
			</Text>

			<Box
				mb="sm"
				style={{
					border: "1px solid var(--border-default)",
					borderRadius: 10,
					padding: 10,
					background: "var(--bg-elevated)",
				}}
			>
				<Group justify="space-between" align="center" wrap="wrap" gap={10}>
					<Group gap={6}>
						<Badge size="sm" variant="light" color="gray">
							{analysisIncludedCount} transcript
							{analysisIncludedCount === 1 ? "" : "s"}
						</Badge>
						<Badge size="sm" variant="light" color="gray">
							~{analysisEstimatedTokens.toLocaleString()} tokens
						</Badge>
						{analysisAvailableTranscriptsCount > 0 ? (
							<Badge size="sm" variant="light" color="gray">
								{analysisAvailableTranscriptsCount} with transcripts
							</Badge>
						) : null}
					</Group>

					<Group gap={8} wrap="wrap" align="center">
						<NumberInput
							value={analysisIncludeFromLastHoursInput}
							onChange={onAnalysisIncludeFromLastHoursInputChange}
							placeholder="All time"
							min={0}
							step={0.5}
							hideControls
							decimalScale={2}
							allowNegative={false}
							size="xs"
							w={140}
							leftSection={
								<Text size="xs" c="dimmed">
									hrs
								</Text>
							}
							styles={{
								input: {
									backgroundColor: "transparent",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
								},
							}}
						/>

						<SegmentedControl
							size="xs"
							value={analysisPromptStyle}
							onChange={(value) =>
								onAnalysisPromptStyleChange(value as AnalysisPromptStyle)
							}
							data={(["productive", "insightful", "structured"] as const).map(
								(style) => ({
									value: style,
									label: analysisStyleLabel(style),
								}),
							)}
							styles={{
								root: {
									backgroundColor: "transparent",
									border: "1px solid var(--border-default)",
								},
								label: { color: "var(--text-primary)" },
							}}
						/>

						<Button
							size="xs"
							color="orange"
							onClick={onGenerateAnalysisPrompt}
							loading={isAnalysisLoading}
						>
							Generate
						</Button>

						<Tooltip label="Copy prompt" withArrow>
							<ActionIcon
								variant="subtle"
								color="gray"
								onClick={onCopyAnalysisPrompt}
								disabled={analysisPrompt.trim().length === 0}
								aria-label="Copy prompt"
							>
								<Copy size={16} />
							</ActionIcon>
						</Tooltip>

						<Tooltip
							label={
								hasAnyLlmProviders
									? "Send to LLM"
									: "No LLM providers are configured"
							}
							withArrow
						>
							<span style={{ display: "inline-flex" }}>
								<ActionIcon
									variant="subtle"
									color="gray"
									disabled={!hasAnyLlmProviders}
									aria-label="Send to LLM"
									onClick={onOpenSendDrawer}
								>
									<Send size={16} />
								</ActionIcon>
							</span>
						</Tooltip>
					</Group>
				</Group>
			</Box>

			<Textarea
				value={analysisPrompt}
				onChange={(event) => onAnalysisPromptChange(event.currentTarget.value)}
				placeholder="Click Generate to create a prompt. Then copy it or send it to a provider."
				styles={{
					input: {
						backgroundColor: "var(--bg-elevated)",
						borderColor: "var(--border-default)",
						color: "var(--text-primary)",
						fontFamily: "monospace",
						fontSize: "13px",
						height: 360,
						overflowY: "auto",
						resize: "none",
					},
				}}
			/>
		</Modal>
	);
}
