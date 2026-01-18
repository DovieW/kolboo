use schemars::schema_for;

fn main() {
    let schema = schema_for!(kolboo_lib::PipelineErrorPayload);
    let serialized = serde_json::to_string_pretty(&schema)
        .expect("Failed to serialize PipelineErrorPayload schema");
    println!("{serialized}");
}
