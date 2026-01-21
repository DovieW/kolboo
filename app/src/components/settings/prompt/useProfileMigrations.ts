import { useEffect, useRef } from "react";
import type {
	AppSettings,
	RewriteProgramPromptProfile,
} from "../../../lib/tauri";
import { tauriAPI } from "../../../lib/tauri";

type UpdateMutation = {
	mutate: (
		profiles: RewriteProgramPromptProfile[],
		options?: { onSuccess?: () => void },
	) => void;
};

export type UseProfileMigrationsOptions = {
	settings: AppSettings | undefined;
	profiles: RewriteProgramPromptProfile[];
	defaultRewriteEnabled: boolean;
	updateRewriteProgramPromptProfiles: UpdateMutation;
};

/**
 * Handles one-time profile migrations:
 *
 * 1. Ensures the "default" profile exists as a persisted profile object
 * 2. Migrates old profiles to have their own rewrite_llm_enabled flag
 *
 * These migrations run once per component mount and are guarded by refs.
 */
export function useProfileMigrations({
	settings,
	profiles,
	defaultRewriteEnabled,
	updateRewriteProgramPromptProfiles,
}: UseProfileMigrationsOptions): void {
	// Ensure every profile has its own rewrite enable flag.
	// This prevents the Default toggle from affecting other profiles.
	const didEnsureDefaultProfile = useRef(false);
	useEffect(() => {
		if (didEnsureDefaultProfile.current) return;
		if (!settings) return;

		// Ensure the Default profile exists as a real, persisted profile object so it can
		// own presets/router configuration.
		const hasDefault = profiles.some((p) => p.id === "default");
		if (hasDefault) {
			didEnsureDefaultProfile.current = true;
			return;
		}

		didEnsureDefaultProfile.current = true;

		const defaultProfile: RewriteProgramPromptProfile = {
			id: "default",
			name: "Default",
			program_paths: [],
			cleanup_prompt_sections: null,
			presets: [],
			default_preset_id: null,
			default_preset_description: null,
			default_target_rewrite_llm_enabled: true,
			router: null,
			active_preset_id: null,
			// Default profile uses the global rewrite toggle.
			rewrite_llm_enabled: null,
			stt_provider: null,
			stt_model: null,
			stt_timeout_seconds: null,
			llm_provider: null,
			llm_model: null,
			openai_reasoning_effort: null,
			gemini_thinking_budget: null,
			gemini_thinking_level: null,
			anthropic_thinking_budget: null,

			context_grab_method: null,

			sound_enabled: null,
			playing_audio_handling: null,
			overlay_mode: null,
			widget_position: null,
			output_mode: null,
			output_hit_enter: null,
		};

		// Insert Default first so it doesn't show up as a "program profile" elsewhere.
		updateRewriteProgramPromptProfiles.mutate([defaultProfile, ...profiles], {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
			},
		});
	}, [profiles, settings, updateRewriteProgramPromptProfiles]);

	const didMigrateProfileRewriteEnabled = useRef(false);
	useEffect(() => {
		if (didMigrateProfileRewriteEnabled.current) return;
		if (!settings) return;
		if (profiles.length === 0) return;

		const needsMigration = profiles.some(
			(p) => p.id !== "default" && typeof p.rewrite_llm_enabled !== "boolean",
		);
		if (!needsMigration) {
			didMigrateProfileRewriteEnabled.current = true;
			return;
		}

		didMigrateProfileRewriteEnabled.current = true;

		const migrated = profiles.map((p) => {
			if (p.id === "default") return p;
			const current = p.rewrite_llm_enabled;
			if (typeof current === "boolean") return p;
			return { ...p, rewrite_llm_enabled: defaultRewriteEnabled };
		});

		updateRewriteProgramPromptProfiles.mutate(migrated, {
			onSuccess: () => {
				tauriAPI.emitSettingsChanged();
			},
		});
	}, [
		settings,
		profiles,
		defaultRewriteEnabled,
		updateRewriteProgramPromptProfiles,
	]);
}
