//! Local STT provider lifecycle decisions.
//!
//! Keep deterministic cache identity, readiness, managed-bypass, and eviction
//! rules here so provider resolution and UI commands do not duplicate Local
//! Whisper-specific behavior.

use std::collections::HashMap;
use std::path::Path;

pub(super) const LOCAL_WHISPER_PROVIDER_ID: &str = "local-whisper";
pub(super) const WHISPER_SERVER_PROVIDER_ID: &str = "whisper-server";
const LOCAL_WHISPER_CACHE_PREFIX: &str = "local-whisper::";
const MISSING_MODEL_PATH_KEY: &str = "<missing-model-path>";
const DISABLED_MODEL_KEY: &str = "<local-whisper-disabled>";
const AUTO_LANGUAGE_KEY: &str = "<auto>";

pub(super) fn local_whisper_model_key_for_cache(
    model_path: Option<&Path>,
    local_whisper_available: bool,
) -> String {
    if !local_whisper_available {
        return DISABLED_MODEL_KEY.to_string();
    }

    model_path
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| MISSING_MODEL_PATH_KEY.to_string())
}

pub(super) fn local_whisper_cache_key_for_language(
    model_key: &str,
    language: Option<&str>,
) -> String {
    let language_key = language
        .map(str::trim)
        .filter(|language| !language.is_empty())
        .unwrap_or(AUTO_LANGUAGE_KEY);
    format!("{LOCAL_WHISPER_CACHE_PREFIX}{model_key}::{language_key}")
}

pub(super) fn is_local_provider(provider_id: &str) -> bool {
    matches!(
        provider_id,
        LOCAL_WHISPER_PROVIDER_ID | WHISPER_SERVER_PROVIDER_ID
    )
}

pub(super) fn bypasses_managed_transport(provider_id: &str) -> bool {
    is_local_provider(provider_id)
}

pub(super) fn is_local_whisper_cache_key(cache_key: &str) -> bool {
    cache_key.starts_with(LOCAL_WHISPER_CACHE_PREFIX)
}

pub(super) fn should_keep_after_local_whisper_unload(cache_key: &str) -> bool {
    !is_local_whisper_cache_key(cache_key)
}

pub(super) fn local_whisper_cache_contains<T>(cache: &HashMap<String, T>, cache_key: &str) -> bool {
    cache.contains_key(cache_key)
}

pub(super) fn retain_after_local_whisper_unload<T>(cache: &mut HashMap<String, T>) {
    // Explicit unload is a local-provider lifecycle operation. Keep the retain
    // predicate here so callers do not need to know the cache-key prefix shape.
    cache.retain(|key, _| should_keep_after_local_whisper_unload(key));
}

#[cfg_attr(not(feature = "local-whisper"), allow(dead_code))]
pub(super) fn insert_loaded_local_whisper<T>(
    cache: &mut HashMap<String, T>,
    cache_key: String,
    provider: T,
) {
    cache.insert(cache_key, provider);
}

#[cfg_attr(not(feature = "local-whisper"), allow(dead_code))]
pub(super) fn insert_loaded_local_whisper_if_absent<T>(
    cache: &mut HashMap<String, T>,
    cache_key: String,
    provider: T,
) {
    // Slow manual loads release the pipeline lock while the model is created.
    // If another path loaded the same identity meanwhile, preserve the first
    // provider so the explicit-load command stays idempotent.
    cache.entry(cache_key).or_insert(provider);
}

pub(super) fn should_evict_local_whisper_cache(
    old_model_key: &str,
    new_model_key: &str,
    old_transcription_prompt: Option<&str>,
    new_transcription_prompt: Option<&str>,
) -> bool {
    old_model_key != new_model_key || old_transcription_prompt != new_transcription_prompt
}

pub(super) fn manual_unloaded_error(
    provider_id: &str,
    load_mode: &str,
    cache_loaded: bool,
) -> Option<&'static str> {
    if provider_id == LOCAL_WHISPER_PROVIDER_ID
        && load_mode.trim().eq_ignore_ascii_case("manual")
        && !cache_loaded
    {
        return Some(
            "Local Whisper is set to Manual load. Click 'Load model' in Settings (or switch load mode to 'On transcribe').",
        );
    }

    None
}

#[cfg_attr(not(feature = "local-whisper"), allow(dead_code))]
pub(super) fn local_whisper_model_unavailable_error(
    provider_id: &str,
    local_whisper_available: bool,
    model_path_present: bool,
) -> Option<&'static str> {
    if provider_id != LOCAL_WHISPER_PROVIDER_ID {
        return None;
    }

    if !local_whisper_available {
        return Some("Local Whisper feature is not enabled");
    }

    if !model_path_present {
        return Some("Local Whisper selected but no model path configured");
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn cache_identity_includes_model_and_language() {
        let model_a = PathBuf::from("C:/models/base.bin");
        let model_b = PathBuf::from("C:/models/small.bin");
        let key_a = local_whisper_model_key_for_cache(Some(&model_a), true);
        let key_b = local_whisper_model_key_for_cache(Some(&model_b), true);

        assert_ne!(key_a, key_b);
        assert_eq!(
            local_whisper_cache_key_for_language(&key_a, Some(" en ")),
            format!("{LOCAL_WHISPER_CACHE_PREFIX}{key_a}::en")
        );
        assert_eq!(
            local_whisper_cache_key_for_language(&key_a, None),
            format!("{LOCAL_WHISPER_CACHE_PREFIX}{key_a}::{AUTO_LANGUAGE_KEY}")
        );
        assert_eq!(
            local_whisper_model_key_for_cache(None, true),
            MISSING_MODEL_PATH_KEY
        );
        assert_eq!(
            local_whisper_model_key_for_cache(Some(&model_a), false),
            DISABLED_MODEL_KEY
        );
    }

    #[test]
    fn loaded_provider_reused_only_for_matching_identity() {
        let model_key = "C:/models/base.bin";
        let loaded_key = local_whisper_cache_key_for_language(model_key, Some("en"));

        assert_eq!(
            loaded_key,
            local_whisper_cache_key_for_language(model_key, Some("en"))
        );
        assert_ne!(
            loaded_key,
            local_whisper_cache_key_for_language(model_key, Some("fr"))
        );
        assert_ne!(
            loaded_key,
            local_whisper_cache_key_for_language("C:/models/small.bin", Some("en"))
        );
    }

    #[test]
    fn manual_mode_requires_loaded_local_whisper() {
        assert!(
            manual_unloaded_error(LOCAL_WHISPER_PROVIDER_ID, "manual", false)
                .expect("manual unloaded should fail")
                .contains("Manual load")
        );
        assert!(manual_unloaded_error(LOCAL_WHISPER_PROVIDER_ID, "manual", true).is_none());
        assert!(manual_unloaded_error(LOCAL_WHISPER_PROVIDER_ID, "on_transcribe", false).is_none());
        assert!(manual_unloaded_error("openai", "manual", false).is_none());
    }

    #[test]
    fn readiness_reports_feature_and_model_path_failures() {
        assert_eq!(
            local_whisper_model_unavailable_error(LOCAL_WHISPER_PROVIDER_ID, false, false),
            Some("Local Whisper feature is not enabled")
        );
        assert_eq!(
            local_whisper_model_unavailable_error(LOCAL_WHISPER_PROVIDER_ID, true, false),
            Some("Local Whisper selected but no model path configured")
        );
        assert_eq!(
            local_whisper_model_unavailable_error(LOCAL_WHISPER_PROVIDER_ID, true, true),
            None
        );
        assert_eq!(
            local_whisper_model_unavailable_error("openai", false, false),
            None
        );
    }

    #[test]
    fn local_providers_bypass_managed_transport() {
        assert!(is_local_provider(LOCAL_WHISPER_PROVIDER_ID));
        assert!(is_local_provider(WHISPER_SERVER_PROVIDER_ID));
        assert!(bypasses_managed_transport(LOCAL_WHISPER_PROVIDER_ID));
        assert!(bypasses_managed_transport(WHISPER_SERVER_PROVIDER_ID));
        assert!(!is_local_provider("openai"));
        assert!(!bypasses_managed_transport("openai"));
    }

    #[test]
    fn unload_and_config_changes_evict_only_local_whisper_cache() {
        assert!(!should_keep_after_local_whisper_unload(
            "local-whisper::model::en"
        ));
        assert!(should_keep_after_local_whisper_unload(
            "openai::whisper-1::en::live=false"
        ));

        assert!(should_evict_local_whisper_cache(
            "old-model",
            "new-model",
            Some("prompt"),
            Some("prompt")
        ));
        assert!(should_evict_local_whisper_cache(
            "model",
            "model",
            Some("old"),
            Some("new")
        ));
        assert!(!should_evict_local_whisper_cache(
            "model",
            "model",
            Some("prompt"),
            Some("prompt")
        ));
    }

    #[test]
    fn cache_controller_helpers_keep_mutation_rules_local() {
        let mut cache = HashMap::from([
            ("local-whisper::model::en".to_string(), 1),
            ("openai::whisper-1::en::live=false".to_string(), 2),
        ]);

        assert!(local_whisper_cache_contains(
            &cache,
            "local-whisper::model::en"
        ));

        retain_after_local_whisper_unload(&mut cache);
        assert!(!cache.contains_key("local-whisper::model::en"));
        assert_eq!(cache.get("openai::whisper-1::en::live=false"), Some(&2));

        insert_loaded_local_whisper(&mut cache, "local-whisper::new::en".to_string(), 3);
        assert_eq!(cache.get("local-whisper::new::en"), Some(&3));

        insert_loaded_local_whisper_if_absent(&mut cache, "local-whisper::new::en".to_string(), 4);
        assert_eq!(cache.get("local-whisper::new::en"), Some(&3));
    }
}
