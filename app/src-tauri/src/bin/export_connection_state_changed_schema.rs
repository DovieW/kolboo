use schemars::schema_for;

fn main() {
    let schema = schema_for!(kolboo_lib::ConnectionStateChangedPayload);
    let serialized = serde_json::to_string_pretty(&schema)
        .expect("Failed to serialize connection-state-changed schema");
    println!("{serialized}");
}
