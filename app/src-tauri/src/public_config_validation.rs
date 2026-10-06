//! Shared by the build script and deterministic tests. Error strings name the
//! setting, never its value (publishable configuration can still be sensitive).
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

pub const PUBLIC_CLOUD_KEYS: [&str; 5] = [
    "TAURI_API_BASE_URL",
    "TAURI_MANAGED_INFERENCE_GATEWAY_URL",
    "TAURI_SUPABASE_URL",
    "TAURI_SUPABASE_PUBLISHABLE_KEY",
    "TAURI_PUBLIC_AUTH_PAGE_URL",
];

/// A Community installer must omit every service origin/key, including values
/// accidentally inherited from a developer's environment. Service installers
/// retain the strict production validation and must enable the matching UI.
pub fn validate_package(
    mode: &str,
    frontend_enabled: &str,
    values: &[(&str, Option<String>)],
) -> Result<(), String> {
    match (mode, frontend_enabled) {
        ("community", "false") => {
            for (key, value) in values {
                if value.as_ref().is_some_and(|value| !value.trim().is_empty()) {
                    return Err(format!(
                        "Community package must omit public cloud configuration: {key}"
                    ));
                }
            }
        }
        ("production", "true") => {
            for (key, value) in values {
                let value = value.as_deref().ok_or_else(|| {
                    format!("Service release is missing public configuration: {key}")
                })?;
                validate(key, value, true)?;
            }
        }
        _ => {
            return Err(
                "Explicit matching TAURI_CLOUD_ENV and VITE_CLOUD_SERVICE_ENABLED are required"
                    .into(),
            )
        }
    }
    Ok(())
}

pub fn validate(key: &str, value: &str, production: bool) -> Result<(), String> {
    let invalid = || format!("Invalid public cloud configuration: {key}");
    if value.trim().is_empty() || value.contains(['\r', '\n']) {
        return Err(invalid());
    }
    if key == "TAURI_SUPABASE_PUBLISHABLE_KEY" {
        if value.starts_with("sb_publishable_") && value.len() > 20 {
            return Ok(());
        }
        let role = value
            .split('.')
            .nth(1)
            .and_then(|payload| URL_SAFE_NO_PAD.decode(payload).ok())
            .and_then(|payload| serde_json::from_slice::<serde_json::Value>(&payload).ok())
            .and_then(|payload| {
                payload
                    .get("role")
                    .and_then(|role| role.as_str())
                    .map(str::to_string)
            });
        return if role.as_deref() == Some("anon") {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    if !PUBLIC_CLOUD_KEYS.contains(&key) {
        return Err(invalid());
    }
    let url = url::Url::parse(value).map_err(|_| invalid())?;
    let host = url.host_str().ok_or_else(invalid)?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    if production {
        if url.scheme() != "https"
            || host == "localhost"
            || host == "kolboo.dovie.dev"
            || host.ends_with(".test")
            || host.starts_with("dev.")
            || host.starts_with("dev-")
        {
            return Err(invalid());
        }
    } else if !matches!(url.scheme(), "https" | "http") {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
#[path = "public_config_validation/tests.rs"]
mod tests;
