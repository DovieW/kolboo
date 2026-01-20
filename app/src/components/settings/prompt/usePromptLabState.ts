import { useEffect, useState } from "react";
import type { RewritePreset } from "../../../lib/tauri";
import type { SectionKey } from "./useSectionManagement";

type PromptLabApplyTarget =
	| { type: "profile"; key: SectionKey }
	| { type: "preset"; presetId: string; key: SectionKey }
	| null;

export type UsePromptLabStateOptions = {
	effectiveCurrentPrompt: string;
	activeProfileLabel: string;
};

export type PromptLabState = {
	promptLabOpen: boolean;
	setPromptLabOpen: (open: boolean) => void;
	promptLabContextPrompt: string;
	promptLabContextLabel: string;
	promptLabApplyTarget: PromptLabApplyTarget;
	setPromptLabApplyTarget: (target: PromptLabApplyTarget) => void;
	handleOpenPresetPromptLab: (
		preset: RewritePreset,
		key: SectionKey,
		initialContent: string,
	) => void;
	handleOpenDefaultPromptLab: () => void;
	closePromptLab: () => void;
};

/**
 * Encapsulates Prompt Lab modal state and handlers.
 *
 * - Manages open/close state
 * - Keeps context prompt/label aligned with current scope when closed
 * - Provides handlers to open the modal for presets or default prompts
 */
export function usePromptLabState({
	effectiveCurrentPrompt,
	activeProfileLabel,
}: UsePromptLabStateOptions): PromptLabState {
	const [promptLabOpen, setPromptLabOpen] = useState(false);
	const [promptLabContextPrompt, setPromptLabContextPrompt] = useState("");
	const [promptLabContextLabel, setPromptLabContextLabel] = useState("");
	const [promptLabApplyTarget, setPromptLabApplyTarget] =
		useState<PromptLabApplyTarget>(null);

	// Keep PromptLab context aligned with current scope by default.
	useEffect(() => {
		if (!promptLabOpen) {
			setPromptLabContextPrompt(effectiveCurrentPrompt);
			setPromptLabContextLabel(activeProfileLabel);
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [effectiveCurrentPrompt, activeProfileLabel, promptLabOpen]);

	const handleOpenPresetPromptLab = (
		preset: RewritePreset,
		key: SectionKey,
		initialContent: string,
	) => {
		const presetLabel = preset.name?.trim() || preset.id;
		setPromptLabContextPrompt(initialContent.trim());
		setPromptLabContextLabel(`${activeProfileLabel} · ${presetLabel}`);
		setPromptLabApplyTarget({ type: "preset", presetId: preset.id, key });
		setPromptLabOpen(true);
	};

	const handleOpenDefaultPromptLab = () => {
		setPromptLabContextPrompt(effectiveCurrentPrompt);
		setPromptLabContextLabel(activeProfileLabel);
		setPromptLabApplyTarget({ type: "profile", key: "system" });
		setPromptLabOpen(true);
	};

	const closePromptLab = () => {
		setPromptLabOpen(false);
		setPromptLabApplyTarget(null);
	};

	return {
		promptLabOpen,
		setPromptLabOpen,
		promptLabContextPrompt,
		promptLabContextLabel,
		promptLabApplyTarget,
		setPromptLabApplyTarget,
		handleOpenPresetPromptLab,
		handleOpenDefaultPromptLab,
		closePromptLab,
	};
}
