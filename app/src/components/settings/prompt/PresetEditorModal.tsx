import {
	Accordion,
	ActionIcon,
	Button,
	Group,
	Modal,
	Select,
	Text,
	Textarea,
	TextInput,
	Tooltip,
} from "@mantine/core";
import { Trash2 } from "lucide-react";
import type { CleanupPromptSections, RewritePreset } from "../../../lib/tauri";
import { PromptSectionEditor } from "../PromptSectionEditor";
import type { PresetRuntimeFallbackViews } from "./effectivePromptSettings";
import type { LinkableProfileOption } from "./PromptSettingsModals";
import { presetRoutingHintsFromText } from "./presetSettingsState";
import { TestRewritePanel } from "./TestRewritePanel";

type SectionKey = "system";

type PresetSelectOption = {
	value: string;
	label: string;
};

interface PresetEditorModalProps {
	opened: boolean;
	onClose: () => void;
	editDefaultPresetId: string;
	presetSelectOptions: PresetSelectOption[];
	editingPresetId: string | null;
	onEditingPresetChange: (value: string | null) => void;
	onNewPreset: () => void;
	linkableProfiles: LinkableProfileOption[];
	onOpenLinkPresetModal: () => void;
	isEditingDefaultPreset: boolean;
	selectedPreset: RewritePreset | null;
	selectedPresetRuntimeFallbackViews: PresetRuntimeFallbackViews | null;
	onRequestDeletePreset: (preset: RewritePreset) => void;
	localPresetName: string;
	onLocalPresetNameChange: (value: string) => void;
	localPresetHintsText: string;
	onLocalPresetHintsChange: (value: string) => void;
	onUpdatePreset: (presetId: string, patch: Partial<RewritePreset>) => void;
	getPresetPromptOverride: (
		preset: RewritePreset,
		key: SectionKey,
	) => CleanupPromptSections[SectionKey] | null;
	profilePromptDefaultContent: string;
	activeProfileId: string;
	activeProfileLabel: string;
	onOpenPresetPromptLab: (
		preset: RewritePreset,
		key: SectionKey,
		initialContent: string,
	) => void;
	onSavePresetSectionOverride: (
		preset: RewritePreset,
		key: SectionKey,
		section: CleanupPromptSections[SectionKey] | null,
	) => void;
	isSavingProfiles: boolean;
	rewriteTestInput: string;
	onRewriteTestInputChange: (value: string) => void;
	onRunRewriteTest: (promptOverride?: string) => void;
	isTestingRewrite: boolean;
	rewriteTestDurationMs: number | null;
	rewriteTestError: string;
	rewriteTestOutput: string;
	defaultPresetRewriteStepValue: string;
	onDefaultPresetRewriteStepChange: (value: string) => void;
	localDefaultPresetDescription: string;
	onLocalDefaultPresetDescriptionChange: (value: string) => void;
	currentDefaultPresetDescription: string | null;
	onSaveDefaultPresetDescription: (value: string | null) => void;
	defaultSystemPromptContent: string;
	defaultSystemPromptDefaultContent: string;
	defaultSystemPromptHasCustom: boolean;
	defaultSystemPromptInheritMode: "inheriting" | "overriding" | null;
	onDisableDefaultSystemPromptOverride?: () => void;
	onOpenDefaultPromptLab: () => void;
	onSaveDefaultSystemPrompt: (content: string) => void;
	onResetDefaultSystemPrompt: () => void;
	isDefaultPromptSaving: boolean;
	isDefaultPromptLabDisabled: boolean;
	isSavingCleanupSections: boolean;
	isSavingRewriteEnabled: boolean;
}

export function PresetEditorModal({
	opened,
	onClose,
	editDefaultPresetId,
	presetSelectOptions,
	editingPresetId,
	onEditingPresetChange,
	onNewPreset,
	linkableProfiles,
	onOpenLinkPresetModal,
	isEditingDefaultPreset,
	selectedPreset,
	selectedPresetRuntimeFallbackViews,
	onRequestDeletePreset,
	localPresetName,
	onLocalPresetNameChange,
	localPresetHintsText,
	onLocalPresetHintsChange,
	onUpdatePreset,
	getPresetPromptOverride,
	profilePromptDefaultContent,
	activeProfileId,
	activeProfileLabel,
	onOpenPresetPromptLab,
	onSavePresetSectionOverride,
	isSavingProfiles,
	rewriteTestInput,
	onRewriteTestInputChange,
	onRunRewriteTest,
	isTestingRewrite,
	rewriteTestDurationMs,
	rewriteTestError,
	rewriteTestOutput,
	defaultPresetRewriteStepValue,
	onDefaultPresetRewriteStepChange,
	localDefaultPresetDescription,
	onLocalDefaultPresetDescriptionChange,
	currentDefaultPresetDescription,
	onSaveDefaultPresetDescription,
	defaultSystemPromptContent,
	defaultSystemPromptDefaultContent,
	defaultSystemPromptHasCustom,
	defaultSystemPromptInheritMode,
	onDisableDefaultSystemPromptOverride,
	onOpenDefaultPromptLab,
	onSaveDefaultSystemPrompt,
	onResetDefaultSystemPrompt,
	isDefaultPromptSaving,
	isDefaultPromptLabDisabled,
	isSavingCleanupSections,
	isSavingRewriteEnabled,
}: PresetEditorModalProps) {
	const formatSource = (
		source: PresetRuntimeFallbackViews[keyof PresetRuntimeFallbackViews]["source"],
	) => {
		switch (source) {
			case "preset":
				return "Preset";
			case "profile":
				return "Profile";
			case "global":
				return "Global";
			default:
				return "Default";
		}
	};

	return (
		<Modal
			opened={opened}
			onClose={onClose}
			title="Edit presets"
			centered
			size="xl"
			keepMounted={false}
			zIndex={1000}
			styles={{
				body: {
					height: "70vh",
					overflowY: "auto",
				},
			}}
		>
			<Group
				justify="space-between"
				align="flex-end"
				wrap="wrap"
				gap={12}
				mb="sm"
			>
				<div style={{ flex: 1, minWidth: 260 }}>
					<Text size="xs" c="dimmed" mb={4}>
						Editing preset
					</Text>
					<Select
						data={[
							{
								value: editDefaultPresetId,
								label: "Default",
							},
							...presetSelectOptions,
						]}
						value={editingPresetId ?? editDefaultPresetId}
						onChange={(value) => {
							onEditingPresetChange(value ?? editDefaultPresetId);
						}}
						comboboxProps={{ withinPortal: true, zIndex: 1400 }}
						placeholder="Default"
						withCheckIcon={false}
						styles={{
							input: {
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
							},
						}}
					/>
				</div>

				<Group gap={8} wrap="wrap">
					<Button color="gray" onClick={onNewPreset}>
						New
					</Button>
					<Tooltip
						label={
							linkableProfiles.length === 0
								? "No presets found in other profiles"
								: "Add a shared preset from another profile"
						}
						disabled={linkableProfiles.length > 0}
					>
						<Button
							color="gray"
							variant="light"
							onClick={onOpenLinkPresetModal}
							disabled={linkableProfiles.length === 0}
						>
							Add
						</Button>
					</Tooltip>

					<ActionIcon
						variant="light"
						color="red"
						size={36}
						disabled={isEditingDefaultPreset || !selectedPreset}
						onClick={() => {
							if (isEditingDefaultPreset) return;
							if (!selectedPreset) return;
							onRequestDeletePreset(selectedPreset);
						}}
						aria-label="Delete preset"
					>
						<Trash2 size={16} />
					</ActionIcon>
				</Group>
			</Group>

			<div
				style={{
					display: "flex",
					flexDirection: "column",
					gap: 12,
				}}
			>
				{selectedPreset ? (
					<div
						style={{
							display: "flex",
							flexDirection: "column",
							gap: 12,
						}}
					>
						{selectedPresetRuntimeFallbackViews ? (
							<div
								style={{
									padding: 12,
									borderRadius: 8,
									backgroundColor: "var(--bg-elevated)",
									border: "1px solid var(--border-default)",
								}}
							>
								<Text size="xs" c="dimmed" mb={6}>
									Effective runtime values for this preset
								</Text>
								<Text size="sm">
									Rewrite LLM:{" "}
									{selectedPresetRuntimeFallbackViews.llmProvider.value ??
										"(none)"}
									{selectedPresetRuntimeFallbackViews.llmModel.value
										? ` / ${selectedPresetRuntimeFallbackViews.llmModel.value}`
										: ""}{" "}
									<span style={{ color: "var(--text-muted)" }}>
										·{" "}
										{formatSource(
											selectedPresetRuntimeFallbackViews.llmProvider.source,
										)}
									</span>
								</Text>
								<Text size="sm">
									STT:{" "}
									{selectedPresetRuntimeFallbackViews.sttProvider.value ??
										"(none)"}
									{selectedPresetRuntimeFallbackViews.sttModel.value
										? ` / ${selectedPresetRuntimeFallbackViews.sttModel.value}`
										: ""}
									{selectedPresetRuntimeFallbackViews.sttLanguage.value
										? ` / ${selectedPresetRuntimeFallbackViews.sttLanguage.value}`
										: ""}
									{` / ${selectedPresetRuntimeFallbackViews.sttTimeoutSeconds.value}s`}{" "}
									<span style={{ color: "var(--text-muted)" }}>
										·{" "}
										{formatSource(
											selectedPresetRuntimeFallbackViews.sttProvider.source,
										)}
									</span>
								</Text>
							</div>
						) : null}

						<TextInput
							label="Preset name"
							value={localPresetName}
							onChange={(e) => onLocalPresetNameChange(e.currentTarget.value)}
							onBlur={() => {
								const next = localPresetName.trim();
								if (next && next !== selectedPreset.name) {
									onUpdatePreset(selectedPreset.id, {
										name: next,
									});
								}
							}}
							styles={{
								label: { fontSize: 12 },
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
								},
							}}
						/>

						<div>
							<Text size="xs" c="dimmed" mb={4}>
								Rewrite step
							</Text>
							<Select
								data={[
									{ value: "on", label: "On" },
									{ value: "off", label: "Off" },
								]}
								value={selectedPreset.rewrite_llm_enabled ? "on" : "off"}
								onChange={(value) => {
									if (!value) return;
									onUpdatePreset(selectedPreset.id, {
										rewrite_llm_enabled: value === "on",
									});
								}}
								comboboxProps={{
									withinPortal: true,
									zIndex: 1400,
								}}
								withCheckIcon={false}
								styles={{
									input: {
										backgroundColor: "var(--bg-elevated)",
										borderColor: "var(--border-default)",
										color: "var(--text-primary)",
										minWidth: 220,
									},
								}}
							/>
						</div>

						<Textarea
							label="Routing hints (one per line)"
							description="If empty, the router falls back to the preset name."
							value={localPresetHintsText}
							onChange={(e) => onLocalPresetHintsChange(e.currentTarget.value)}
							onBlur={() => {
								const next = presetRoutingHintsFromText(localPresetHintsText);
								const current = selectedPreset.routing_hints ?? null;
								if (JSON.stringify(current) !== JSON.stringify(next)) {
									onUpdatePreset(selectedPreset.id, {
										routing_hints: next,
									});
								}
							}}
							autosize
							minRows={3}
							styles={{
								label: { fontSize: 12 },
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
									fontFamily: "monospace",
									fontSize: "13px",
								},
							}}
						/>

						<div>
							<Text size="sm" fw={600} mb={6}>
								System Prompt override (relative to this profile)
							</Text>

							<Accordion variant="separated" radius="md">
								{(() => {
									const key: SectionKey = "system";
									const override = getPresetPromptOverride(selectedPreset, key);
									const baseContent = profilePromptDefaultContent;

									const initialContent =
										override && override.content != null
											? override.content
											: baseContent;

									return (
										<PromptSectionEditor
											key={`${activeProfileId}-${selectedPreset.id}-${key}`}
											sectionKey={`${activeProfileId}-preset-${selectedPreset.id}-${key}`}
											title="System Prompt"
											description="Override the profile System Prompt for this preset."
											enabled={true}
											hideToggle={true}
											headerActions={
												<Button
													variant="light"
													color="gray"
													disabled={isSavingProfiles}
													onClick={() => {
														onOpenPresetPromptLab(
															selectedPreset,
															key,
															(initialContent ?? "").trim(),
														);
													}}
												>
													Prompt Lab
												</Button>
											}
											initialContent={initialContent}
											defaultContent={baseContent}
											hasCustom={override != null}
											inheritMode={
												override == null ? "inheriting" : "overriding"
											}
											inheritTooltip="Inheriting from the profile System Prompt"
											disableOverrideTooltip="Disable override (inherit from profile)"
											onDisableOverride={() =>
												onSavePresetSectionOverride(selectedPreset, key, null)
											}
											resetLabel="Reset to Profile"
											onToggle={() => {}}
											onSave={(content) => {
												const contentToStore =
													content === baseContent ? null : content || null;
												const next = {
													content: contentToStore,
												};
												onSavePresetSectionOverride(selectedPreset, key, next);
											}}
											onReset={() =>
												onSavePresetSectionOverride(selectedPreset, key, null)
											}
											isSaving={isSavingProfiles}
										/>
									);
								})()}
							</Accordion>
						</div>

						<div style={{ marginTop: 12 }}>
							<Accordion variant="separated" radius="md">
								<Accordion.Item
									value={`${activeProfileId}-${selectedPreset.id}-test-rewrite`}
								>
									<Accordion.Control>
										<div>
											<p className="settings-label">Test rewrite</p>
											<p className="settings-description">
												Paste a raw transcript and run it through this preset’s
												effective System Prompt.
											</p>
										</div>
									</Accordion.Control>
									<Accordion.Panel>
										{(() => {
											const baseContent = profilePromptDefaultContent;
											const override = getPresetPromptOverride(
												selectedPreset,
												"system",
											);
											const promptForTest =
												override && override.content != null
													? override.content
													: baseContent;

											const isDisabled =
												rewriteTestInput.trim().length === 0 ||
												isSavingProfiles ||
												isSavingCleanupSections ||
												isSavingRewriteEnabled;

											return (
												<TestRewritePanel
													header={`Testing: ${activeProfileLabel} · ${selectedPreset.name?.trim() || selectedPreset.id}`}
													inputValue={rewriteTestInput}
													onInputChange={onRewriteTestInputChange}
													onRun={() => onRunRewriteTest(promptForTest)}
													isRunning={isTestingRewrite}
													durationMs={rewriteTestDurationMs}
													error={rewriteTestError}
													output={rewriteTestOutput}
													isDisabled={isDisabled}
													inputPlaceholder="Raw transcript"
												/>
											);
										})()}
									</Accordion.Panel>
								</Accordion.Item>
							</Accordion>
						</div>
					</div>
				) : isEditingDefaultPreset ? (
					<>
						<div
							style={{
								display: "flex",
								flexWrap: "wrap",
								gap: 12,
								alignItems: "flex-end",
							}}
						>
							<div>
								<Text size="xs" c="dimmed" mb={4}>
									Rewrite step
								</Text>
								<Select
									data={[
										{ value: "on", label: "On" },
										{ value: "off", label: "Off" },
									]}
									value={defaultPresetRewriteStepValue}
									onChange={(value) => {
										if (!value) return;
										onDefaultPresetRewriteStepChange(value);
									}}
									comboboxProps={{
										withinPortal: true,
										zIndex: 1400,
									}}
									withCheckIcon={false}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 240,
										},
									}}
								/>
							</div>
						</div>

						<div style={{ marginTop: 8 }}>
							<Accordion variant="separated" radius="md">
								<PromptSectionEditor
									sectionKey={`${activeProfileId}-default-system-prompt`}
									title="System Prompt"
									description="Instructions used when rewriting the transcript"
									enabled={true}
									hideToggle={true}
									headerActions={
										<Button
											variant="light"
											color="gray"
											disabled={isDefaultPromptLabDisabled}
											onClick={onOpenDefaultPromptLab}
										>
											Prompt Lab
										</Button>
									}
									initialContent={defaultSystemPromptContent}
									defaultContent={defaultSystemPromptDefaultContent}
									hasCustom={defaultSystemPromptHasCustom}
									inheritMode={defaultSystemPromptInheritMode}
									onDisableOverride={
										defaultSystemPromptInheritMode
											? onDisableDefaultSystemPromptOverride
											: undefined
									}
									onToggle={() => {}}
									onSave={onSaveDefaultSystemPrompt}
									onReset={onResetDefaultSystemPrompt}
									isSaving={isDefaultPromptSaving}
								/>
							</Accordion>
						</div>

						<Textarea
							label="Default target routing hints (optional)"
							description="Used by the intent router when deciding to use the profile defaults (no preset). You can put multiple lines here; the router will treat them as additional hints."
							value={localDefaultPresetDescription}
							onChange={(e) =>
								onLocalDefaultPresetDescriptionChange(e.currentTarget.value)
							}
							onBlur={() => {
								const trimmed = localDefaultPresetDescription.trim();
								const next = trimmed.length === 0 ? null : trimmed;
								if (currentDefaultPresetDescription !== next) {
									onSaveDefaultPresetDescription(next);
								}
							}}
							autosize
							minRows={2}
							styles={{
								label: { fontSize: 12 },
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
								},
							}}
						/>

						<div style={{ marginTop: 12 }}>
							<Accordion variant="separated" radius="md">
								<Accordion.Item
									value={`${activeProfileId}-default-test-rewrite`}
								>
									<Accordion.Control>
										<div>
											<p className="settings-label">Test rewrite</p>
											<p className="settings-description">
												Paste a raw transcript and run it through the Default
												preset.
											</p>
										</div>
									</Accordion.Control>
									<Accordion.Panel>
										{(() => {
											const promptForTest = defaultSystemPromptContent;
											const isDisabled =
												rewriteTestInput.trim().length === 0 ||
												isSavingProfiles ||
												isSavingCleanupSections ||
												isSavingRewriteEnabled;

											return (
												<TestRewritePanel
													header={`Testing: ${activeProfileLabel} · Default`}
													inputValue={rewriteTestInput}
													onInputChange={onRewriteTestInputChange}
													onRun={() => onRunRewriteTest(promptForTest)}
													isRunning={isTestingRewrite}
													durationMs={rewriteTestDurationMs}
													error={rewriteTestError}
													output={rewriteTestOutput}
													isDisabled={isDisabled}
													inputPlaceholder="Raw transcript"
												/>
											);
										})()}
									</Accordion.Panel>
								</Accordion.Item>
							</Accordion>
						</div>
					</>
				) : null}
			</div>
		</Modal>
	);
}
