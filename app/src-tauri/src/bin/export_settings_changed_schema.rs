use schemars::schema_for;

fn main() {
    let schema = schema_for!(kolboo_lib::SettingsChangedPayload);
    let serialized =
        serde_json::to_string_pretty(&schema).expect("Failed to serialize settings-changed schema");
    println!("{serialized}");
}
