use schemars::{schema_for, JsonSchema};

pub fn print_schema<T: JsonSchema>(label: &str) {
    let schema = schema_for!(T);
    let serialized = serde_json::to_string_pretty(&schema).unwrap_or_else(|err| {
        panic!("Failed to serialize {label} schema: {err}");
    });
    println!("{serialized}");
}

#[macro_export]
macro_rules! schema_export_bin {
    ($ty:ty, $label:expr) => {
        fn main() {
            $crate::schema_export::print_schema::<$ty>($label);
        }
    };
}
