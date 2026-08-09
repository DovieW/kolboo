# Faster builds (especially on Windows)

This repo’s `pnpm -C app dev` runs `tauri dev`, which builds both:

- the Vite frontend, and
- the Rust/Tauri backend via Cargo.

If rebuilds are slow, the fastest wins are typically (1) antivirus exclusions and (2) enabling a Rust compiler cache.

## 1) Antivirus / Defender exclusions

If you’re on Windows, excluding these paths from real-time scanning often removes “random long rebuilds”:

- `app/src-tauri/target` (Rust build artifacts)
- `app/node_modules` (JS deps)
- `app/node_modules/.vite` (Vite dep optimizer cache)

## 2) Enable Rust compiler caching with sccache

`sccache` is a compiler cache that can significantly speed up repeated Rust builds.

Upstream docs:
- https://github.com/mozilla/sccache/blob/main/docs/Rust.md

### Install sccache

Use any of the following:

- Scoop: `scoop install sccache`
- Chocolatey: `choco install sccache`
- Cargo: `cargo install sccache`

### Use it for `tauri dev`

Run:

- `pnpm -C app dev:sccache`

This uses `RUSTC_WRAPPER=sccache` for the `tauri dev` process.

### Verify it’s working

After a build, run:

- `sccache --show-stats`

You should see cache hits/misses; hits will increase after the first build.

### Optional knobs

You can optionally set (system/user env vars):

- `SCCACHE_DIR` (put it on a fast SSD)
- `SCCACHE_CACHE_SIZE` (example: `20G`)

## 3) More parallelism (compile phase)

Cargo compiles crates in parallel by default, but you can force the job count by setting:

- `CARGO_BUILD_JOBS` (e.g. to your logical core count)

Note: linking on Windows can still dominate rebuild time and doesn’t scale as well with cores.

## 4) Reclaim Cargo target disk space safely

Use `pnpm -C app clean:rust-cache` to preview the exact known Cargo target directories and their sizes. Use `pnpm -C app clean:rust-cache:apply` only when you are ready to discard those rebuildable outputs. See [Windows v1 release operations](RELEASE_OPERATIONS.md#cargo-cache-disk-usage) for the deletion boundary and rebuild impact.
