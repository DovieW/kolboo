// Release endpoint and publishable-key validation.
use super::*;

#[test]
fn package_modes_cannot_mix_unavailable_ui_and_cloud_configuration() {
    let values: Vec<_> = PUBLIC_CLOUD_KEYS.iter().map(|key| (*key, None)).collect();
    assert!(validate_package("community", "false", &values).is_ok());
    let blank: Vec<_> = PUBLIC_CLOUD_KEYS
        .iter()
        .map(|key| (*key, Some(" ".into())))
        .collect();
    assert!(validate_package("community", "false", &blank).is_ok());
    for (mode, frontend) in [
        ("community", "true"),
        ("production", "false"),
        ("production", ""),
        ("", "false"),
        ("development", "true"),
    ] {
        assert!(validate_package(mode, frontend, &values).is_err());
    }
    for key in PUBLIC_CLOUD_KEYS {
        let mut inherited = values.clone();
        inherited
            .iter_mut()
            .find(|(name, _)| *name == key)
            .unwrap()
            .1 = Some("DO_NOT_PRINT".into());
        assert_eq!(
            validate_package("community", "false", &inherited),
            Err(format!(
                "Community package must omit public cloud configuration: {key}"
            ))
        );
    }
    assert!(validate_package("production", "true", &values).is_err());
    assert!(validate_package("production", "true", &blank).is_err());
    let production: Vec<_> = PUBLIC_CLOUD_KEYS
        .iter()
        .map(|key| {
            (
                *key,
                Some(
                    if *key == "TAURI_SUPABASE_PUBLISHABLE_KEY" {
                        "sb_publishable_synthetic_public_value"
                    } else {
                        "https://api.example.com"
                    }
                    .into(),
                ),
            )
        })
        .collect();
    assert!(validate_package("production", "true", &production).is_ok());
}

#[test]
fn production_rejects_development_missing_and_credential_bearing_origins() {
    for value in [
        "",
        "https://prod.example\nBAD=1",
        "invalid",
        "file:///tmp/test",
        "https://localhost",
        "http://prod.example",
        "https://kolboo.dovie.dev",
        "https://api.test",
        "https://dev.example",
        "https://dev-api.example",
        "https://user@prod.example",
        "https://user:password@prod.example",
        "https://prod.example?token=secret",
        "https://prod.example#token",
    ] {
        assert_eq!(
            validate("TAURI_API_BASE_URL", value, true),
            Err("Invalid public cloud configuration: TAURI_API_BASE_URL".into())
        );
    }
    assert!(validate("TAURI_API_BASE_URL", "http://localhost:54321", false).is_ok());
    assert!(validate("TAURI_API_BASE_URL", "ftp://prod.example", false).is_err());
    assert!(validate("SERVER_SECRET", "https://prod.example", true).is_err());
    for key in PUBLIC_CLOUD_KEYS
        .into_iter()
        .filter(|key| *key != "TAURI_SUPABASE_PUBLISHABLE_KEY")
    {
        assert!(validate(key, "https://prod.example/path", true).is_ok());
    }
}

#[test]
fn only_publishable_and_legacy_anon_keys_are_allowed_in_installers() {
    let key_name = "TAURI_SUPABASE_PUBLISHABLE_KEY";
    assert!(validate(key_name, "sb_publishable_synthetic_public_value", true).is_ok());
    for key in [
        "sb_publishable_short",
        "sb_secret_synthetic_private_value",
        "abc",
        "a.!invalid.b",
        "a.bm90LWpzb24.b",
        "a.e30.b",
        "a.eyJyb2xlIjoxfQ.b",
    ] {
        assert!(validate(key_name, key, true).is_err());
    }
    for role in ["anon", "service_role", "authenticated"] {
        let payload = URL_SAFE_NO_PAD.encode(format!("{{\"role\":\"{role}\"}}"));
        assert_eq!(
            validate(key_name, &format!("header.{payload}.signature"), true).is_ok(),
            role == "anon"
        );
    }
}
