import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useUpdateRewriteProgramPromptProfiles } from "../../../lib/queries";
import { tauriAPI } from "../../../lib/tauri";
import type {
	RewritePreset,
	RewriteProgramPromptProfile,
} from "../../../lib/tauri/types";

/** Sentinel value for editing the "default" pseudo-preset. */
export const EDIT_DEFAULT_PRESET = "__default__";

/** Option used in link preset modal for selecting a profile. */
export interface LinkableProfileOption {
	id: string;
	label: string;
	presets: RewritePreset[];
}

/** Dialog state for confirming preset deletion. */
export interface DeletePresetDialogState {
	presetId: string;
	presetName: string;
	isShared: boolean;
}

/** Generate a new unique ID for a preset. */
function createId(): string {
	return (
		globalThis.crypto?.randomUUID?.() ??
		`id_${Date.now()}_${Math.random().toString(16).slice(2)}`
	);
}

export interface UsePresetManagementParams {
	activeProfile: RewriteProgramPromptProfile | null;
	activeProfileId: string;
	profiles: RewriteProgramPromptProfile[];
	saveProfileMetadata: (patch: Partial<RewriteProgramPromptProfile>) => void;
}

export interface UsePresetManagementResult {
	// State
	presets: RewritePreset[];
	editingPresetId: string | null;
	setEditingPresetId: (id: string | null) => void;
	selectedPreset: RewritePreset | null;
	isEditingDefaultPreset: boolean;

	// Local form state for preset editor
	localPresetName: string;
	setLocalPresetName: (name: string) => void;
	localPresetHintsText: string;
	setLocalPresetHintsText: (text: string) => void;
	localDefaultPresetDescription: string;
	setLocalDefaultPresetDescription: (desc: string) => void;

	// Preset editor modal
	presetEditorOpen: boolean;
	setPresetEditorOpen: (open: boolean) => void;

	// Delete preset dialog
	deletePresetDialog: DeletePresetDialogState | null;
	setDeletePresetDialog: (state: DeletePresetDialogState | null) => void;
	handleConfirmDeletePreset: () => void;

	// Link preset modal
	linkPresetModalOpen: boolean;
	setLinkPresetModalOpen: (open: boolean) => void;
	linkableProfiles: LinkableProfileOption[];
	linkSourceProfileId: string | null;
	linkSourcePresetId: string | null;
	linkSourceProfile: LinkableProfileOption | null;
	linkSourcePreset: RewritePreset | null;
	openLinkPresetModal: () => void;
	confirmLinkPreset: () => void;
	handleLinkSourceProfileChange: (value: string) => void;
	handleLinkSourcePresetChange: (value: string) => void;

	// CRUD operations
	newPreset: () => void;
	updatePreset: (presetId: string, patch: Partial<RewritePreset>) => void;
	deletePreset: (presetId: string) => void;
	isSharedPresetId: (presetId: string) => boolean;

	// Mutation state
	isSavingProfiles: boolean;
}

export function usePresetManagement({
	activeProfile,
	activeProfileId,
	profiles,
	saveProfileMetadata,
}: UsePresetManagementParams): UsePresetManagementResult {
	const updateRewriteProgramPromptProfiles =
		useUpdateRewriteProgramPromptProfiles();

	// Computed presets for active profile
	const presets: RewritePreset[] = useMemo(() => {
		if (!activeProfile) return [];
		return Array.isArray(activeProfile.presets) ? activeProfile.presets : [];
	}, [activeProfile]);

	// Helper to get presets for any profile
	const getPresetsForProfile = useCallback(
		(p: RewriteProgramPromptProfile): RewritePreset[] => {
			const raw = p.presets;
			return Array.isArray(raw) ? raw : [];
		},
		[],
	);

	// Count how many profiles reference each preset ID (for shared detection)
	const presetRefCounts = useMemo(() => {
		const counts = new Map<string, number>();
		for (const profile of profiles) {
			const seen = new Set<string>();
			for (const preset of getPresetsForProfile(profile)) {
				if (seen.has(preset.id)) continue;
				seen.add(preset.id);
				counts.set(preset.id, (counts.get(preset.id) ?? 0) + 1);
			}
		}
		return counts;
	}, [profiles, getPresetsForProfile]);

	const isSharedPresetId = useCallback(
		(presetId: string): boolean => {
			return (presetRefCounts.get(presetId) ?? 0) > 1;
		},
		[presetRefCounts],
	);

	// Editing state
	const [editingPresetId, setEditingPresetId] = useState<string | null>(null);
	const pendingPresetIdRef = useRef<string | null>(null);
	const [localPresetName, setLocalPresetName] = useState<string>("");
	const [localPresetHintsText, setLocalPresetHintsText] = useState<string>("");
	const [localDefaultPresetDescription, setLocalDefaultPresetDescription] =
		useState<string>("");

	// Modal/dialog state
	const [presetEditorOpen, setPresetEditorOpen] = useState(false);
	const [deletePresetDialog, setDeletePresetDialog] =
		useState<DeletePresetDialogState | null>(null);
	const [linkPresetModalOpen, setLinkPresetModalOpen] = useState(false);
	const [linkSourceProfileId, setLinkSourceProfileId] = useState<string | null>(
		null,
	);
	const [linkSourcePresetId, setLinkSourcePresetId] = useState<string | null>(
		null,
	);

	// Selected preset in editor
	const selectedPreset: RewritePreset | null = useMemo(() => {
		if (!activeProfile) return null;
		if (!editingPresetId) return null;
		return presets.find((p) => p.id === editingPresetId) ?? null;
	}, [activeProfile, presets, editingPresetId]);

	const isEditingDefaultPreset = editingPresetId === EDIT_DEFAULT_PRESET;

	// Sync editing preset ID when profile/presets change
	useEffect(() => {
		if (!activeProfile) {
			setEditingPresetId(null);
			pendingPresetIdRef.current = null;
			return;
		}

		// Keep selection stable where possible.
		if (
			editingPresetId &&
			(editingPresetId === EDIT_DEFAULT_PRESET ||
				presets.some((p) => p.id === editingPresetId))
		) {
			if (
				pendingPresetIdRef.current === editingPresetId &&
				presets.some((p) => p.id === editingPresetId)
			) {
				pendingPresetIdRef.current = null;
			}
			return;
		}

		// We may have just created/linked a preset and are waiting for settings to
		// propagate back into `activeProfile.presets`. Don't snap the selection back.
		if (editingPresetId && pendingPresetIdRef.current === editingPresetId) {
			return;
		}

		setEditingPresetId(presets[0]?.id ?? EDIT_DEFAULT_PRESET);
	}, [activeProfile, presets, editingPresetId]);

	// Sync local form state when selected preset changes
	useEffect(() => {
		if (!selectedPreset) {
			setLocalPresetName("");
			setLocalPresetHintsText("");
			return;
		}

		setLocalPresetName(selectedPreset.name);
		const lines = (selectedPreset.routing_hints ?? []).filter(Boolean);
		setLocalPresetHintsText(lines.join("\n"));
	}, [selectedPreset]);

	// Sync local default preset description when profile changes
	useEffect(() => {
		if (!activeProfile) {
			setLocalDefaultPresetDescription("");
			return;
		}
		setLocalDefaultPresetDescription(
			activeProfile.default_preset_description ?? "",
		);
	}, [activeProfile]);

	// Save presets to profile
	const savePresets = useCallback(
		(
			nextPresets: RewritePreset[],
			extra?: Partial<RewriteProgramPromptProfile>,
		) => {
			if (!activeProfile) return;
			saveProfileMetadata({ presets: nextPresets, ...(extra ?? {}) });
		},
		[activeProfile, saveProfileMetadata],
	);

	// Update a preset (handles shared presets across profiles)
	const updatePreset = useCallback(
		(presetId: string, patch: Partial<RewritePreset>) => {
			// When a preset id appears in more than one profile, treat it like a shared
			// entity: edits in any profile update all profiles that reference that id.
			if (isSharedPresetId(presetId)) {
				const updated = profiles.map((profile) => {
					const profilePresets = getPresetsForProfile(profile);
					if (!profilePresets.some((p) => p.id === presetId)) return profile;
					return {
						...profile,
						presets: profilePresets.map((p) =>
							p.id === presetId ? { ...p, ...patch } : p,
						),
					};
				});

				updateRewriteProgramPromptProfiles.mutate(updated, {
					onSuccess: () => {
						tauriAPI.emitSettingsChanged();
					},
				});
				return;
			}

			const next = presets.map((p) =>
				p.id === presetId ? { ...p, ...patch } : p,
			);
			savePresets(next);
		},
		[
			isSharedPresetId,
			profiles,
			getPresetsForProfile,
			updateRewriteProgramPromptProfiles,
			presets,
			savePresets,
		],
	);

	// Delete a preset
	const deletePreset = useCallback(
		(presetId: string) => {
			if (!activeProfile) return;
			const nextPresets = presets.filter((p) => p.id !== presetId);

			const nextProfilePatch: Partial<RewriteProgramPromptProfile> = {};
			if (activeProfile.default_preset_id === presetId) {
				nextProfilePatch.default_preset_id = null;
			}
			if (activeProfile.active_preset_id === presetId) {
				nextProfilePatch.active_preset_id = null;
			}

			savePresets(nextPresets, nextProfilePatch);
			if (editingPresetId === presetId) {
				setEditingPresetId(nextPresets[0]?.id ?? null);
			}
		},
		[activeProfile, presets, editingPresetId, savePresets],
	);

	// Create a new preset
	const newPreset = useCallback(() => {
		if (!activeProfile) return;
		const id = createId();
		const p: RewritePreset = {
			id,
			name: "New preset",
			routing_hints: null,
			cleanup_prompt_sections: null,
			// Default presets to rewrite "On".
			rewrite_llm_enabled: true,
			stt_provider: null,
			stt_model: null,
			stt_timeout_seconds: null,
			llm_provider: null,
			llm_model: null,
			openai_reasoning_effort: null,
			gemini_thinking_budget: null,
			gemini_thinking_level: null,
			anthropic_thinking_budget: null,
			sound_enabled: null,
			playing_audio_handling: null,
			overlay_mode: null,
			widget_position: null,
			output_mode: null,
			output_hit_enter: null,
		};

		const next = [...presets, p];
		savePresets(next);
		pendingPresetIdRef.current = id;
		setEditingPresetId(id);
	}, [activeProfile, presets, savePresets]);

	// Linkable profiles (profiles with presets that can be linked)
	const linkableProfiles: LinkableProfileOption[] = useMemo(() => {
		return profiles
			.filter((p) => p.id !== activeProfileId)
			.map((p) => {
				const profilePresets = getPresetsForProfile(p);
				return {
					id: p.id,
					label: p.name?.trim() || p.id,
					presets: profilePresets,
				};
			})
			.filter((p) => p.presets.length > 0);
	}, [profiles, activeProfileId, getPresetsForProfile]);

	const linkSourceProfile = useMemo(() => {
		if (!linkSourceProfileId) return null;
		return linkableProfiles.find((p) => p.id === linkSourceProfileId) ?? null;
	}, [linkSourceProfileId, linkableProfiles]);

	const linkSourcePreset = useMemo(() => {
		if (!linkSourceProfile) return null;
		if (!linkSourcePresetId) return null;
		return (
			linkSourceProfile.presets.find((p) => p.id === linkSourcePresetId) ?? null
		);
	}, [linkSourceProfile, linkSourcePresetId]);

	const openLinkPresetModal = useCallback(() => {
		const firstProfile = linkableProfiles[0] ?? null;
		if (!firstProfile) return;
		setLinkSourceProfileId(firstProfile.id);
		setLinkSourcePresetId(firstProfile.presets[0]?.id ?? null);
		setLinkPresetModalOpen(true);
	}, [linkableProfiles]);

	const confirmLinkPreset = useCallback(() => {
		if (!activeProfile) return;
		if (!linkSourcePreset) return;

		// If it's already linked, just switch the editor to it.
		if (presets.some((p) => p.id === linkSourcePreset.id)) {
			setEditingPresetId(linkSourcePreset.id);
			setLinkPresetModalOpen(false);
			return;
		}

		// "Hard link" semantics: we reuse the same preset id across profiles.
		// We still store an object in this profile, but updates propagate by id.
		const next = [...presets, { ...linkSourcePreset }];
		savePresets(next);
		pendingPresetIdRef.current = linkSourcePreset.id;
		setEditingPresetId(linkSourcePreset.id);
		setLinkPresetModalOpen(false);
	}, [activeProfile, linkSourcePreset, presets, savePresets]);

	const handleLinkSourceProfileChange = useCallback(
		(value: string) => {
			setLinkSourceProfileId(value);
			const nextProfile = linkableProfiles.find((p) => p.id === value) ?? null;
			setLinkSourcePresetId(nextProfile?.presets[0]?.id ?? null);
		},
		[linkableProfiles],
	);

	const handleLinkSourcePresetChange = useCallback((value: string) => {
		setLinkSourcePresetId(value);
	}, []);

	const handleConfirmDeletePreset = useCallback(() => {
		const args = deletePresetDialog;
		if (!args) return;
		setDeletePresetDialog(null);
		deletePreset(args.presetId);
	}, [deletePresetDialog, deletePreset]);

	return {
		// State
		presets,
		editingPresetId,
		setEditingPresetId,
		selectedPreset,
		isEditingDefaultPreset,

		// Local form state
		localPresetName,
		setLocalPresetName,
		localPresetHintsText,
		setLocalPresetHintsText,
		localDefaultPresetDescription,
		setLocalDefaultPresetDescription,

		// Preset editor modal
		presetEditorOpen,
		setPresetEditorOpen,

		// Delete preset dialog
		deletePresetDialog,
		setDeletePresetDialog,
		handleConfirmDeletePreset,

		// Link preset modal
		linkPresetModalOpen,
		setLinkPresetModalOpen,
		linkableProfiles,
		linkSourceProfileId,
		linkSourcePresetId,
		linkSourceProfile,
		linkSourcePreset,
		openLinkPresetModal,
		confirmLinkPreset,
		handleLinkSourceProfileChange,
		handleLinkSourcePresetChange,

		// CRUD operations
		newPreset,
		updatePreset,
		deletePreset,
		isSharedPresetId,

		// Mutation state
		isSavingProfiles: updateRewriteProgramPromptProfiles.isPending,
	};
}
