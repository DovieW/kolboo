use schemars::schema_for;

fn main() {
    let schema = schema_for!(kolboo_lib::WhisperModelInfo);
    let serialized =
        serde_json::to_string_pretty(&schema).expect("Failed to serialize WhisperModelInfo schema");
    println!("{serialized}");
}
