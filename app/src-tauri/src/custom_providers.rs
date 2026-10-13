//! User-owned OpenAI-compatible endpoints. Only non-secret metadata lives here.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CustomProvider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub stt_models: Vec<String>,
    pub llm_models: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stt_model_names: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub llm_model_names: BTreeMap<String, String>,
}

impl CustomProvider {
    pub fn validate(mut self) -> Result<Self, String> {
        if !self.id.starts_with("custom_")
            || self.id.len() > 80
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
        {
            return Err("Invalid custom provider ID".into());
        }
        self.name = self.name.trim().to_string();
        if self.name.is_empty() || self.name.len() > 100 {
            return Err("Enter a provider name (up to 100 characters)".into());
        }
        if self.base_url.len() > 2048 {
            return Err("Base URL is too long".into());
        }
        let url =
            reqwest::Url::parse(self.base_url.trim()).map_err(|_| "Enter a valid base URL")?;
        if !matches!(url.scheme(), "https" | "http")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(
                "Use an HTTP(S) base URL without credentials, query parameters or fragments".into(),
            );
        }
        if url.scheme() == "http" {
            let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
            let local = match host.parse::<std::net::IpAddr>() {
                Ok(std::net::IpAddr::V4(ip)) => {
                    ip.is_loopback() || ip.is_private() || ip.is_link_local()
                }
                Ok(std::net::IpAddr::V6(ip)) => {
                    ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
                }
                Err(_) => host == "localhost",
            };
            if !local {
                return Err(
                    "Use HTTPS for remote providers; HTTP is only supported for local IP addresses"
                        .into(),
                );
            }
        }
        self.base_url = url.as_str().trim_end_matches('/').to_string();
        for models in [&mut self.stt_models, &mut self.llm_models] {
            if models.len() > 100 {
                return Err("Use at most 100 models per capability".into());
            }
            for model in models.iter_mut() {
                *model = model.trim().to_string();
                if model.is_empty() || model.len() > 200 || model.chars().any(char::is_control) {
                    return Err("Enter valid model IDs".into());
                }
            }
            let mut seen = std::collections::HashSet::new();
            models.retain(|m| seen.insert(m.clone()));
        }
        if self.stt_models.is_empty() && self.llm_models.is_empty() {
            return Err("Add at least one model".into());
        }
        for (models, names) in [
            (&self.stt_models, &mut self.stt_model_names),
            (&self.llm_models, &mut self.llm_model_names),
        ] {
            if names.keys().any(|id| !models.contains(id)) {
                return Err("Model names must belong to configured model IDs".into());
            }
            for name in names.values_mut() {
                *name = name.trim().to_string();
                if name.chars().count() > 100 || name.chars().any(char::is_control) {
                    return Err(
                        "Use display names up to 100 characters without control characters".into(),
                    );
                }
            }
            names.retain(|_, name| !name.is_empty());
        }
        Ok(self)
    }
    pub fn key_name(&self) -> String {
        format!("{}_api_key", self.id)
    }
}

#[cfg(desktop)]
pub fn load(app: &tauri::AppHandle) -> Vec<CustomProvider> {
    load_checked(app).unwrap_or_default()
}

#[cfg(desktop)]
pub fn load_checked(app: &tauri::AppHandle) -> Result<Vec<CustomProvider>, String> {
    use tauri_plugin_store::StoreExt;
    let store = app
        .store("settings.json")
        .map_err(|_| "Could not open custom providers")?;
    let Some(value) = store.get("custom_providers") else {
        return Ok(Vec::new());
    };
    let providers: Vec<CustomProvider> =
        serde_json::from_value(value).map_err(|_| "Saved custom providers could not be read")?;
    providers
        .into_iter()
        .map(CustomProvider::validate)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn provider(url: &str) -> CustomProvider {
        CustomProvider {
            id: "custom_test".into(),
            name: "Test".into(),
            base_url: url.into(),
            stt_models: vec![],
            llm_models: vec!["model".into()],
            stt_model_names: BTreeMap::new(),
            llm_model_names: BTreeMap::new(),
        }
    }
    #[test]
    fn rejects_secret_bearing_and_non_http_urls() {
        for url in [
            "https://user:secret@example.com/v1",
            "https://example.com/v1?key=secret",
            "https://example.com/#secret",
            "file:///tmp/test",
        ] {
            assert!(provider(url).validate().is_err());
        }
    }
    #[test]
    fn preserves_custom_prefix_and_normalizes_metadata() {
        let p = provider("http://localhost:8080/v1/").validate().unwrap();
        assert_eq!(p.base_url, "http://localhost:8080/v1");
        assert_eq!(p.key_name(), "custom_test_api_key");
        let mut invalid = p;
        invalid.id = "openai".into();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn legacy_metadata_and_names_keep_wire_identifiers_unchanged() {
        let legacy = serde_json::to_value(provider("https://example.test/v1")).unwrap();
        assert!(legacy.get("llm_model_names").is_none());
        let mut p: CustomProvider = serde_json::from_value(legacy).unwrap();
        p.stt_models = vec!["speech".into()];
        p.stt_model_names
            .insert("speech".into(), "  My speech model  ".into());
        p.llm_model_names.insert("model".into(), " ".into());
        let normalized = p.validate().unwrap();
        assert_eq!(normalized.stt_models, ["speech"]);
        assert_eq!(normalized.llm_models, ["model"]);
        assert_eq!(normalized.stt_model_names["speech"], "My speech model");
        assert!(normalized.llm_model_names.is_empty());
        let mut unicode_name = normalized.clone();
        unicode_name
            .llm_model_names
            .insert("model".into(), "א".repeat(100));
        assert_eq!(
            unicode_name.validate().unwrap().llm_model_names["model"],
            "א".repeat(100)
        );
        let mut unknown = normalized.clone();
        unknown
            .llm_model_names
            .insert("other".into(), "Other".into());
        assert!(unknown
            .validate()
            .unwrap_err()
            .contains("configured model IDs"));
        for bad in ["x".repeat(101), "unsafe\nname".into()] {
            let mut invalid = normalized.clone();
            invalid.stt_model_names.insert("speech".into(), bad);
            assert!(invalid.validate().is_err());
        }
    }
}
