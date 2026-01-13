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
