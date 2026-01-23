use anyhow::Context;
use std::path::{Path, PathBuf};

use crate::schema_registry::SCHEMAS;

pub(crate) fn run(_args: Vec<String>) -> anyhow::Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("xtask should live under src-tauri/xtask")?
        .to_path_buf();

    let args = _args;
    let mut out_dir = root.join("gen").join("schemas");

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out-dir" | "-o" => {
                let value = args.get(i + 1).context("--out-dir expects a path")?.clone();
                let value = PathBuf::from(value);
                out_dir = if value.is_absolute() {
                    value
                } else {
                    root.join(value)
                };
                i += 2;
            }
            "-h" | "--help" => {
                eprintln!("Usage: cargo run -p xtask -- schemas [--out-dir <path>]\n");
                return Ok(());
            }
            other => anyhow::bail!("Unknown argument for `schemas`: {other}"),
        }
    }

    let generated = write_all_schemas(&out_dir)?;
    println!("Generated {generated} schemas into {}", out_dir.display());
    Ok(())
}

fn write_all_schemas(out_dir: &Path) -> anyhow::Result<usize> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("Failed to create output dir {out_dir:?}"))?;

    for spec in SCHEMAS {
        let schema = (spec.generator)();
        let json = serde_json::to_string_pretty(&schema)
            .with_context(|| format!("Failed to serialize schema for {}", spec.label))?;
        let normalized = format!("{}\n", json.replace("\r\n", "\n").trim_end());
        let output_path = out_dir.join(spec.out_file);
        std::fs::write(&output_path, normalized)
            .with_context(|| format!("Failed to write {output_path:?}"))?;
    }

    Ok(SCHEMAS.len())
}
