import { ActionModelPicker } from "../ActionModelPicker";
import { Accordion, Select, Switch, Tooltip } from "@mantine/core";
import type { Dispatch, SetStateAction } from "react";
import type {
	ActiveWindowOcrMode,
	RewriteProgramPromptProfile,
} from "../../lib/tauri";
import { PromptSectionEditor } from "./PromptSectionEditor";
import { SettingsInheritanceIndicator } from "./SettingsInheritance";
import { SettingsRow } from "./SettingsRow";


export function QuickReplaceSettings({
	activeProfileId,
	activeProfile,
	isDefaultScope,
	inheritTooltip,
	defaultSystemPrompt,
	ocrProviderAvailable,
	ocrProviderUnavailableReason,
	defaultQuickReplaceEnabled,
	defaultQuickReplaceIncludeClipboardContext,
	localProfileQuickReplaceActiveWindowOcrMode,
	quickReplaceActiveWindowOcrModeInheriting,
	setQuickReplaceActiveWindowOcrModeInheriting,
	setLocalProfileQuickReplaceActiveWindowOcrMode,
	defaultQuickReplaceSystemPrompt,
	localProfileQuickReplaceEnabled,
	localProfileQuickReplaceIncludeClipboardContext,
	localQuickReplaceSystemPrompt,
	quickReplaceEnabledInheriting,
	quickReplaceIncludeClipboardContextInheriting,
	quickReplaceSystemPromptInheriting,
	setQuickReplaceEnabledInheriting,
	setQuickReplaceIncludeClipboardContextInheriting,
	setQuickReplaceSystemPromptInheriting,
	setLocalProfileQuickReplaceEnabled,
	setLocalProfileQuickReplaceIncludeClipboardContext,
	setLocalQuickReplaceSystemPrompt,
	saveProfileMetadata,
	openDisableOverrideDialog,
	isSaving,
}: {
	activeProfileId: string;
	activeProfile: RewriteProgramPromptProfile | null;
	isDefaultScope: boolean;
	inheritTooltip: string;
	defaultSystemPrompt: string;
	ocrProviderAvailable: boolean;
	ocrProviderUnavailableReason: string | null;
	defaultQuickReplaceEnabled: boolean;
	defaultQuickReplaceIncludeClipboardContext: boolean;
	localProfileQuickReplaceActiveWindowOcrMode: ActiveWindowOcrMode;
	quickReplaceActiveWindowOcrModeInheriting: boolean;
	setQuickReplaceActiveWindowOcrModeInheriting: Dispatch<
		SetStateAction<boolean>
	>;
	setLocalProfileQuickReplaceActiveWindowOcrMode: Dispatch<
		SetStateAction<ActiveWindowOcrMode>
	>;
	defaultQuickReplaceSystemPrompt: string;
	localProfileQuickReplaceEnabled: boolean;
	localProfileQuickReplaceIncludeClipboardContext: boolean;
	localQuickReplaceSystemPrompt: string;
	quickReplaceEnabledInheriting: boolean;
	quickReplaceIncludeClipboardContextInheriting: boolean;
	quickReplaceSystemPromptInheriting: boolean;
	setQuickReplaceEnabledInheriting: Dispatch<SetStateAction<boolean>>;
	setQuickReplaceIncludeClipboardContextInheriting: Dispatch<
		SetStateAction<boolean>
	>;
	setQuickReplaceSystemPromptInheriting: Dispatch<SetStateAction<boolean>>;
	setLocalProfileQuickReplaceEnabled: Dispatch<SetStateAction<boolean>>;
	setLocalProfileQuickReplaceIncludeClipboardContext: Dispatch<
		SetStateAction<boolean>
	>;
	setLocalQuickReplaceSystemPrompt: Dispatch<SetStateAction<string>>;
	saveProfileMetadata: (next: Partial<RewriteProgramPromptProfile>) => void;
	openDisableOverrideDialog: (args: {
		title: string;
		onConfirm: () => void;
	}) => void;
	isSaving: boolean;
}) {
	const ocrModeDisabled = !ocrProviderAvailable;
	const ocrModeDisabledTooltip =
		ocrProviderUnavailableReason ??
		"OCR is disabled until an OCR Base URL is set in Settings → Providers.";

	const quickReplaceHasCustom = isDefaultScope
		? (() => {
				const stored = activeProfile?.quick_replace_system_prompt;
				if (stored == null) return false;
				return stored !== defaultSystemPrompt;
			})()
		: activeProfile?.quick_replace_system_prompt !== null &&
			activeProfile?.quick_replace_system_prompt !== undefined;


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
						copy the selection, treat your transcript as instructions, rewrite
						the selected text with an LLM, then output using your output mode
						(Paste replaces the selection).
					</>
				}
				right={
					<>
						<SettingsInheritanceIndicator
							isDefaultScope={isDefaultScope}
							inheriting={quickReplaceEnabledInheriting}
							inheritTooltip={inheritTooltip}
							onDisableOverride={() =>
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
						/>
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
						<SettingsInheritanceIndicator
							isDefaultScope={isDefaultScope}
							inheriting={quickReplaceIncludeClipboardContextInheriting}
							inheritTooltip={inheritTooltip}
							onDisableOverride={() =>
								openDisableOverrideDialog({
									title: "Disable Quick Replace Clipboard Context override?",
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
						/>
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
				label="Active Window OCR"
				description={
					<>
						Optionally capture the currently active window, run OCR, and include
						the text as extra context in Quick Replace prompts.
					</>
				}
				className={ocrModeDisabled ? "settings-row--disabled" : undefined}
				right={
					<>
						<SettingsInheritanceIndicator
							isDefaultScope={isDefaultScope}
							inheriting={quickReplaceActiveWindowOcrModeInheriting}
							inheritTooltip={inheritTooltip}
							onDisableOverride={() =>
								openDisableOverrideDialog({
									title: "Disable Quick Replace Active Window OCR override?",
									onConfirm: () => {
										setQuickReplaceActiveWindowOcrModeInheriting(true);
										saveProfileMetadata({
											quick_replace_active_window_ocr_mode: null,
										});
									},
								})
							}
							disabled={isSaving || ocrModeDisabled}
						/>
						<Tooltip
							label={ocrModeDisabledTooltip}
							withArrow
							disabled={!ocrModeDisabled}
						>
							<div>
								<Select
									data={[
										{ value: "off", label: "Off" },
									{ value: "auto", label: "Always" },
										{ value: "manual", label: "Manual" },
									]}
									value={localProfileQuickReplaceActiveWindowOcrMode}
									onChange={(value) => {
										if (!value) return;
										if (
											value === "off" ||
											value === "auto" ||
											value === "manual"
										) {
											if (!isDefaultScope) {
												setQuickReplaceActiveWindowOcrModeInheriting(false);
											}
											setLocalProfileQuickReplaceActiveWindowOcrMode(value);
											saveProfileMetadata({
												quick_replace_active_window_ocr_mode: value,
											});
										}
									}}
									withCheckIcon={false}
									disabled={isSaving || ocrModeDisabled}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 140,
										},
									}}
								/>
							</div>
						</Tooltip>
					</>
				}
			/>

			<div className="settings-row"><ActionModelPicker action="quick_replace" label="Quick Replace model" profileId={activeProfileId} /></div>

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
