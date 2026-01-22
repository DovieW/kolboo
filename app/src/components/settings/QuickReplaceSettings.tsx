import { Accordion, Select, Switch } from "@mantine/core";
import { Info, RotateCcw } from "lucide-react";
import type { Dispatch, SetStateAction } from "react";
import type { RewriteProgramPromptProfile } from "../../lib/tauri";
import { PromptSectionEditor } from "./PromptSectionEditor";
import { SettingsIconButton, SettingsRow, SettingsTooltipIcon } from "./SettingsRow";

type LlmOption = { value: string; label: string };
type LlmOptionGroup = { group: string; items: LlmOption[] };

export function QuickReplaceSettings({
	activeProfileId,
	activeProfile,
	isDefaultScope,
	inheritTooltip,
	defaultSystemPrompt,
	defaultQuickReplaceEnabled,
	defaultQuickReplaceIncludeClipboardContext,
	defaultQuickReplaceProvider,
	defaultQuickReplaceModel,
	defaultQuickReplaceSystemPrompt,
	effectiveQuickReplaceProvider,
	llmProviderOptions,
	llmProviderDisabled,
	quickReplaceModelOptions,
	selectedQuickReplaceModelForUi,
	localProfileQuickReplaceEnabled,
	localProfileQuickReplaceIncludeClipboardContext,
	localQuickReplaceSystemPrompt,
	quickReplaceEnabledInheriting,
	quickReplaceIncludeClipboardContextInheriting,
	quickReplaceProviderInheriting,
	quickReplaceModelInheriting,
	quickReplaceSystemPromptInheriting,
	setQuickReplaceEnabledInheriting,
	setQuickReplaceIncludeClipboardContextInheriting,
	setQuickReplaceProviderInheriting,
	setQuickReplaceModelInheriting,
	setQuickReplaceSystemPromptInheriting,
	setLocalProfileQuickReplaceEnabled,
	setLocalProfileQuickReplaceIncludeClipboardContext,
	setLocalProfileQuickReplaceProvider,
	setLocalProfileQuickReplaceModel,
	setLocalQuickReplaceSystemPrompt,
	saveProfileMetadata,
	openDisableOverrideDialog,
	getLlmModelOptionsForProvider,
	rewriteProvider,
	rewriteModel,
	isSaving,
}: {
	activeProfileId: string;
	activeProfile: RewriteProgramPromptProfile | null;
	isDefaultScope: boolean;
	inheritTooltip: string;
	defaultSystemPrompt: string;
	defaultQuickReplaceEnabled: boolean;
	defaultQuickReplaceIncludeClipboardContext: boolean;
	defaultQuickReplaceProvider: string | null;
	defaultQuickReplaceModel: string | null;
	defaultQuickReplaceSystemPrompt: string;
	effectiveQuickReplaceProvider: string | null;
	llmProviderOptions: LlmOptionGroup[];
	llmProviderDisabled: boolean;
	quickReplaceModelOptions: LlmOption[];
	selectedQuickReplaceModelForUi: string | null;
	localProfileQuickReplaceEnabled: boolean;
	localProfileQuickReplaceIncludeClipboardContext: boolean;
	localQuickReplaceSystemPrompt: string;
	quickReplaceEnabledInheriting: boolean;
	quickReplaceIncludeClipboardContextInheriting: boolean;
	quickReplaceProviderInheriting: boolean;
	quickReplaceModelInheriting: boolean;
	quickReplaceSystemPromptInheriting: boolean;
	setQuickReplaceEnabledInheriting: Dispatch<SetStateAction<boolean>>;
	setQuickReplaceIncludeClipboardContextInheriting: Dispatch<
		SetStateAction<boolean>
	>;
	setQuickReplaceProviderInheriting: Dispatch<SetStateAction<boolean>>;
	setQuickReplaceModelInheriting: Dispatch<SetStateAction<boolean>>;
	setQuickReplaceSystemPromptInheriting: Dispatch<SetStateAction<boolean>>;
	setLocalProfileQuickReplaceEnabled: Dispatch<SetStateAction<boolean>>;
	setLocalProfileQuickReplaceIncludeClipboardContext: Dispatch<
		SetStateAction<boolean>
	>;
	setLocalProfileQuickReplaceProvider: Dispatch<SetStateAction<string | null>>;
	setLocalProfileQuickReplaceModel: Dispatch<SetStateAction<string | null>>;
	setLocalQuickReplaceSystemPrompt: Dispatch<SetStateAction<string>>;
	saveProfileMetadata: (next: Partial<RewriteProgramPromptProfile>) => void;
	openDisableOverrideDialog: (args: {
		title: string;
		onConfirm: () => void;
	}) => void;
	getLlmModelOptionsForProvider: (provider: string | null) => LlmOption[];
	rewriteProvider: string | null;
	rewriteModel: string | null;
	isSaving: boolean;
}) {
	const quickReplaceHasCustom = isDefaultScope
		? (() => {
				const stored = activeProfile?.quick_replace_system_prompt;
				if (stored == null) return false;
				return stored !== defaultSystemPrompt;
			})()
		: activeProfile?.quick_replace_system_prompt !== null &&
			activeProfile?.quick_replace_system_prompt !== undefined;

	const showRewriteProviderReset =
		isDefaultScope &&
		(activeProfile?.quick_replace_provider != null ||
			activeProfile?.quick_replace_model != null);

	return (
		<>
			<div className="settings-mini-header">
				<span className="settings-mini-header__text">Quick Replace</span>
			</div>

			<SettingsRow
				label="Quick Replace"
				description={
					<>
						If you have text highlighted when transcription starts, Kolboo will
						copy the selection, treat your transcript as instructions, rewrite the
						selected text with an LLM, then output using your output mode (Paste
						replaces the selection).
					</>
				}
				right={
					<>
						{!isDefaultScope && quickReplaceEnabledInheriting && (
							<SettingsTooltipIcon label={inheritTooltip}>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</SettingsTooltipIcon>
						)}
						{!isDefaultScope && !quickReplaceEnabledInheriting && (
							<SettingsIconButton
								label="Disable override (inherit from Default)"
								onClick={() =>
									openDisableOverrideDialog({
										title: "Disable Quick Replace override?",
										onConfirm: () => {
											setQuickReplaceEnabledInheriting(true);
											setLocalProfileQuickReplaceEnabled(
												defaultQuickReplaceEnabled,
											);
											saveProfileMetadata({ quick_replace_enabled: null });
										},
									})
								}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</SettingsIconButton>
						)}
						<Switch
							checked={localProfileQuickReplaceEnabled}
							onChange={(e) => {
								const enabled = e.currentTarget.checked;
								if (!isDefaultScope) setQuickReplaceEnabledInheriting(false);
								setLocalProfileQuickReplaceEnabled(enabled);
								saveProfileMetadata({ quick_replace_enabled: enabled });
							}}
							color="gray"
							size="md"
						/>
					</>
				}
			/>


			<SettingsRow
				label="Include Clipboard Context"
				description={
					<>
						When enabled, Kolboo reads your clipboard text and includes it as
						optional context during Quick Replace.
					</>
				}
				right={
					<>
						{!isDefaultScope && quickReplaceIncludeClipboardContextInheriting && (
							<SettingsTooltipIcon label={inheritTooltip}>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</SettingsTooltipIcon>
						)}
						{!isDefaultScope &&
							!quickReplaceIncludeClipboardContextInheriting && (
								<SettingsIconButton
									label="Disable override (inherit from Default)"
									onClick={() =>
										openDisableOverrideDialog({
											title:
											"Disable Quick Replace Clipboard Context override?",
											onConfirm: () => {
												setQuickReplaceIncludeClipboardContextInheriting(true);
												setLocalProfileQuickReplaceIncludeClipboardContext(
													defaultQuickReplaceIncludeClipboardContext,
												);
												saveProfileMetadata({
													quick_replace_include_clipboard_context: null,
												});
										},
									})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</SettingsIconButton>
							)}
						<Switch
							checked={localProfileQuickReplaceIncludeClipboardContext}
							onChange={(e) => {
								const enabled = e.currentTarget.checked;
								if (!isDefaultScope) {
									setQuickReplaceIncludeClipboardContextInheriting(false);
								}
								setLocalProfileQuickReplaceIncludeClipboardContext(enabled);
								saveProfileMetadata({
									quick_replace_include_clipboard_context: enabled,
							});
							}}
							color="gray"
							size="md"
						/>
					</>
				}
			/>

			<SettingsRow
				label="Provider"
				description="AI service used to rewrite the highlighted text."
				right={
					<>
						{!isDefaultScope && quickReplaceProviderInheriting && (
							<SettingsTooltipIcon label={inheritTooltip}>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</SettingsTooltipIcon>
						)}
						{showRewriteProviderReset && (
							<SettingsIconButton
								label="Use Rewrite provider/model"
								onClick={() => {
									setLocalProfileQuickReplaceProvider(rewriteProvider);
									setLocalProfileQuickReplaceModel(rewriteModel);
									saveProfileMetadata({
										quick_replace_provider: null,
										quick_replace_model: null,
									});
								}}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</SettingsIconButton>
						)}
						{!isDefaultScope && !quickReplaceProviderInheriting && (
							<SettingsIconButton
								label="Disable override (inherit from Default)"
								onClick={() =>
									openDisableOverrideDialog({
										title: "Disable Quick Replace Provider override?",
										onConfirm: () => {
											setQuickReplaceProviderInheriting(true);
											setQuickReplaceModelInheriting(true);
											setLocalProfileQuickReplaceProvider(
												defaultQuickReplaceProvider,
											);
											setLocalProfileQuickReplaceModel(
												defaultQuickReplaceModel,
											);
											saveProfileMetadata({
												quick_replace_provider: null,
												quick_replace_model: null,
											});
										},
									})
								}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</SettingsIconButton>
						)}
						<Select
						data={llmProviderOptions}
						value={effectiveQuickReplaceProvider}
						onChange={(value) => {
							if (!value) return;

							if (!isDefaultScope) {
								setQuickReplaceProviderInheriting(false);
								setQuickReplaceModelInheriting(false);
							}

							setLocalProfileQuickReplaceProvider(value);
							const models = getLlmModelOptionsForProvider(value);
							const firstModel = models[0]?.value ?? null;
							setLocalProfileQuickReplaceModel(firstModel);
							saveProfileMetadata({
								quick_replace_provider: value,
								quick_replace_model: firstModel,
							});
						}}
						placeholder="Select provider"
						withCheckIcon={false}
						disabled={llmProviderDisabled}
						styles={{
							input: {
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							},
						}}
					/>
					</>
				}
			/>

			{quickReplaceModelOptions.length > 0 ? (
				<SettingsRow
					label="Model"
					description="LLM model used to rewrite the highlighted text."
					right={
						<>
							{!isDefaultScope && quickReplaceModelInheriting && (
								<SettingsTooltipIcon label={inheritTooltip}>
									<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
								</SettingsTooltipIcon>
							)}
							{!isDefaultScope && !quickReplaceModelInheriting && (
								<SettingsIconButton
									label="Disable override (inherit from Default)"
									onClick={() =>
										openDisableOverrideDialog({
											title: "Disable Quick Replace Model override?",
											onConfirm: () => {
												setQuickReplaceModelInheriting(true);
												setLocalProfileQuickReplaceModel(
													defaultQuickReplaceModel,
												);
												saveProfileMetadata({ quick_replace_model: null });
										},
									})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</SettingsIconButton>
							)}
							<Select
								data={quickReplaceModelOptions}
								value={selectedQuickReplaceModelForUi}
								onChange={(value) => {
									if (!value) return;
									if (!isDefaultScope) setQuickReplaceModelInheriting(false);
									setLocalProfileQuickReplaceModel(value);
									saveProfileMetadata({ quick_replace_model: value });
								}}
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
						</>
					}
				/>
			) : null}

			<div
				className="settings-accordion-block"
				style={{ marginTop: 0, marginBottom: 16 }}
			>
				<Accordion variant="separated" radius="md">
					<PromptSectionEditor
						sectionKey={`${activeProfileId}-quick-replace-system-prompt`}
						title="System Prompt"
						description="Optional instructions that apply to all Quick Replace rewrites."
						enabled={true}
						hideToggle={true}
						placeholder="(leave empty to use the default prompt)"
						initialContent={localQuickReplaceSystemPrompt}
						defaultContent={
							isDefaultScope
								? defaultSystemPrompt
								: defaultQuickReplaceSystemPrompt
						}
						hasCustom={quickReplaceHasCustom}
						inheritMode={
							isDefaultScope
								? null
								: quickReplaceSystemPromptInheriting
									? "inheriting"
									: "overriding"
						}
						inheritTooltip={inheritTooltip}
						disableOverrideTooltip="Disable override (inherit from Default)"
						onDisableOverride={
							isDefaultScope
								? undefined
								: () =>
										openDisableOverrideDialog({
											title: "Disable Quick Replace System Prompt override?",
											onConfirm: () => {
												setQuickReplaceSystemPromptInheriting(true);
												setLocalQuickReplaceSystemPrompt(
													defaultQuickReplaceSystemPrompt,
												);
												saveProfileMetadata({
													quick_replace_system_prompt: null,
												});
											},
										})
						}
						onToggle={() => {}}
						onSave={(content) => {
							if (isDefaultScope) {
								const normalized = content.trim();
								const toStore: string | null =
									normalized.length > 0 && content !== defaultSystemPrompt
										? content
										: null;

								const nextLocal =
									toStore == null ? defaultSystemPrompt : content;

								setLocalQuickReplaceSystemPrompt(nextLocal);
								saveProfileMetadata({ quick_replace_system_prompt: toStore });
								return;
							}

							const base = defaultQuickReplaceSystemPrompt;
							const toStore = content === base ? null : content;
							const nextLocal = toStore == null ? base : content;

							setLocalQuickReplaceSystemPrompt(nextLocal);
							setQuickReplaceSystemPromptInheriting(toStore == null);
							saveProfileMetadata({ quick_replace_system_prompt: toStore });
						}}
						onReset={() => {
							if (isDefaultScope) {
								setLocalQuickReplaceSystemPrompt(defaultSystemPrompt);
								saveProfileMetadata({ quick_replace_system_prompt: null });
								return;
							}

							const base = defaultQuickReplaceSystemPrompt;
							setLocalQuickReplaceSystemPrompt(base);
							setQuickReplaceSystemPromptInheriting(true);
							saveProfileMetadata({ quick_replace_system_prompt: null });
						}}
						isSaving={isSaving}
					/>
				</Accordion>
			</div>
		</>
	);
}
