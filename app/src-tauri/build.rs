#[path = "src/public_config_validation.rs"]
mod public_config_validation;

fn main() {
    embed_public_cloud_config();
    // Ensure Cargo rebuilds the Windows resources (exe icon) when our icon assets change.
    // Without these, `build.rs` may not rerun and Windows can keep embedding the old icon.
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/32x32.png");
    println!("cargo:rerun-if-changed=icons/icon.png");

    // Guardrail: do not allow release builds with CSP disabled.
    // This is intentionally strict; if a future change needs CSP off, it should be a deliberate
    // code change, not an accidental config regression.
    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile == "release" {
        let conf = std::fs::read_to_string("tauri.conf.json")
            .expect("Failed to read tauri.conf.json for CSP validation");
        let json: serde_json::Value =
            serde_json::from_str(&conf).expect("tauri.conf.json must be valid JSON");

        let csp = json
            .get("app")
            .and_then(|v| v.get("security"))
            .and_then(|v| v.get("csp"));

        let ok = match csp {
            Some(serde_json::Value::String(s)) => !s.trim().is_empty(),
            _ => false,
        };

        if !ok {
            panic!(
                "Refusing to build release with CSP disabled. Fix app.security.csp in tauri.conf.json."
            );
        }

        validate_release_cloudflare_access_env();
    }

    // Windows-specific fix for local/CI Rust tests:
    // Our crate (via Tauri and dialogs) imports comctl32 APIs like `TaskDialogIndirect`.
    // If the test harness exe doesn't have a manifest requesting Common Controls v6,
    // Windows may load the legacy comctl32 v5, which *doesn't* export those symbols,
    // causing an immediate process-start crash: 0xc0000139 (STATUS_ENTRYPOINT_NOT_FOUND).
    //
    // /MANIFESTDEPENDENCY merges into the generated manifest (doesn't replace it), so it
    // is safe to apply to all Windows link steps, including tests.
    #[cfg(target_os = "windows")]
    {
        let common_controls_dep = "type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'";
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:{}",
            common_controls_dep
        );
    }

    tauri_build::build()
}

fn embed_public_cloud_config() {
    let release = std::env::var("PROFILE").as_deref() == Ok("release");
    let offline = env_flag_is_truthy("KOLBOO_ALLOW_MISSING_RELEASE_CLOUD_ENDPOINTS");
    println!("cargo:rerun-if-env-changed=KOLBOO_ALLOW_MISSING_RELEASE_CLOUD_ENDPOINTS");
    println!("cargo:rerun-if-env-changed=TAURI_CLOUD_ENV");
    println!("cargo:rerun-if-env-changed=VITE_CLOUD_SERVICE_ENABLED");
    let values: Vec<_> = public_config_validation::PUBLIC_CLOUD_KEYS
        .iter()
        .map(|key| (*key, std::env::var(key).ok()))
        .collect();
    let mode = std::env::var("TAURI_CLOUD_ENV").unwrap_or_default();
    let service = release && !offline && mode == "production";
    if release && !offline {
        public_config_validation::validate_package(
            &mode,
            &std::env::var("VITE_CLOUD_SERVICE_ENABLED").unwrap_or_default(),
            &values,
        )
        .unwrap_or_else(|error| panic!("{error}"));
    }
    for key in public_config_validation::PUBLIC_CLOUD_KEYS {
        println!("cargo:rerun-if-env-changed={key}");
        // Never read a developer .env while constructing a release installer.
        let value = std::env::var(key)
            .ok()
            .filter(|value| !value.trim().is_empty());
        if let Some(value) = value {
            public_config_validation::validate(key, &value, service)
                .unwrap_or_else(|error| panic!("{error}"));
            println!("cargo:rustc-env={key}={}", value.trim());
        } else if service {
            panic!("Service release is missing public configuration: {key}");
        }
    }
}

fn validate_release_cloudflare_access_env() {
    let forbidden_vars = [
        "TAURI_CLOUDFLARE_ACCESS_CLIENT_ID",
        "TAURI_CLOUDFLARE_ACCESS_CLIENT_SECRET",
    ];
    let present = forbidden_vars
        .iter()
        .copied()
        .filter(|name| release_raw_env_value(name).is_some())
        .collect::<Vec<_>>();

    if !present.is_empty() {
        panic!(
            "Refusing to build release with dev-only Cloudflare Access service-token env vars set: {}. Keep these local to hosted-dev only and remove them from release build env.",
            present.join(", ")
        );
    }
}

fn release_raw_env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .and_then(|value| normalize_nonempty_env_value(&value))
        .or_else(|| env_file_value(name, normalize_nonempty_env_value))
}

fn env_file_value(name: &str, normalize: fn(&str) -> Option<String>) -> Option<String> {
    let env_text = std::fs::read_to_string("../.env").ok()?;
    env_text.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }

        let (key, value) = trimmed.split_once('=')?;
        if key.trim() != name {
            return None;
        }

        normalize(value)
    })
}

fn normalize_nonempty_env_value(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.to_string())
}

fn env_flag_is_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}
