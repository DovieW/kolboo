fn main() {
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
        let json: serde_json::Value = serde_json::from_str(&conf)
            .expect("tauri.conf.json must be valid JSON");

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
    }

    tauri_build::build()
}
