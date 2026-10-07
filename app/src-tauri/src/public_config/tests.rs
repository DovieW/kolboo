// Public build/runtime configuration contracts.
use super::*;

#[test]
fn installed_apps_use_packaged_values_even_when_environment_is_set() {
    assert_eq!(
        resolve(
            &["api"],
            false,
            &|_| panic!("release read runtime env"),
            &|_| Some(" https://prod.example/ ".into())
        ),
        Some("https://prod.example/".into())
    );
}

#[test]
fn development_overrides_are_nonempty_and_aliases_preserve_priority() {
    assert_eq!(
        resolve(
            &["empty", "api"],
            true,
            &|key| Some(
                if key == "empty" {
                    " "
                } else {
                    "https://dev.example"
                }
                .into()
            ),
            &|_| panic!("unnecessary packaged lookup")
        ),
        Some("https://dev.example".into())
    );
    assert_eq!(
        resolve(
            &["missing", "empty", "api"],
            true,
            &|_| None,
            &|key| match key {
                "empty" => Some(" ".into()),
                "api" => Some(" https://prod.example ".into()),
                _ => None,
            }
        ),
        Some("https://prod.example".into())
    );
    assert_eq!(resolve(&[], true, &|_| None, &|_| None), None);
    assert_eq!(resolve(&["api"], false, &|_| None, &|_| None), None);
}

#[test]
fn public_key_allowlist_never_reads_a_server_secret() {
    for key in [
        "TAURI_API_BASE_URL",
        "TAURI_MANAGED_INFERENCE_GATEWAY_URL",
        "TAURI_SUPABASE_URL",
        "TAURI_SUPABASE_PUBLISHABLE_KEY",
        "TAURI_PUBLIC_AUTH_PAGE_URL",
        "SUPABASE_SECRET_KEY",
    ] {
        let expected = match key {
            "TAURI_API_BASE_URL" => option_env!("TAURI_API_BASE_URL"),
            "TAURI_MANAGED_INFERENCE_GATEWAY_URL" => {
                option_env!("TAURI_MANAGED_INFERENCE_GATEWAY_URL")
            }
            "TAURI_SUPABASE_URL" => option_env!("TAURI_SUPABASE_URL"),
            "TAURI_SUPABASE_PUBLISHABLE_KEY" => option_env!("TAURI_SUPABASE_PUBLISHABLE_KEY"),
            "TAURI_PUBLIC_AUTH_PAGE_URL" => option_env!("TAURI_PUBLIC_AUTH_PAGE_URL"),
            _ => None,
        };
        assert_eq!(compiled(key).as_deref(), expected);
        assert_eq!(
            read(&[key]),
            resolve(
                &[key],
                cfg!(debug_assertions),
                &|name| std::env::var(name).ok(),
                &compiled
            )
        );
    }
}
