mod schema_registry;
mod schemas;

use anyhow::Context;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        print_usage();
        return Ok(());
    };

    match cmd.as_str() {
        "schemas" => schemas::run(args.collect()).context("xtask schemas failed"),
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        other => anyhow::bail!("Unknown xtask command: {other}"),
    }
}

fn print_usage() {
    eprintln!("Usage: cargo run -p xtask -- <command>\n\nCommands:\n  schemas   Generate JSON Schemas into src-tauri/gen/schemas\n");
}
