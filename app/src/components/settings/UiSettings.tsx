import {
	ActionIcon,
	Button,
	Checkbox,
	Group,
	Modal,
	SegmentedControl,
	Select,
	Switch,
	Tooltip,
} from "@mantine/core";
import { invoke } from "@tauri-apps/api/core";
import { Info, Play, RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";
import { applyAccentColor, DEFAULT_ACCENT_HEX } from "../../lib/accentColor";
import {
	useIsAudioMuteSupported,
	useSettings,
	useUpdateAccentColor,
	useUpdateAudioCue,
	useUpdateMainWindowCloseBehavior,
	useUpdateOutputHitEnter,
	useUpdateOutputMode,
	useUpdateOutputSmartPasteProtection,
	useUpdateOverlayMode,
	useUpdateOverlayMonitorTarget,
	useUpdateOverlayShowDetailedLoading,
	useUpdatePlayingAudioHandling,
	useUpdateRewriteProgramPromptProfiles,
	useUpdateSoundEnabled,
} from "../../lib/queries";
import type {
	AudioCue,
	ContextGrabMethod,
	MainWindowCloseBehavior,
	OutputMode,
	OverlayMode,
	OverlayMonitorTarget,
	PlayingAudioHandling,
	RewriteProgramPromptProfile,
} from "../../lib/tauri";
import { DEFAULT_SETTINGS_VALUES } from "../../lib/tauri/settingsDefaults";
import {
	findProfileById,
	inheritedSettingView,
	isInheritedSettingValue,
} from "../../lib/tauri/settingsViews";
import {
	SettingsIconButton,
	SettingsRow,
	SettingsTooltipIcon,
} from "./SettingsRow";

const INHERIT_TOOLTIP = "Inheriting from Default profile";

const GLOBAL_ONLY_TOOLTIP =
	"This setting can only be changed in the Default profile";

const OVERLAY_MODE_OPTIONS = [
	{ value: "always", label: "Always visible" },
	{ value: "recording_only", label: "Only when recording" },
	{ value: "never", label: "Hidden" },
];

const OVERLAY_MONITOR_TARGET_OPTIONS: Array<{
	value: OverlayMonitorTarget;
	label: string;
}> = [
	{ value: "main", label: "Main monitor" },
	{ value: "cursor", label: "Monitor with cursor" },
	{ value: "active_window", label: "Monitor with active window" },
];

const PLAYING_AUDIO_HANDLING_OPTIONS: Array<{
	value: PlayingAudioHandling;
	label: string;
}> = [
	// Keep a "None" option so existing users who had auto-mute disabled
	// don't suddenly start muting/pausing after this UI change.
	{ value: "none", label: "None" },
	{ value: "mute", label: "Mute" },
	{ value: "pause", label: "Pause" },
	{ value: "mute_and_pause", label: "Mute and Pause" },
];

const AUDIO_CUE_OPTIONS: Array<{ value: AudioCue; label: string }> = [
	{ value: "kolboo", label: "Kolboo" },
	{ value: "maraca", label: "Maraca" },
	{ value: "clave", label: "Claves" },
	// Required: current cue should be last in the dropdown.
	{ value: "legacy", label: "Tambourine" },
];

const CONTEXT_GRAB_METHOD_OPTIONS: Array<{
	value: ContextGrabMethod;
	label: string;
}> = [
	{ value: "ctrl_c", label: "Ctrl+C" },
	{ value: "ctrl_shift_c", label: "Ctrl+Shift+C" },
	{ value: "ctrl_insert", label: "Ctrl+Insert" },
	{ value: "none", label: "None" },
];

const normalizeBoolean = (value: unknown): boolean | null =>
	typeof value === "boolean" ? value : null;

const normalizePlayingAudioHandling = (
	value: unknown,
): PlayingAudioHandling | null =>
	value === "none" ||
	value === "mute" ||
	value === "pause" ||
	value === "mute_and_pause"
		? value
		: null;

const normalizeOverlayMode = (value: unknown): OverlayMode | null =>
	value === "always" || value === "recording_only" || value === "never"
		? value
		: null;

const normalizeOutputMode = (value: unknown): OutputMode | null =>
	value === "paste" || value === "clipboard" || value === "paste_and_clipboard"
		? value
		: null;

const normalizeContextGrabMethod = (
	value: unknown,
): ContextGrabMethod | null =>
	value === "ctrl_c" ||
	value === "ctrl_shift_c" ||
	value === "ctrl_insert" ||
	value === "none" ||
	value === "clipboard_only"
		? value
		: null;

export function UiSettings({
	editingProfileId,
}: {
	editingProfileId?: string;
}) {
	const { data: settings, isLoading } = useSettings();
	const isWindows =
		typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent);
	const { data: isAudioMuteSupported } = useIsAudioMuteSupported();
	const updateSoundEnabled = useUpdateSoundEnabled();
	const updateAccentColor = useUpdateAccentColor();
	const updateAudioCue = useUpdateAudioCue();
	const updateMainWindowCloseBehavior = useUpdateMainWindowCloseBehavior();
	const updatePlayingAudioHandling = useUpdatePlayingAudioHandling();
	const updateOverlayMode = useUpdateOverlayMode();
	const updateOverlayShowDetailedLoading =
		useUpdateOverlayShowDetailedLoading();
	const updateOverlayMonitorTarget = useUpdateOverlayMonitorTarget();
	const updateOutputMode = useUpdateOutputMode();
	const updateOutputHitEnter = useUpdateOutputHitEnter();
	const updateOutputSmartPasteProtection =
		useUpdateOutputSmartPasteProtection();
	const updateRewriteProgramPromptProfiles =
		useUpdateRewriteProgramPromptProfiles();

	const profiles = settings?.rewrite_program_prompt_profiles ?? [];
	const defaultProfile = findProfileById(profiles, "default");
	const profile: RewriteProgramPromptProfile | null =
		editingProfileId && editingProfileId !== "default"
			? findProfileById(profiles, editingProfileId)
			: null;

	const isProfileScope = profile !== null;

	const [resetDialog, setResetDialog] = useState<null | {
		title: string;
		onConfirm: () => void;
	}>(null);

	const openDisableOverrideDialog = (args: {
		title: string;
		onConfirm: () => void;
	}) => {
		setResetDialog(args);
	};

	const updateProfile = (partial: Partial<RewriteProgramPromptProfile>) => {
		if (!profile) return;
		const next = profiles.map((p) =>
			p.id === profile.id ? { ...p, ...partial } : p,
		);
		updateRewriteProgramPromptProfiles.mutate(next);
	};

	const updateDefaultProfile = (
		partial: Partial<RewriteProgramPromptProfile>,
	) => {
		const existing = profiles.find((p) => p.id === "default") ?? null;
		const base: RewriteProgramPromptProfile =
			existing ??
			({ id: "default", name: "Default" } as RewriteProgramPromptProfile);

		const next = existing
			? profiles.map((p) => (p.id === "default" ? { ...p, ...partial } : p))
			: [{ ...base, ...partial }, ...profiles];

		updateRewriteProgramPromptProfiles.mutate(next);
	};

	// Profile-scoped UI reads go through Settings View helpers so inheritance,
	// explicit-null, and malformed values are resolved in one place instead of as
	// repeated `profileValue ?? globalValue` snippets across settings screens.
	const globalSoundEnabled =
		settings?.sound_enabled ?? DEFAULT_SETTINGS_VALUES.sound_enabled;
	const soundView = inheritedSettingView({
		globalValue: globalSoundEnabled,
		profile,
		key: "sound_enabled",
		defaultValue: DEFAULT_SETTINGS_VALUES.sound_enabled,
		normalize: normalizeBoolean,
	});
	const soundEnabled = soundView.value;
	const soundInheriting =
		isProfileScope && isInheritedSettingValue(profile, "sound_enabled");

	const audioCueFromSettings: AudioCue = settings?.audio_cue ?? "kolboo";
	const [audioCueDropdownValue, setAudioCueDropdownValue] =
		useState<AudioCue>(audioCueFromSettings);

	useEffect(() => {
		setAudioCueDropdownValue(audioCueFromSettings);
	}, [audioCueFromSettings]);

	const globalPlayingAudioHandling: PlayingAudioHandling =
		settings?.playing_audio_handling ??
		DEFAULT_SETTINGS_VALUES.playing_audio_handling;
	const playingAudioHandlingView = inheritedSettingView({
		globalValue: globalPlayingAudioHandling,
		profile,
		key: "playing_audio_handling",
		defaultValue: DEFAULT_SETTINGS_VALUES.playing_audio_handling,
		normalize: normalizePlayingAudioHandling,
	});
	const playingAudioHandling = playingAudioHandlingView.value;
	const playingAudioHandlingInheriting =
		isProfileScope &&
		isInheritedSettingValue(profile, "playing_audio_handling");

	const cueDisabledByMuteHandling =
		playingAudioHandling === "mute" ||
		playingAudioHandling === "mute_and_pause";

	const cueDisabledReason = cueDisabledByMuteHandling
		? "Sound cues are disabled while Playing audio handling includes mute. Choose 'none' or 'pause' to enable cues."
		: null;

	const globalOverlayMode: OverlayMode =
		settings?.overlay_mode ?? DEFAULT_SETTINGS_VALUES.overlay_mode;
	const overlayModeView = inheritedSettingView({
		globalValue: globalOverlayMode,
		profile,
		key: "overlay_mode",
		defaultValue: DEFAULT_SETTINGS_VALUES.overlay_mode,
		normalize: normalizeOverlayMode,
	});
	const overlayMode = overlayModeView.value;
	const overlayModeInheriting =
		isProfileScope && isInheritedSettingValue(profile, "overlay_mode");

	const overlayShowDetailedLoading =
		settings?.overlay_show_detailed_loading ?? false;

	const overlayMonitorTarget: OverlayMonitorTarget =
		settings?.overlay_monitor_target ?? "main";

	const globalOutputMode: OutputMode =
		settings?.output_mode ?? DEFAULT_SETTINGS_VALUES.output_mode;
	const outputModeView = inheritedSettingView({
		globalValue: globalOutputMode,
		profile,
		key: "output_mode",
		defaultValue: DEFAULT_SETTINGS_VALUES.output_mode,
		normalize: normalizeOutputMode,
	});
	const outputMode = outputModeView.value;
	const outputModeInheriting =
		isProfileScope && isInheritedSettingValue(profile, "output_mode");

	const globalOutputHitEnter =
		settings?.output_hit_enter ?? DEFAULT_SETTINGS_VALUES.output_hit_enter;
	const outputHitEnterView = inheritedSettingView({
		globalValue: globalOutputHitEnter,
		profile,
		key: "output_hit_enter",
		defaultValue: DEFAULT_SETTINGS_VALUES.output_hit_enter,
		normalize: normalizeBoolean,
	});
	const outputHitEnter = outputHitEnterView.value;
	const outputHitEnterInheriting =
		isProfileScope && isInheritedSettingValue(profile, "output_hit_enter");

	const outputSmartPasteProtection =
		settings?.output_smart_paste_protection ?? false;

	const globalContextGrabMethod: ContextGrabMethod =
		defaultProfile?.context_grab_method ?? "ctrl_c";
	const contextGrabMethodView = inheritedSettingView({
		globalValue: globalContextGrabMethod,
		profile,
		key: "context_grab_method",
		defaultValue: "ctrl_c" as const,
		normalize: normalizeContextGrabMethod,
	});
	const contextGrabMethod = contextGrabMethodView.value;
	const contextGrabMethodInheriting =
		isProfileScope && isInheritedSettingValue(profile, "context_grab_method");

	// Backward compatible: older settings may contain the deprecated value
	// "clipboard_only". We no longer show it as an option; display it as Ctrl+C
	// so the dropdown doesn't appear blank.
	const contextGrabMethodUiValue: ContextGrabMethod =
		contextGrabMethod === "clipboard_only" ? "ctrl_c" : contextGrabMethod;

	const mainWindowCloseBehavior: MainWindowCloseBehavior =
		settings?.main_window_close_behavior ?? "minimize_to_tray";

	// Accent color (global only)
	const ACCENT_COLOR_OPTIONS: Array<{ value: string; label: string }> = [
		{ value: "#f97316", label: "Tangerine" },
		{ value: "#ef4444", label: "Red" },
		{ value: "#ec4899", label: "Pink" },
		{ value: "#a855f7", label: "Purple" },
		{ value: "#3b82f6", label: "Blue" },
		{ value: "#06b6d4", label: "Cyan" },
		{ value: "#22c55e", label: "Green" },
		{ value: "#eab308", label: "Yellow" },
		{ value: "#9ca3af", label: "Grey" },
	];

	const accentColorValue = settings?.accent_color ?? DEFAULT_ACCENT_HEX;
	const accentDropdownValueFromSettings = accentColorValue;
	const [accentDropdownValue, setAccentDropdownValue] = useState<string>(
		accentDropdownValueFromSettings,
	);

	useEffect(() => {
		setAccentDropdownValue(accentDropdownValueFromSettings);
	}, [accentDropdownValueFromSettings]);

	const getAccentSwatch = (value: string): string => {
		return value;
	};

	const handleAccentColorChange = (value: string | null) => {
		if (!value) return;
		if (isProfileScope) return;

		// Update UI immediately (even before the settings query refetch completes)
		setAccentDropdownValue(value);

		applyAccentColor(value);
		updateAccentColor.mutate(value);
	};

	// Handlers - update profile or global depending on scope
	const handleSoundToggle = (checked: boolean) => {
		if (isProfileScope) {
			updateProfile({ sound_enabled: checked });
			return;
		}
		updateSoundEnabled.mutate(checked);
	};

	const handleAudioCueChange = (value: string | null) => {
		if (!value) return;
		if (isProfileScope) return;

		const next = value as AudioCue;
		setAudioCueDropdownValue(next);
		updateAudioCue.mutate(next);
	};

	const handlePreviewAudioCue = async () => {
		if (cueDisabledByMuteHandling) return;
		try {
			await invoke("play_audio_cue_preview", { cue: audioCueDropdownValue });
		} catch (err) {
			console.error("Failed to play audio cue preview", err);
		}
	};

	const handlePlayingAudioHandlingChange = (value: string | null) => {
		if (!value) return;
		const next = value as PlayingAudioHandling;
		if (isProfileScope) {
			updateProfile({ playing_audio_handling: next });
			return;
		}
		updatePlayingAudioHandling.mutate(next);
	};

	const handleOverlayModeChange = (value: string | null) => {
		if (!value) return;
		if (isProfileScope) {
			updateProfile({ overlay_mode: value as OverlayMode });
			return;
		}
		updateOverlayMode.mutate(value as OverlayMode);
	};

	const handleOverlayShowDetailedLoadingChange = (checked: boolean) => {
		// Global-only setting
		if (isProfileScope) return;
		updateOverlayShowDetailedLoading.mutate(checked);
	};

	const handleOverlayMonitorTargetChange = (value: string | null) => {
		// Global-only setting
		if (!value) return;
		if (isProfileScope) return;
		updateOverlayMonitorTarget.mutate(value as OverlayMonitorTarget);
	};

	const handleOutputHitEnterToggle = (checked: boolean) => {
		if (isProfileScope) {
			updateProfile({ output_hit_enter: checked });
			return;
		}
		updateOutputHitEnter.mutate(checked);
	};

	const handleOutputModeChange = (next: string) => {
		const nextMode = next as OutputMode;

		// If switching to clipboard-only, hit-enter becomes invalid; clear it.
		if (nextMode === "clipboard" && outputHitEnter) {
			handleOutputHitEnterToggle(false);
		}

		// Avoid no-op writes
		if (nextMode === outputMode) return;

		if (isProfileScope) {
			updateProfile({ output_mode: nextMode });
			return;
		}
		updateOutputMode.mutate(nextMode);
	};

	const handleOutputSmartPasteProtectionToggle = (checked: boolean) => {
		if (isProfileScope) return;
		updateOutputSmartPasteProtection.mutate(checked);
	};

	const handleContextGrabMethodChange = (value: string | null) => {
		if (!value) return;
		const next = value as ContextGrabMethod;

		if (next === contextGrabMethod) return;

		if (isProfileScope) {
			updateProfile({ context_grab_method: next });
			return;
		}

		updateDefaultProfile({ context_grab_method: next });
	};

	const handleMainWindowCloseBehaviorChange = (next: string) => {
		if (isProfileScope) return;

		const behavior = next as MainWindowCloseBehavior;
		if (behavior !== "exit_program" && behavior !== "minimize_to_tray") return;
		if (behavior === mainWindowCloseBehavior) return;

		updateMainWindowCloseBehavior.mutate(behavior);
	};

	return (
		<>
			<Modal
				opened={resetDialog !== null}
				onClose={() => setResetDialog(null)}
				title={resetDialog?.title ?? ""}
				centered
			>
				<div style={{ fontSize: 13, opacity: 0.85, lineHeight: 1.4 }}>
					This setting is currently overriding the Default profile. Disable the
					override to inherit from Default.
				</div>
				<Group justify="flex-end" mt="md" gap="sm">
					<Button variant="default" onClick={() => setResetDialog(null)}>
						Keep override
					</Button>
					<Button
						color="gray"
						onClick={() => {
							const confirm = resetDialog?.onConfirm;
							setResetDialog(null);
							confirm?.();
						}}
					>
						Disable override
					</Button>
				</Group>
			</Modal>

			<SettingsRow
				label="Sound feedback"
				description="Play sounds when recording starts and stops"
				right={
					<>
						{isProfileScope && !soundInheriting && (
							<SettingsIconButton
								label="Disable override (inherit from Default)"
								disabled={isLoading}
								onClick={() =>
									openDisableOverrideDialog({
										title: "Disable Sound feedback override?",
										onConfirm: () => updateProfile({ sound_enabled: null }),
									})
								}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</SettingsIconButton>
						)}
						{soundInheriting && (
							<SettingsTooltipIcon label={INHERIT_TOOLTIP}>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</SettingsTooltipIcon>
						)}
						<Switch
							checked={soundEnabled}
							onChange={(event) =>
								handleSoundToggle(event.currentTarget.checked)
							}
							disabled={isLoading}
							color="gray"
							size="md"
						/>
					</>
				}
			/>

			<SettingsRow
				label="Sound cue"
				description="Choose which sound plays when recording starts and stops"
				right={
					<>
						<Tooltip
							label={cueDisabledReason ?? "Preview (start + stop)"}
							withArrow
						>
							{/* Wrap so tooltip still shows even when the button is disabled */}
							<span style={{ display: "inline-flex" }}>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									disabled={isLoading || cueDisabledByMuteHandling}
									onMouseDown={(e) => {
										// Prevent focusing/opening the select when clicking the button.
										e.preventDefault();
										e.stopPropagation();
									}}
									onClick={handlePreviewAudioCue}
								>
									<Play size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</span>
						</Tooltip>

						<Tooltip
							label={
								isProfileScope ? GLOBAL_ONLY_TOOLTIP : (cueDisabledReason ?? "")
							}
							disabled={!(isProfileScope || cueDisabledByMuteHandling)}
							withArrow
						>
							{/* Wrap so tooltip still shows even when the select is disabled */}
							<div style={{ display: "inline-block" }}>
								<Select
									data={AUDIO_CUE_OPTIONS}
									value={audioCueDropdownValue}
									onChange={handleAudioCueChange}
									disabled={
										isLoading || isProfileScope || cueDisabledByMuteHandling
									}
									withCheckIcon={false}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											minWidth: 180,
										},
									}}
								/>
							</div>
						</Tooltip>
					</>
				}
			/>

			<SettingsRow
				label="Playing audio handling"
				description="Mute and/or pause playing audio while recording"
				right={
					<>
						{isProfileScope && !playingAudioHandlingInheriting && (
							<SettingsIconButton
								label="Disable override (inherit from Default)"
								disabled={isLoading}
								onClick={() =>
									openDisableOverrideDialog({
										title: "Disable Playing audio handling override?",
										onConfirm: () =>
											updateProfile({ playing_audio_handling: null }),
									})
								}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</SettingsIconButton>
						)}
						{playingAudioHandlingInheriting && (
							<SettingsTooltipIcon label={INHERIT_TOOLTIP}>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</SettingsTooltipIcon>
						)}
						<Tooltip
							label="Mute not supported on this platform"
							disabled={isAudioMuteSupported !== false}
							withArrow
						>
							<Select
								data={PLAYING_AUDIO_HANDLING_OPTIONS.map((o) => ({
									...o,
									disabled:
										isAudioMuteSupported === false &&
										(o.value === "mute" || o.value === "mute_and_pause"),
								}))}
								value={playingAudioHandling}
								onChange={handlePlayingAudioHandlingChange}
								disabled={isLoading}
								withCheckIcon={false}
								styles={{
									input: {
										backgroundColor: "var(--bg-elevated)",
										borderColor: "var(--border-default)",
										color: "var(--text-primary)",
										minWidth: 180,
									},
								}}
							/>
						</Tooltip>
					</>
				}
			/>

			<SettingsRow
				label="Overlay widget"
				description="When to show the on-screen recording widget"
				right={
					<>
						{isProfileScope && !overlayModeInheriting && (
							<SettingsIconButton
								label="Disable override (inherit from Default)"
								disabled={isLoading}
								onClick={() =>
									openDisableOverrideDialog({
										title: "Disable Overlay widget override?",
										onConfirm: () => updateProfile({ overlay_mode: null }),
									})
								}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</SettingsIconButton>
						)}
						{overlayModeInheriting && (
							<SettingsTooltipIcon label={INHERIT_TOOLTIP}>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</SettingsTooltipIcon>
						)}
						<Select
							data={OVERLAY_MODE_OPTIONS}
							value={overlayMode}
							onChange={handleOverlayModeChange}
							disabled={isLoading}
							withCheckIcon={false}
							styles={{
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
									minWidth: 180,
								},
							}}
						/>
					</>
				}
			/>

			<SettingsRow
				label="Overlay detailed loading"
				description={
					<>
						Show text like “transcribing…”, “routing…”, “rewriting…” instead of
						a loading waveform
					</>
				}
				right={
					<Tooltip
						label={GLOBAL_ONLY_TOOLTIP}
						disabled={!isProfileScope}
						withArrow
						position="top-start"
					>
						<div style={isProfileScope ? { opacity: 0.5 } : undefined}>
							<Switch
								checked={overlayShowDetailedLoading}
								onChange={(event) =>
									handleOverlayShowDetailedLoadingChange(
										event.currentTarget.checked,
									)
								}
								disabled={isLoading || isProfileScope}
								color="gray"
								size="md"
							/>
						</div>
					</Tooltip>
				}
			/>

			{/* Widget position setting intentionally hidden for now (we may bring it back later). */}

			<SettingsRow
				label="Overlay monitor"
				description="Which display the overlay windows should appear on"
				right={
					<Tooltip
						label={GLOBAL_ONLY_TOOLTIP}
						disabled={!isProfileScope}
						withArrow
						position="top-start"
					>
						<div style={isProfileScope ? { opacity: 0.5 } : undefined}>
							<Select
								data={OVERLAY_MONITOR_TARGET_OPTIONS}
								value={overlayMonitorTarget}
								onChange={handleOverlayMonitorTargetChange}
								disabled={isLoading || isProfileScope}
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
					</Tooltip>
				}
			/>

			<SettingsRow
				label="Output"
				description="How to output transcribed text"
				right={
					<>
						{isProfileScope &&
							!(outputModeInheriting && outputHitEnterInheriting) && (
								<SettingsIconButton
									label="Disable override (inherit from Default)"
									disabled={isLoading}
									onClick={() =>
										openDisableOverrideDialog({
											title: "Disable Output override?",
											onConfirm: () =>
												updateProfile({
													output_mode: null,
													output_hit_enter: null,
												}),
										})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</SettingsIconButton>
							)}
						{isProfileScope &&
							outputModeInheriting &&
							outputHitEnterInheriting && (
								<SettingsTooltipIcon label={INHERIT_TOOLTIP}>
									<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
								</SettingsTooltipIcon>
							)}
						<div
							style={{
								display: "flex",
								alignItems: "center",
								justifyContent: "flex-end",
								gap: 12,
							}}
						>
							<SegmentedControl
								value={outputMode}
								onChange={handleOutputModeChange}
								disabled={isLoading}
								data={[
									{ value: "paste", label: "Paste" },
									{ value: "clipboard", label: "Copy" },
									{ value: "paste_and_clipboard", label: "Both" },
								]}
								size="sm"
								radius="md"
								styles={{
									root: {
										backgroundColor: "var(--bg-elevated)",
										border: "1px solid var(--border-default)",
										minWidth: 260,
									},
								}}
							/>
							<Checkbox
								label="Press Enter after paste"
								checked={outputHitEnter}
								onChange={(event) =>
									handleOutputHitEnterToggle(event.currentTarget.checked)
								}
								disabled={isLoading || outputMode === "clipboard"}
								color="gray"
								size="sm"
							/>
						</div>
					</>
				}
			/>

			<SettingsRow
				label="Smart paste protection"
				description="Avoid pasting into sensitive fields (like password boxes)"
				right={
					<Tooltip
						label={GLOBAL_ONLY_TOOLTIP}
						disabled={!isProfileScope}
						withArrow
						position="top-start"
					>
						<div style={isProfileScope ? { opacity: 0.5 } : undefined}>
							<Switch
								checked={outputSmartPasteProtection}
								onChange={(event) =>
									handleOutputSmartPasteProtectionToggle(
										event.currentTarget.checked,
									)
								}
								disabled={isLoading || isProfileScope}
								color="gray"
								size="md"
							/>
						</div>
					</Tooltip>
				}
			/>

			{!isWindows && (
				<SettingsRow
					label="Context Grab Shortcut"
					description={
						<>
							Shortcut Kolboo uses to copy your highlighted selection when a
							feature needs selected-text context
						</>
					}
					right={
						<>
							{isProfileScope && !contextGrabMethodInheriting && (
								<SettingsIconButton
									label="Disable override (inherit from Default)"
									disabled={isLoading}
									onClick={() =>
										openDisableOverrideDialog({
											title: "Disable Context Grab Shortcut override?",
											onConfirm: () =>
												updateProfile({ context_grab_method: null }),
										})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</SettingsIconButton>
							)}
							{isProfileScope && contextGrabMethodInheriting && (
								<SettingsTooltipIcon label={INHERIT_TOOLTIP}>
									<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
								</SettingsTooltipIcon>
							)}
							<Select
								data={CONTEXT_GRAB_METHOD_OPTIONS}
								value={contextGrabMethodUiValue}
								onChange={handleContextGrabMethodChange}
								disabled={isLoading}
								withCheckIcon={false}
								allowDeselect={false}
								styles={{
									input: {
										backgroundColor: "var(--bg-elevated)",
										borderColor: "var(--border-default)",
										color: "var(--text-primary)",
										minWidth: 220,
									},
								}}
							/>
						</>
					}
				/>
			)}

			<SettingsRow
				label="Accent color"
				description="Changes the app highlight color"
				right={
					isProfileScope ? (
						<Tooltip label={GLOBAL_ONLY_TOOLTIP} withArrow position="top-start">
							<div style={{ opacity: 0.5, cursor: "not-allowed" }}>
								<div style={{ pointerEvents: "none" }}>
									<Select
										data={ACCENT_COLOR_OPTIONS}
										value={accentDropdownValue}
										onChange={handleAccentColorChange}
										disabled={true}
										withCheckIcon={false}
										leftSection={
											<span
												aria-hidden="true"
												style={{
													width: 10,
													height: 10,
													borderRadius: 999,
													backgroundColor: getAccentSwatch(accentDropdownValue),
													boxShadow: "0 0 0 1px rgba(255, 255, 255, 0.18)",
													display: "inline-block",
												}}
											/>
										}
										leftSectionPointerEvents="none"
										renderOption={({ option }) => (
											<div
												style={{
													display: "flex",
													alignItems: "center",
													gap: 8,
												}}
											>
												<span
													aria-hidden="true"
													style={{
														width: 10,
														height: 10,
														borderRadius: 999,
														backgroundColor: getAccentSwatch(option.value),
														boxShadow: "0 0 0 1px rgba(255, 255, 255, 0.18)",
														display: "inline-block",
														flex: "0 0 auto",
													}}
												/>
												<span>{option.label}</span>
											</div>
										)}
										styles={{
											input: {
												backgroundColor: "var(--bg-elevated)",
												borderColor: "var(--border-default)",
												color: "var(--text-primary)",
												minWidth: 180,
											},
										}}
									/>
								</div>
							</div>
						</Tooltip>
					) : (
						<Select
							data={ACCENT_COLOR_OPTIONS}
							value={accentDropdownValue}
							onChange={handleAccentColorChange}
							disabled={isLoading}
							withCheckIcon={false}
							leftSection={
								<span
									aria-hidden="true"
									style={{
										width: 10,
										height: 10,
										borderRadius: 999,
										backgroundColor: getAccentSwatch(accentDropdownValue),
										boxShadow: "0 0 0 1px rgba(255, 255, 255, 0.18)",
										display: "inline-block",
									}}
								/>
							}
							leftSectionPointerEvents="none"
							renderOption={({ option }) => (
								<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
									<span
										aria-hidden="true"
										style={{
											width: 10,
											height: 10,
											borderRadius: 999,
											backgroundColor: getAccentSwatch(option.value),
											boxShadow: "0 0 0 1px rgba(255, 255, 255, 0.18)",
											display: "inline-block",
											flex: "0 0 auto",
										}}
									/>
									<span>{option.label}</span>
								</div>
							)}
							styles={{
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
									minWidth: 180,
								},
							}}
						/>
					)
				}
			/>

			<SettingsRow
				label="Close button"
				description="What happens when you close the settings window"
				right={
					<Tooltip
						label={GLOBAL_ONLY_TOOLTIP}
						disabled={!isProfileScope}
						withArrow
						position="top-start"
					>
						<div style={isProfileScope ? { opacity: 0.5 } : undefined}>
							<SegmentedControl
								value={mainWindowCloseBehavior}
								onChange={handleMainWindowCloseBehaviorChange}
								disabled={isLoading || isProfileScope}
								data={[
									{ value: "exit_program", label: "Exit Program" },
									{
										value: "minimize_to_tray",
										label: "Minimize to tray",
									},
								]}
								size="sm"
								radius="md"
								styles={{
									root: {
										backgroundColor: "var(--bg-elevated)",
										border: "1px solid var(--border-default)",
										minWidth: 260,
									},
								}}
							/>
						</div>
					</Tooltip>
				}
			/>
		</>
	);
}
