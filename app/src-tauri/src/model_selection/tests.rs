use super::*;
use serde_json::json;

#[test]
fn enforced_policy_rejects_conflicting_atomic_routes_without_rewriting_the_source() {
    let saved = choice(ModelSource::Byok, "chosen", "model");
    for action in ModelAction::ALL {
        let paths = match action {
            ModelAction::Transcription => ["stt_provider", "stt_model"],
            ModelAction::QuickAsk => ["quick_ask_provider", "quick_ask_model"],
            ModelAction::QuickReplace => ["quick_replace_provider", "quick_replace_model"],
            _ => ["llm_provider", "llm_model"],
        };
        for scope in ["global", "profile", "preset"] {
            let mut settings = serde_json::Map::new();
            settings.insert("policy_state".into(),json!({"source":"cloud","is_valid":true,"enforced_fields":[{"path":paths[0]},{"path":paths[1]}]}));
            settings.insert(
                "policy_effective_values".into(),
                json!({paths[0]:"chosen",paths[1]:"model"}),
            );
            let name = serde_json::to_value(action).unwrap();
            let name = name.as_str().unwrap();
            match scope {
                "global" => {
                    settings.insert(action.settings_key().into(), json!(saved));
                }
                "profile" => {
                    settings.insert(
                        "rewrite_program_prompt_profiles".into(),
                        json!([{"id":"p","name":"p","model_selections":{name:saved}}]),
                    );
                }
                _ => {
                    settings.insert("rewrite_program_prompt_profiles".into(),json!([{"id":"p","name":"p","presets":[{"id":"q","name":"q","model_selections":{name:saved}}]}]));
                }
            }
            assert!(load_global_selections(&|key| settings.get(key).cloned())
                .error
                .is_none());
            for value in [json!("different"), json!(null)] {
                settings.insert("policy_effective_values".into(), json!({paths[0]:value}));
                let blocked = load_global_selections(&|key| settings.get(key).cloned());
                assert!(blocked.error.unwrap().contains("organization policy"));
                assert!(blocked.selections.is_empty() && blocked.profiles.is_empty());
            }
            settings.insert(
                "policy_state".into(),
                json!({"source":"cloud","is_valid":false}),
            );
            assert!(load_global_selections(&|key| settings.get(key).cloned())
                .error
                .is_none());
            assert_eq!(saved.source, ModelSource::Byok);
        }
    }
}

#[test]
fn scoped_keys_are_canonical_unambiguous_and_never_alias_default_globals() {
    let key = scoped_selection_key("p::א", Some("q\""), ModelAction::Rewrite);
    assert_eq!(
        parse_scoped_selection_key(&key).unwrap(),
        (None, Some("q\"".into()), ModelAction::Rewrite)
    );
    assert_eq!(
        parse_scoped_selection_key(&scoped_selection_key("p", None, ModelAction::QuickAsk))
            .unwrap(),
        (Some("p".into()), None, ModelAction::QuickAsk)
    );
    for key in [
        "other",
        "model_selection_scope:null",
        "model_selection_scope:[\"p\", null, \"rewrite\"]",
        "model_selection_scope:[\"\",null,\"rewrite\"]",
        "model_selection_scope:[\"default\",null,\"rewrite\"]",
        "model_selection_scope:[\"p\",\"\",\"rewrite\"]",
    ] {
        assert!(parse_scoped_selection_key(key).is_err(), "{key}");
    }
}

#[test]
fn independent_scoped_choices_survive_stale_profile_edits_and_null_means_inherit() {
    let saved = choice(ModelSource::Managed, "groq", "new-model");
    let old = choice(ModelSource::Byok, "openai", "old-model");
    let mut settings = serde_json::Map::new();
    settings.insert("rewrite_program_prompt_profiles".into(), json!([
        {"id":"default","name":"Default","model_selections":{"rewrite":old}},
        {"id":"p","name":"p","stt_provider":"legacy","stt_model":"legacy","llm_provider":"legacy","llm_model":"legacy", "quick_ask_provider":"legacy","quick_ask_model":"legacy","quick_replace_provider":"legacy","quick_replace_model":"legacy","router":{"llm_provider":"legacy","llm_model":"legacy"},
        "model_selections":{"rewrite":old},"presets":[{"id":"q","name":"q","stt_provider":"legacy","stt_model":"legacy","llm_provider":"legacy","llm_model":"legacy","model_selections":{"rewrite":old}}]}
    ]));
    for action in ModelAction::ALL {
        settings.insert(scoped_selection_key("p", None, action), json!(saved));
        settings.insert(scoped_selection_key("p", Some("q"), action), json!(saved));
    }
    let loaded = load_global_selections(&|key| settings.get(key).cloned());
    assert!(loaded.error.is_none());
    assert_eq!(loaded.selections[&ModelAction::Rewrite], old);
    for action in ModelAction::ALL {
        assert_eq!(loaded.profiles[1].model_selections[&action], saved);
        assert_eq!(
            loaded.profiles[1].presets[0].model_selections[&action],
            saved
        );
        settings.insert(scoped_selection_key("p", None, action), json!(null));
        settings.insert(scoped_selection_key("p", Some("q"), action), json!(null));
    }
    settings.insert(ModelAction::Rewrite.settings_key().into(), json!(null));
    let loaded = load_global_selections(&|key| settings.get(key).cloned());
    assert!(loaded.error.is_none());
    assert!(loaded.selections.is_empty());
    assert!(loaded.profiles[0].model_selections.is_empty());
    let p = &loaded.profiles[1];
    assert!(p.model_selections.is_empty());
    assert!(
        p.stt_provider.is_none()
            && p.stt_model.is_none()
            && p.llm_provider.is_none()
            && p.llm_model.is_none()
    );
    assert!(
        p.quick_ask_provider.is_none()
            && p.quick_ask_model.is_none()
            && p.quick_replace_provider.is_none()
            && p.quick_replace_model.is_none()
    );
    assert!(p.router.as_ref().unwrap().llm_provider.is_none());
    let q = &p.presets[0];
    assert!(
        q.model_selections.is_empty()
            && q.stt_provider.is_none()
            && q.stt_model.is_none()
            && q.llm_provider.is_none()
            && q.llm_model.is_none()
    );
    settings.insert(ModelAction::Rewrite.settings_key().into(), json!(saved));
    let loaded = load_global_selections(&|key| settings.get(key).cloned());
    assert_eq!(
        loaded.profiles[0].model_selections[&ModelAction::Rewrite],
        saved
    );
}

#[test]
fn invalid_scoped_choices_fail_closed_before_legacy_or_other_choices_can_run() {
    for preset in [None, Some("q")] {
        for invalid in [
            json!({"source":"bad","provider_id":"secret","model_id":"secret"}),
            json!({"source":"byok","provider_id":"p","model_id":""}),
        ] {
            let key = scoped_selection_key("p", preset, ModelAction::Rewrite);
            let loaded = load_global_selections(&|name| {
                if name == "rewrite_program_prompt_profiles" {
                    Some(json!([{"id":"p","name":"p","presets":[{"id":"q","name":"q"}]}]))
                } else if name == key {
                    Some(invalid.clone())
                } else {
                    None
                }
            });
            assert!(loaded.profiles.is_empty() && loaded.selections.is_empty());
            assert!(!loaded.error.unwrap().contains("secret"));
        }
    }
}

#[test]
fn identifiers_validate_bytes_and_control_characters_without_normalizing_raw_ids() {
    for invalid in [
        choice(ModelSource::Byok, " ", "m"),
        choice(ModelSource::Byok, "p", " "),
        choice(ModelSource::Byok, &"א".repeat(51), "m"),
        choice(ModelSource::Byok, "p", &"א".repeat(129)),
        choice(ModelSource::Byok, "p\u{85}", "m"),
        choice(ModelSource::Byok, "p", "m\n"),
    ] {
        assert!(invalid.validate().is_err());
    }
    let valid = choice(ModelSource::Byok, " p ", " raw/א ");
    assert!(valid.validate().is_ok());
    assert_eq!(valid.model_id, " raw/א ");
}

#[test]
fn linked_presets_share_atomic_choices_and_tombstones_across_profile_metadata() {
    let selected = choice(ModelSource::Byok, "custom_endpoint", "raw-id");
    let key = scoped_selection_key("first", Some("shared"), ModelAction::Rewrite);
    assert_eq!(
        key,
        scoped_selection_key("second", Some("shared"), ModelAction::Rewrite)
    );
    for value in [json!(selected), json!(null)] {
        let loaded = load_global_selections(&|name| {
            if name == "rewrite_program_prompt_profiles" {
                Some(json!([
                    {"id":"first","name":"First","presets":[{"id":"shared","name":"Shared"}]},
                    {"id":"second","name":"Second","presets":[{"id":"shared","name":"Shared"}]}
                ]))
            } else if name == key {
                Some(value.clone())
            } else {
                None
            }
        });
        assert!(loaded.error.is_none());
        for profile in loaded.profiles {
            assert_eq!(
                profile.presets[0]
                    .model_selections
                    .get(&ModelAction::Rewrite),
                (!value.is_null()).then_some(&selected)
            );
        }
    }
}

fn choice(source: ModelSource, provider: &str, model: &str) -> ModelSelection {
    ModelSelection {
        source,
        provider_id: provider.into(),
        model_id: model.into(),
    }
}

#[test]
fn actions_have_independent_stable_settings_keys() {
    let mut keys = std::collections::BTreeSet::new();
    for action in ModelAction::ALL {
        let name = serde_json::to_value(action).unwrap();
        assert_eq!(
            action.settings_key(),
            format!("model_selection_{}", name.as_str().unwrap())
        );
        assert!(keys.insert(action.settings_key()));
        assert_eq!(serde_json::from_value::<ModelAction>(name).unwrap(), action);
    }
    assert_eq!(keys.len(), 7);
}

#[test]
fn absent_or_null_atomic_settings_do_not_seed_or_replace_legacy_choices() {
    for raw in [None, Some(serde_json::Value::Null)] {
        let loaded = load_global_selections(&|_| raw.clone());
        assert!(loaded.selections.is_empty());
        assert!(loaded.profiles.is_empty());
        assert!(loaded.error.is_none());
    }
    assert!(inherit_model_selection(
        None,
        &[(None, Some("legacy-provider"), Some("legacy-model"))]
    )
    .is_none());
}

#[test]
fn saved_choices_and_profile_preset_maps_round_trip_without_rewriting_identifiers() {
    let global = choice(ModelSource::Byok, "custom_endpoint", "vendor/א::raw-id");
    let profile_choice = choice(ModelSource::Managed, "groq", "catalog-id");
    let preset_choice = choice(ModelSource::Byok, "openai", "gpt-5.4");
    let profile = json!({"id":"p", "name":"profile", "program_paths": [], "model_selections":{"rewrite":profile_choice}, "presets":[{"id":"preset","name":"preset","model_selections":{"rewrite":preset_choice}}]});
    let loaded = load_global_selections(&|key| match key {
        "model_selection_transcription" => Some(serde_json::to_value(&global).unwrap()),
        "rewrite_program_prompt_profiles" => Some(json!([profile])),
        _ => None,
    });
    assert!(loaded.error.is_none());
    assert_eq!(
        loaded.selections.get(&ModelAction::Transcription),
        Some(&global)
    );
    assert_eq!(
        loaded.profiles[0].model_selections[&ModelAction::Rewrite],
        profile_choice
    );
    assert_eq!(
        loaded.profiles[0].presets[0].model_selections[&ModelAction::Rewrite],
        preset_choice
    );
    let serialized = serde_json::to_value(&loaded.profiles[0]).unwrap();
    assert_eq!(
        serialized["model_selections"]["rewrite"]["model_id"],
        "catalog-id"
    );
}

#[test]
fn invalid_explicit_choices_block_routing_instead_of_dropping_into_defaults() {
    for invalid in [
        json!({"source":"unknown", "provider_id":"sensitive-provider", "model_id":"sensitive-model"}),
        json!({"source":"byok", "provider_id":"p", "model_id":""}),
        json!({"source":"byok", "provider_id":"p", "model_id":"m", "extra":"sensitive-value"}),
    ] {
        let loaded = load_global_selections(&|key| {
            (key == "model_selection_rewrite").then_some(invalid.clone())
        });
        assert!(loaded.selections.is_empty());
        let error = loaded.error.unwrap();
        assert!(!error.contains("sensitive"));
    }
    for invalid in [
        json!("invalid"),
        json!([{"id":"p","name":"p","model_selections":{"rewrite":{"source":"unknown","provider_id":"p","model_id":"m"}}}]),
        json!([{"id":"p","name":"p","presets":[{"id":"q","name":"q","model_selections":{"rewrite":{"source":"byok","provider_id":"p","model_id":""}}}]}]),
    ] {
        let loaded = load_global_selections(&|key| {
            (key == "rewrite_program_prompt_profiles").then_some(invalid.clone())
        });
        assert!(loaded.error.is_some());
        assert!(loaded.profiles.is_empty());
    }
}

#[test]
fn legacy_nullable_maps_inherit_and_atomic_layers_win_over_stale_legacy_fields() {
    for raw in [
        json!({"id":"p","name":"p"}),
        json!({"id":"p","name":"p","model_selections":null}),
    ] {
        let profile: crate::settings::RewriteProgramPromptProfile =
            serde_json::from_value(raw).unwrap();
        assert!(profile.model_selections.is_empty());
    }
    let base = choice(ModelSource::Byok, "openai", "global-model");
    let profile = choice(ModelSource::Managed, "groq", "profile-model");
    let preset = choice(ModelSource::Byok, "custom_endpoint", "raw-model");
    assert_eq!(
        inherit_model_selection(
            Some(&base),
            &[
                (Some(&profile), Some("stale-provider"), Some("stale-model")),
                (None, Some("groq"), Some("legacy-preset-model")),
            ]
        ),
        Some(choice(ModelSource::Managed, "groq", "legacy-preset-model"))
    );
    assert_eq!(
        inherit_model_selection(
            Some(&base),
            &[
                (Some(&profile), None, None),
                (Some(&preset), Some("stale-provider"), Some("stale-model")),
            ]
        ),
        Some(preset)
    );
    assert_eq!(
        inherit_model_selection(Some(&base), &[(None, Some(" "), Some("\t"))]),
        Some(base)
    );
}
