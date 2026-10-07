//! Client-safe cloud configuration. Packaged apps must not depend on their cwd
//! or inherit authentication endpoints from an unrelated process environment.

pub(crate) fn read(keys: &[&str]) -> Option<String> {
    resolve(
        keys,
        cfg!(debug_assertions),
        &|key| std::env::var(key).ok(),
        &compiled,
    )
}

fn compiled(key: &str) -> Option<String> {
    match key {
        "TAURI_API_BASE_URL" => option_env!("TAURI_API_BASE_URL"),
        "TAURI_MANAGED_INFERENCE_GATEWAY_URL" => option_env!("TAURI_MANAGED_INFERENCE_GATEWAY_URL"),
        "TAURI_SUPABASE_URL" => option_env!("TAURI_SUPABASE_URL"),
        "TAURI_SUPABASE_PUBLISHABLE_KEY" => option_env!("TAURI_SUPABASE_PUBLISHABLE_KEY"),
        "TAURI_PUBLIC_AUTH_PAGE_URL" => option_env!("TAURI_PUBLIC_AUTH_PAGE_URL"),
        _ => None,
    }
    .map(str::to_string)
}

fn resolve(
    keys: &[&str],
    development: bool,
    runtime: &dyn Fn(&str) -> Option<String>,
    packaged: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let normalize = |value: String| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    };
    let runtime_value = development
        .then(|| keys.iter().find_map(|key| runtime(key).and_then(normalize)))
        .flatten();
    runtime_value.or_else(|| {
        keys.iter()
            .find_map(|key| packaged(key).and_then(normalize))
    })
}

#[cfg(test)]
#[path = "public_config/tests.rs"]
mod tests;
