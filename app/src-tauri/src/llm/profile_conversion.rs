//! Shared persisted profile/preset to runtime conversion. Startup and refresh
//! must preserve exactly the same routing overrides and prompt inheritance.
use crate::llm;

pub(crate) fn build_program_prompt_profiles(
    rewrite_program_prompt_profiles: Vec<crate::settings::RewriteProgramPromptProfile>,
    base_prompts: &llm::PromptSections,
) -> Vec<llm::ProgramPromptProfile> {
    rewrite_program_prompt_profiles
        .into_iter()
        .map(|p| {
            let profile_prompts = p
                .cleanup_prompt_sections
                .as_ref()
                .map(|o| o.apply_to(base_prompts))
                .unwrap_or_else(|| base_prompts.clone());

            let presets = p
                .presets
                .into_iter()
                .map(|preset| {
                    let preset_prompts = preset
                        .cleanup_prompt_sections
                        .as_ref()
                        .map(|o| o.apply_to(&profile_prompts))
                        .unwrap_or_else(|| profile_prompts.clone());

                    llm::ProgramPreset {
                        model_selections: preset.model_selections.clone(),
                        id: preset.id,
                        name: preset.name,
                        routing_hints: preset.routing_hints,
                        prompts: preset_prompts,
                        rewrite_llm_enabled: preset.rewrite_llm_enabled,
                        stt_provider: preset.stt_provider,
                        stt_model: preset.stt_model,
                        stt_language: preset.stt_language,
                        stt_timeout_seconds: preset.stt_timeout_seconds,
                        llm_provider: preset.llm_provider,
                        llm_model: preset.llm_model,
                        openai_reasoning_effort: preset.openai_reasoning_effort,
                        gemini_thinking_budget: preset.gemini_thinking_budget,
                        gemini_thinking_level: preset.gemini_thinking_level,
                        anthropic_thinking_budget: preset.anthropic_thinking_budget,
                    }
                })
                .collect();

            llm::ProgramPromptProfile {
                model_selections: p.model_selections.clone(),
                id: p.id,
                name: p.name,
                program_paths: p.program_paths,
                prompts: profile_prompts,

                presets,
                default_preset_id: p.default_preset_id,
                default_preset_description: p.default_preset_description,
                default_target_rewrite_llm_enabled: p.default_target_rewrite_llm_enabled,
                active_preset_id: p.active_preset_id,
                router: p.router,

                rewrite_llm_enabled: p.rewrite_llm_enabled,
                stt_provider: p.stt_provider,
                stt_model: p.stt_model,
                stt_language: p.stt_language,
                stt_timeout_seconds: p.stt_timeout_seconds,
                llm_provider: p.llm_provider,
                llm_model: p.llm_model,
                openai_reasoning_effort: p.openai_reasoning_effort,
                gemini_thinking_budget: p.gemini_thinking_budget,
                gemini_thinking_level: p.gemini_thinking_level,
                anthropic_thinking_budget: p.anthropic_thinking_budget,

                quick_ask_provider: p.quick_ask_provider,
                quick_ask_model: p.quick_ask_model,
                quick_ask_system_prompt: p.quick_ask_system_prompt,
                context_grab_method: p.context_grab_method,

                quick_replace_enabled: p.quick_replace_enabled,
                quick_replace_provider: p.quick_replace_provider,
                quick_replace_model: p.quick_replace_model,
                quick_replace_system_prompt: p.quick_replace_system_prompt,
                quick_ask_openai_reasoning_effort: p.quick_ask_openai_reasoning_effort,
                quick_ask_gemini_thinking_budget: p.quick_ask_gemini_thinking_budget,
                quick_ask_gemini_thinking_level: p.quick_ask_gemini_thinking_level,
                quick_ask_anthropic_thinking_budget: p.quick_ask_anthropic_thinking_budget,

                rewrite_include_clipboard_context: p.rewrite_include_clipboard_context,
                quick_replace_include_clipboard_context: p.quick_replace_include_clipboard_context,
                quick_ask_include_clipboard_context: p.quick_ask_include_clipboard_context,

                rewrite_active_window_ocr_mode: p.rewrite_active_window_ocr_mode,
                quick_replace_active_window_ocr_mode: p.quick_replace_active_window_ocr_mode,
                quick_ask_active_window_ocr_mode: p.quick_ask_active_window_ocr_mode,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_selection::{ModelAction, ModelSource};

    #[test]
    fn startup_and_refresh_preserve_independent_routes_and_prompt_inheritance() {
        let profiles=serde_json::from_value(serde_json::json!([
            {"id":"p","name":"Profile","program_paths":["/editor"],"llm_provider":"legacy","stt_language":"yi",
            "model_selections":{"rewrite":{"source":"managed","provider_id":"openai","model_id":"profile-model"}},
            "cleanup_prompt_sections":{"system":{"content":"profile prompt"}},
            "presets":[
                {"id":"q","name":"Preset","model_selections":{"rewrite":{"source":"byok","provider_id":"custom_text","model_id":"preset-model"}},"cleanup_prompt_sections":{"system":{"content":"preset prompt"}}},
                {"id":"inherit","name":"Inherited"}
            ]},
            {"id":"plain","name":"Plain"}
        ])).unwrap();
        let base = llm::PromptSections::default();
        let result = build_program_prompt_profiles(profiles, &base);
        let profile = &result[0];
        assert_eq!(profile.id, "p");
        assert_eq!(profile.program_paths, ["/editor"]);
        assert_eq!(profile.llm_provider.as_deref(), Some("legacy"));
        assert_eq!(profile.stt_language.as_deref(), Some("yi"));
        assert_eq!(
            profile.model_selections[&ModelAction::Rewrite].source,
            ModelSource::Managed
        );
        assert_eq!(
            profile.model_selections[&ModelAction::Rewrite].model_id,
            "profile-model"
        );
        let preset = &profile.presets[0];
        assert_eq!(
            preset.model_selections[&ModelAction::Rewrite].source,
            ModelSource::Byok
        );
        assert_eq!(
            preset.model_selections[&ModelAction::Rewrite].model_id,
            "preset-model"
        );
        assert_eq!(
            profile.prompts.system_custom.as_deref(),
            Some("profile prompt")
        );
        assert_eq!(
            preset.prompts.system_custom.as_deref(),
            Some("preset prompt")
        );
        assert_eq!(
            profile.presets[1].prompts.system_custom,
            profile.prompts.system_custom
        );
        assert_eq!(result[1].prompts.system_custom, base.system_custom);
        assert!(result[1].model_selections.is_empty());
    }
}
