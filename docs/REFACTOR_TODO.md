# Refactor / maintenance TODOs

Small, self-contained improvements that are valuable but out-of-scope for the current ticket.

## Tooling / CI

- Investigate Windows CI/local failures for Rust tests that look environment-related (paging file / mmap / "found staticlib instead of rlib" errors during `cargo test` under `src-tauri/target-ci`). Consider documenting required Windows settings or adjusting the Rust build/test setup to be less memory-hungry.

- Consider speeding up `pnpm -C app schemas:check` (schema generation took ~18 minutes locally) — likely caching or a more incremental check.
