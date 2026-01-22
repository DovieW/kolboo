import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
	CleanupPromptSections,
	RewriteProgramPromptProfile,
} from "../../../lib/tauri";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

interface LocalSectionState {
	content: string;
}

export interface LocalSections {
	system: LocalSectionState;
}

export type SectionKey = "system";

type CleanupPromptSectionsOverride = {
	system?: CleanupPromptSections["system"] | null;
};

const DEFAULT_SECTIONS: CleanupPromptSections = {
	system: { content: null },
};

// ─────────────────────────────────────────────────────────────────────────────
// Hook options
// ─────────────────────────────────────────────────────────────────────────────

type UseSectionManagementOptions = {
	settings:
		| { cleanup_prompt_sections: CleanupPromptSections | null }
		| undefined;
	defaultSections: { system: string } | undefined;
	activeProfileId: string;
	profiles: RewriteProgramPromptProfile[];
	activeProfile: RewriteProgramPromptProfile | null;
	updateCleanupPromptSections: {
		mutate: (
			sections: CleanupPromptSections,
			opts?: { onSuccess?: () => void },
		) => void;
		isPending: boolean;
	};
	saveProfileMetadata: (updates: Partial<RewriteProgramPromptProfile>) => void;
};

export type SectionManagementReturn = {
	localSections: LocalSections | null;
	setLocalSections: React.Dispatch<React.SetStateAction<LocalSections | null>>;
	effectiveCurrentPrompt: string;
	profilePromptOverridesRef: React.MutableRefObject<CleanupPromptSectionsOverride | null>;
	handleSave: (key: SectionKey, content: string) => void;
	handleReset: (key: SectionKey) => void;
	buildSections: (overrides?: {
		key: SectionKey;
		content?: string | null;
	}) => CleanupPromptSections;
	buildSectionOverride: (
		key: SectionKey,
		overrides?: { content?: string | null },
	) => CleanupPromptSections[SectionKey];
	saveProfileSectionOverride: (
		key: SectionKey,
		section: CleanupPromptSections[SectionKey] | null,
	) => void;
	normalizePromptOverrides: (
		overrides: CleanupPromptSectionsOverride,
	) => CleanupPromptSectionsOverride | null;
};

/**
 * Hook that manages prompt section state, including local edits, saving,
 * resetting, and building section overrides for profiles.
 */
export function useSectionManagement({
	settings,
	defaultSections,
	activeProfileId,
	profiles,
	activeProfile,
	updateCleanupPromptSections,
	saveProfileMetadata,
}: UseSectionManagementOptions): SectionManagementReturn {
	// ─────────────────────────────────────────────────────────────────────────
	// Local sections state
	// ─────────────────────────────────────────────────────────────────────────

	const [localSections, setLocalSections] = useState<LocalSections | null>(
		null,
	);

	const effectiveCurrentPrompt = useMemo(() => {
		if (localSections == null) return "";
		return (localSections.system.content ?? "").trim();
	}, [localSections]);

	// When updating per-section prompt overrides quickly (e.g., toggling Advanced then
	// Dictionary immediately), relying on `activeProfile` can drop earlier changes
	// because the component may not have re-rendered with the optimistic update yet.
	// Keep a ref of the latest per-profile prompt overrides to merge safely.
	const profilePromptOverridesRef =
		useRef<CleanupPromptSectionsOverride | null>(null);

	useEffect(() => {
		profilePromptOverridesRef.current =
			activeProfile?.cleanup_prompt_sections ?? null;
	}, [activeProfile?.cleanup_prompt_sections]);

	// Initialize localSections when settings/defaultSections/profile changes
	useEffect(() => {
		if (settings !== undefined && defaultSections !== undefined) {
			const base = settings.cleanup_prompt_sections ?? DEFAULT_SECTIONS;

			const profileOverrides: CleanupPromptSectionsOverride | null | undefined =
				activeProfileId === "default"
					? null
					: profiles.find((p) => p.id === activeProfileId)
							?.cleanup_prompt_sections;

			const resolved: CleanupPromptSections =
				activeProfileId === "default"
					? base
					: {
							system: profileOverrides?.system ?? base.system,
						};

			setLocalSections({
				system: {
					content: resolved.system.content ?? defaultSections.system,
				},
			});
		}
	}, [settings, defaultSections, activeProfileId, profiles]);

	// ─────────────────────────────────────────────────────────────────────────
	// Section building utilities
	// ─────────────────────────────────────────────────────────────────────────

	const buildSections = useCallback(
		(overrides?: {
			key: SectionKey;
			content?: string | null;
		}): CleanupPromptSections => {
			if (localSections === null) {
				return DEFAULT_SECTIONS;
			}

			const getContent = (key: SectionKey): string | null => {
				const content =
					overrides?.key === key && overrides.content !== undefined
						? overrides.content
						: localSections[key].content;

				// Return null if content matches default (to use server default)
				if (content === defaultSections?.[key]) {
					return null;
				}
				return content || null;
			};

			return {
				system: { content: getContent("system") },
			};
		},
		[defaultSections, localSections],
	);

	const saveAllSections = useCallback(
		(sections: CleanupPromptSections) => {
			// Only used for Default scope. Per-profile prompt changes are stored as per-section overrides.
			updateCleanupPromptSections.mutate(sections);
		},
		[updateCleanupPromptSections],
	);

	const normalizePromptOverrides = useCallback(
		(
			overrides: CleanupPromptSectionsOverride,
		): CleanupPromptSectionsOverride | null => {
			const hasAny = overrides.system != null;
			return hasAny ? overrides : null;
		},
		[],
	);

	const saveProfileSectionOverride = useCallback(
		(key: SectionKey, section: CleanupPromptSections[SectionKey] | null) => {
			const current: CleanupPromptSectionsOverride =
				profilePromptOverridesRef.current ?? {};
			const next: CleanupPromptSectionsOverride = {
				...current,
				[key]: section,
			};
			const normalized = normalizePromptOverrides(next);
			profilePromptOverridesRef.current = normalized;

			saveProfileMetadata({ cleanup_prompt_sections: normalized });
		},
		[normalizePromptOverrides, saveProfileMetadata],
	);

	const buildSectionOverride = useCallback(
		(
			key: SectionKey,
			overrides?: { content?: string | null },
		): CleanupPromptSections[SectionKey] => {
			if (localSections === null) {
				return DEFAULT_SECTIONS[key];
			}

			const contentRaw =
				overrides?.content !== undefined
					? overrides.content
					: localSections[key].content;

			const contentToStore =
				contentRaw === defaultSections?.[key] ? null : contentRaw || null;

			return {
				content: contentToStore,
			};
		},
		[defaultSections, localSections],
	);

	// ─────────────────────────────────────────────────────────────────────────
	// Handlers
	// ─────────────────────────────────────────────────────────────────────────

	const handleSave = useCallback(
		(key: SectionKey, content: string) => {
			setLocalSections((prev) => {
				if (prev === null) return prev;
				return { ...prev, [key]: { ...prev[key], content } };
			});

			if (activeProfileId === "default") {
				saveAllSections(buildSections({ key, content }));
				return;
			}

			saveProfileSectionOverride(key, buildSectionOverride(key, { content }));
		},
		[
			activeProfileId,
			buildSectionOverride,
			buildSections,
			saveAllSections,
			saveProfileSectionOverride,
		],
	);

	const handleReset = useCallback(
		(key: SectionKey) => {
			const defaultContent = defaultSections?.[key] ?? "";
			setLocalSections((prev) => {
				if (prev === null) return prev;
				return { ...prev, [key]: { ...prev[key], content: defaultContent } };
			});

			if (activeProfileId === "default") {
				saveAllSections(buildSections({ key, content: null }));
				return;
			}

			saveProfileSectionOverride(
				key,
				buildSectionOverride(key, { content: null }),
			);
		},
		[
			activeProfileId,
			buildSectionOverride,
			buildSections,
			defaultSections,
			saveAllSections,
			saveProfileSectionOverride,
		],
	);

	return {
		localSections,
		setLocalSections,
		effectiveCurrentPrompt,
		profilePromptOverridesRef,
		handleSave,
		handleReset,
		buildSections,
		buildSectionOverride,
		saveProfileSectionOverride,
		normalizePromptOverrides,
	};
}
