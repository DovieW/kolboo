use schemars::schema_for;

fn main() {
    let schema = schema_for!(kolboo_lib::ProxySettings);
    let serialized =
        serde_json::to_string_pretty(&schema).expect("Failed to serialize ProxySettings schema");
    println!("{serialized}");
}
