use schemars::schema_for;
use std::fs;
use std::path::PathBuf;

fn main() {
    let schema = schema_for!(kolboo_lib::RequestLog);
    let json = serde_json::to_string_pretty(&schema)
        .expect("Failed to serialize generated RequestLog schema as JSON");

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("gen");
    path.push("schemas");
    fs::create_dir_all(&path).expect("Failed to create gen/schemas directory");
    path.push("request-log.schema.json");

    fs::write(&path, format!("{json}\n"))
        .unwrap_or_else(|_| panic!("Failed to write schema file: {}", path.display()));

    println!("Wrote {}", path.display());
}
