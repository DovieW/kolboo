import {
	ActionIcon,
	Button,
	Group,
	Loader,
	Modal,
	Select,
	Text,
	TextInput,
	Tooltip,
} from "@mantine/core";
import { Crosshair, Plus, RotateCcw, Trash2 } from "lucide-react";
import type { ReactNode } from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
	useSettings,
	useUpdateRewriteProgramPromptProfiles,
} from "../../lib/queries";
import type { OpenWindowInfo } from "../../lib/tauri";
import { type RewriteProgramPromptProfile, tauriAPI } from "../../lib/tauri";

function createId(): string {
	// `crypto.randomUUID()` is available in modern browsers; keep a fallback for safety.
	// This only needs to be unique enough for local settings.
	return (
		globalThis.crypto?.randomUUID?.() ??
		`id_${Date.now()}_${Math.random().toString(16).slice(2)}`
	);
}

export function ProfileConfigModal({
	opened,
	onClose,
	editingProfileId,
	onEditingProfileChange,
	autoCreateProfile,
}: {
	opened: boolean;
	onClose: () => void;
	editingProfileId: string;
	onEditingProfileChange: (id: string) => void;
	autoCreateProfile?: boolean;
}) {
	const { data: settings, isLoading } = useSettings();
	const updateRewriteProgramPromptProfiles =
		useUpdateRewriteProgramPromptProfiles();

	const profiles: RewriteProgramPromptProfile[] =
		settings?.rewrite_program_prompt_profiles ?? [];

	const didAutoCreate = useRef(false);

	const addProfile = useCallback(() => {
		// Create new profile with all settings in inherit mode (null)
		// Settings will inherit from default until user explicitly changes them
		const newProfile: RewriteProgramPromptProfile = {
			id: createId(),
			name: "New Profile",
			program_paths: [],
			cleanup_prompt_sections: null, // inherit
			// AI settings - all inherit
			stt_provider: null,
			stt_model: null,
			stt_timeout_seconds: null,
			llm_provider: null,
			llm_model: null,
			rewrite_llm_enabled: null,
			// UI settings - all inherit
			sound_enabled: null,
			playing_audio_handling: null,
			overlay_mode: null,
			widget_position: null,
			output_mode: null,
		};

		const next = [...profiles, newProfile];
		updateRewriteProgramPromptProfiles.mutate(next, {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
				onEditingProfileChange(newProfile.id);
			},
		});
	}, [profiles, updateRewriteProgramPromptProfiles, onEditingProfileChange]);

	useEffect(() => {
		if (!opened) {
			didAutoCreate.current = false;
			return;
		}
		if (!autoCreateProfile) return;
		if (didAutoCreate.current) return;
		if (!settings) return;
		didAutoCreate.current = true;
		addProfile();
	}, [opened, autoCreateProfile, settings, addProfile]);

	const selectedProfile: RewriteProgramPromptProfile | null =
		editingProfileId !== "default"
			? (profiles.find((p) => p.id === editingProfileId) ?? null)
			: null;

	const [localName, setLocalName] = useState<string>("");
	const [localPaths, setLocalPaths] = useState<string[]>([]);

	const programPathInputRefs = useRef<Array<HTMLInputElement | null>>([]);
	const [pendingProgramPathFocusIndex, setPendingProgramPathFocusIndex] =
		useState<number | null>(null);
	const windowPickerSelectRef = useRef<HTMLInputElement>(null);

	const [confirmDialog, setConfirmDialog] = useState<null | {
		title: string;
		description?: ReactNode;
		confirmLabel?: string;
		cancelLabel?: string;
		confirmColor?: string;
		onConfirm: () => void;
	}>(null);

	const openConfirmDialog = (args: {
		title: string;
		description?: ReactNode;
		confirmLabel?: string;
		cancelLabel?: string;
		confirmColor?: string;
		onConfirm: () => void;
	}) => {
		setConfirmDialog(args);
	};

	useEffect(() => {
		if (!selectedProfile) {
			setLocalName("");
			setLocalPaths([]);
			setPendingProgramPathFocusIndex(null);
			return;
		}
		setLocalName(selectedProfile.name);
		setLocalPaths(selectedProfile.program_paths ?? []);
		setPendingProgramPathFocusIndex(null);
	}, [selectedProfile]);

	const saveProfileMetadata = (
		partial: Partial<RewriteProgramPromptProfile>,
	) => {
		if (!selectedProfile) return;

		const next = profiles.map((p) =>
			p.id === selectedProfile.id ? { ...p, ...partial } : p,
		);

		updateRewriteProgramPromptProfiles.mutate(next, {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
			},
		});
	};

	const deleteSelectedProfile = () => {
		if (!selectedProfile) return;

		const name = selectedProfile.name.trim() || "this profile";
		openConfirmDialog({
			title: `Delete ${name}?`,
			description: (
				<Text size="sm" c="dimmed" style={{ lineHeight: 1.4 }}>
					This cannot be undone.
				</Text>
			),
			confirmLabel: "Delete profile",
			cancelLabel: "Cancel",
			confirmColor: "red",
			onConfirm: () => {
				const next = profiles.filter((p) => p.id !== selectedProfile.id);
				updateRewriteProgramPromptProfiles.mutate(next, {
					onSuccess: () => {
						tauriAPI.emitSettingsChanged();
						// If we just deleted the currently selected profile, close the modal
						// and return to Default scope.
						if (editingProfileId === selectedProfile.id) {
							onEditingProfileChange("default");
							onClose();
						}
					},
				});
			},
		});
	};

	const disableAllOverridesForSelectedProfile = () => {
		if (!selectedProfile) return;
		const name = selectedProfile.name.trim() || "this profile";
		openConfirmDialog({
			title: `Disable all overrides for ${name}?`,
			description: (
				<Text size="sm" c="dimmed" style={{ lineHeight: 1.4 }}>
					This will reset the profile to inherit all settings from Default.
					Program matching will remain unchanged.
				</Text>
			),
			confirmLabel: "Disable overrides",
			cancelLabel: "Cancel",
			confirmColor: "orange",
			onConfirm: () => {
				saveProfileMetadata({
					// Prompts
					cleanup_prompt_sections: null,

					// AI settings
					stt_provider: null,
					stt_model: null,
					stt_timeout_seconds: null,
					llm_provider: null,
					llm_model: null,
					rewrite_llm_enabled: null,

					// UI settings
					sound_enabled: null,
					playing_audio_handling: null,
					overlay_mode: null,
					widget_position: null,
					output_mode: null,
				});
			},
		});
	};

	const updateProgramPathAtIndex = (idx: number, nextValue: string) => {
		const next = [...localPaths];
		next[idx] = nextValue;
		setLocalPaths(next);
	};

	const persistProgramPaths = (paths: string[]) => {
		if (!selectedProfile) return;
		saveProfileMetadata({ program_paths: paths });
	};

	const removeProgramPathAtIndex = (idx: number) => {
		const next = localPaths.filter((_, i) => i !== idx);
		setLocalPaths(next);
		persistProgramPaths(next);
	};

	const addEmptyProgramPath = () => {
		if (!selectedProfile) return;
		setPendingProgramPathFocusIndex(localPaths.length);
		setLocalPaths((prev) => [...prev, ""]);
	};

	const hasAnyConfiguredProgram = useMemo(
		() => localPaths.some((p) => p.trim().length > 0),
		[localPaths],
	);

	// Window picker (open app list)
	const [windowPickerOpen, setWindowPickerOpen] = useState(false);
	const [windowPickerDropdownOpened, setWindowPickerDropdownOpened] =
		useState(false);
	const [windowPickerEntered, setWindowPickerEntered] = useState(false);
	const [openWindows, setOpenWindows] = useState<OpenWindowInfo[]>([]);
	const [isLoadingWindows, setIsLoadingWindows] = useState(false);

	useEffect(() => {
		if (!opened) return;
		// Don't focus behind other modals.
		if (windowPickerOpen) return;
		if (confirmDialog) return;
		if (pendingProgramPathFocusIndex === null) return;

		const el = programPathInputRefs.current[pendingProgramPathFocusIndex];
		if (!el) return;

		// Wait a frame to ensure the input is mounted and laid out.
		requestAnimationFrame(() => {
			el.focus();
			try {
				const len = el.value.length;
				el.setSelectionRange(len, len);
			} catch {
				// Ignore environments that do not support setSelectionRange.
			}
		});

		setPendingProgramPathFocusIndex(null);
	}, [opened, windowPickerOpen, confirmDialog, pendingProgramPathFocusIndex]);

	useEffect(() => {
		if (!windowPickerOpen) return;
		if (!windowPickerEntered) return;
		if (isLoadingWindows) return;

		// Open dropdown and focus the Select input after the modal enter transition.
		// This avoids Mantine mis-positioning the dropdown (it needs layout to be final).
		setWindowPickerDropdownOpened(true);
		requestAnimationFrame(() => {
			windowPickerSelectRef.current?.focus();
		});
	}, [windowPickerOpen, windowPickerEntered, isLoadingWindows]);

	const windowPickerOptions = useMemo(() => {
		const byPath = new Map<string, OpenWindowInfo>();
		for (const w of openWindows) {
			if (!byPath.has(w.process_path)) {
				byPath.set(w.process_path, w);
			}
		}

		const basename = (p: string) => {
			const trimmed = p.trim().replaceAll('"', "");
			// Windows paths, but keep it tolerant.
			return trimmed.split(/[/\\]+/).pop() || trimmed;
		};

		return Array.from(byPath.values()).map((w) => ({
			value: w.process_path,
			// Avoid showing full filesystem paths in the picker by default.
			// Titles may be omitted by the backend for privacy.
			label: w.title?.trim()
				? `${w.title} — ${basename(w.process_path)}`
				: basename(w.process_path),
		}));
	}, [openWindows]);

	const openWindowPicker = async () => {
		setWindowPickerOpen(true);
		// Open the dropdown only after the modal finished its enter transition.
		// Otherwise Mantine positions the dropdown before the Select input is laid out,
		// which can cause it to render in the wrong place and overlap the whole modal.
		setWindowPickerDropdownOpened(false);
		setIsLoadingWindows(true);
		try {
			const windows = await tauriAPI.listOpenWindows();
			setOpenWindows(windows);
		} finally {
			setIsLoadingWindows(false);
		}
	};

	const addProgramPathFromPicker = (value: string) => {
		if (!selectedProfile) return;
		const trimmed = value.trim();
		if (!trimmed) return;

		const existing = new Set(localPaths.map((p) => p.trim()).filter(Boolean));
		if (existing.has(trimmed)) {
			setWindowPickerOpen(false);
			return;
		}

		const firstEmptyIndex = localPaths.findIndex((p) => p.trim().length === 0);

		const nextLocal = [...localPaths];
		if (firstEmptyIndex >= 0) {
			nextLocal[firstEmptyIndex] = trimmed;
		} else {
			nextLocal.push(trimmed);
		}

		// Persist a clean list (trim, remove empties, de-dupe while preserving order).
		const deduped: string[] = [];
		const seen = new Set<string>();
		for (const p of nextLocal) {
			const v = p.trim();
			if (!v) continue;
			if (seen.has(v)) continue;
			seen.add(v);
			deduped.push(v);
		}

		const focusIndex = deduped.indexOf(trimmed);
		if (focusIndex >= 0) {
			setPendingProgramPathFocusIndex(focusIndex);
		}

		setLocalPaths(deduped);
		persistProgramPaths(deduped);
		setWindowPickerOpen(false);
	};

	const isBusy = isLoading || updateRewriteProgramPromptProfiles.isPending;

	return (
		<>
			<Modal opened={opened} onClose={onClose} title="Profile config" centered>
				{editingProfileId === "default" ? (
					<div style={{ fontSize: 12, opacity: 0.75, padding: "8px 0" }}>
						Select a profile in the Profile dropdown above, then click the gear
						icon to configure it.
					</div>
				) : null}

				{editingProfileId !== "default" && !selectedProfile ? (
					<div style={{ fontSize: 12, opacity: 0.75, padding: "8px 0" }}>
						That profile no longer exists.
					</div>
				) : null}

				{selectedProfile ? (
					<>
						<TextInput
							label="Profile name"
							placeholder="e.g. VS Code"
							value={localName}
							onChange={(e) => setLocalName(e.currentTarget.value)}
							onBlur={() => {
								if (!selectedProfile) return;
								if (localName !== selectedProfile.name) {
									saveProfileMetadata({ name: localName });
								}
							}}
							styles={{ label: { fontSize: 12 } }}
						/>

						<Group justify="space-between" align="center" mt="sm" mb={6}>
							<div style={{ fontSize: 12, fontWeight: 500 }}>Programs</div>
							<Group gap={8}>
								<Tooltip
									label="Pick from currently open apps"
									withArrow
									position="bottom"
								>
									<ActionIcon
										variant="subtle"
										color="gray"
										onClick={openWindowPicker}
										aria-label="Pick from open programs"
										disabled={isBusy}
									>
										<Crosshair size={16} />
									</ActionIcon>
								</Tooltip>

								<Tooltip label="Add a program path" withArrow position="bottom">
									<ActionIcon
										variant="light"
										color="gray"
										onClick={addEmptyProgramPath}
										aria-label="Add program"
										disabled={isBusy}
									>
										<Plus size={16} />
									</ActionIcon>
								</Tooltip>
							</Group>
						</Group>

						{localPaths.length === 0 ? (
							<div style={{ fontSize: 12, opacity: 0.7, paddingBottom: 8 }}>
								No programs yet. Add one to apply this profile automatically.
							</div>
						) : null}

						{localPaths.map((path, idx) => (
							<Group
								key={`${selectedProfile.id}-path-${idx}`}
								mt={6}
								wrap="nowrap"
							>
								<TextInput
									ref={(el) => {
										programPathInputRefs.current[idx] = el;
									}}
									placeholder="C:\\Program Files\\...\\app.exe"
									value={path}
									onChange={(e) =>
										updateProgramPathAtIndex(idx, e.currentTarget.value)
									}
									onBlur={() => {
										const trimmed = localPaths.map((p) => p.trim());
										const filtered = trimmed.filter((p) => p.length > 0);

										const deduped: string[] = [];
										const seen = new Set<string>();
										for (const p of filtered) {
											if (!seen.has(p)) {
												seen.add(p);
												deduped.push(p);
											}
										}

										setLocalPaths(deduped);
										if (
											JSON.stringify(deduped) !==
											JSON.stringify(selectedProfile.program_paths ?? [])
										) {
											persistProgramPaths(deduped);
										}
									}}
									styles={{ input: { fontSize: 12 } }}
									style={{ flex: 1 }}
								/>
								<ActionIcon
									variant="subtle"
									color="red"
									aria-label="Remove program"
									onClick={() => removeProgramPathAtIndex(idx)}
									disabled={isBusy}
								>
									<Trash2 size={16} />
								</ActionIcon>
							</Group>
						))}

						{!hasAnyConfiguredProgram ? (
							<div style={{ fontSize: 12, opacity: 0.7, marginTop: 16 }}>
								Tip: you can use the crosshair to pick from open apps.
							</div>
						) : null}

						<Group justify="flex-end" mt="md" gap={8}>
							<Button
								size="xs"
								variant="light"
								color="gray"
								leftSection={<RotateCcw size={14} />}
								onClick={disableAllOverridesForSelectedProfile}
								disabled={isBusy}
							>
								Disable all overrides
							</Button>

							<Button
								size="xs"
								variant="light"
								color="red"
								leftSection={<Trash2 size={14} />}
								onClick={deleteSelectedProfile}
								disabled={isBusy}
							>
								Delete profile
							</Button>
						</Group>
					</>
				) : null}

				{updateRewriteProgramPromptProfiles.isPending ? (
					<Group justify="center" mt="md">
						<Loader size="sm" color="orange" />
					</Group>
				) : null}
			</Modal>

			<Modal
				opened={windowPickerOpen}
				onClose={() => {
					setWindowPickerOpen(false);
					setWindowPickerDropdownOpened(false);
					setWindowPickerEntered(false);
				}}
				title="Pick an open program"
				centered
				onEnterTransitionEnd={() => setWindowPickerEntered(true)}
				onExitTransitionEnd={() => {
					setWindowPickerEntered(false);
					setWindowPickerDropdownOpened(false);
				}}
			>
				{isLoadingWindows ? (
					<Group justify="center" p="md">
						<Loader size="sm" color="orange" />
					</Group>
				) : (
					<Select
						ref={windowPickerSelectRef}
						searchable
						nothingFoundMessage="No windows found"
						placeholder="Select a window"
						data={windowPickerOptions}
						dropdownOpened={windowPickerDropdownOpened}
						onDropdownOpen={() => setWindowPickerDropdownOpened(true)}
						onDropdownClose={() => setWindowPickerDropdownOpened(false)}
						onChange={(value) => {
							if (!value) return;
							addProgramPathFromPicker(value);
						}}
					/>
				)}
			</Modal>

			<Modal
				opened={confirmDialog !== null}
				onClose={() => setConfirmDialog(null)}
				title={confirmDialog?.title ?? ""}
				centered
				zIndex={1000}
			>
				{confirmDialog?.description}
				<Group justify="flex-end" mt="md" gap="sm">
					<Button variant="default" onClick={() => setConfirmDialog(null)}>
						{confirmDialog?.cancelLabel ?? "Cancel"}
					</Button>
					<Button
						color={confirmDialog?.confirmColor ?? "orange"}
						onClick={() => {
							const confirm = confirmDialog?.onConfirm;
							setConfirmDialog(null);
							confirm?.();
						}}
					>
						{confirmDialog?.confirmLabel ?? "Confirm"}
					</Button>
				</Group>
			</Modal>
		</>
	);
}
