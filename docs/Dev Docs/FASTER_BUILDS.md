# Faster Rust/Tauri builds

This repo’s `pnpm -C app dev` runs `tauri dev`, which builds both:

- the Vite frontend, and
- the Rust/Tauri backend via Cargo.

Kolboo treats compiler caching as part of the development environment rather
than an optional command variant. Run this after provisioning a machine:

```sh
pnpm -C app setup:check
```

Normal `pnpm -C app dev`, Cargo validation scripts, and the fast Linux package
path automatically enable the available accelerators.

Coverage is also a required part of the development toolchain:

```sh
cargo +stable install cargo-llvm-cov --locked --version 0.9.1
rustup component add llvm-tools-preview
pnpm -C app setup:check
```

CI uses the corresponding verified prebuilt release. Keeping the version pinned
makes local and CI coverage semantics consistent.

## 1) Antivirus / Defender exclusions

If you’re on Windows, excluding these paths from real-time scanning often removes “random long rebuilds”:

- `app/src-tauri/target` (Rust build artifacts)
- `app/node_modules` (JS deps)
- `app/node_modules/.vite` (Vite dep optimizer cache)

## 2) Required Rust compiler caching with sccache

`sccache` is a compiler cache that can significantly speed up repeated Rust builds.

Upstream docs:
- https://github.com/mozilla/sccache/blob/main/docs/Rust.md

### Install sccache

Install it with the platform package manager:

- Ubuntu/Kubuntu: `sudo apt-get install sccache`
- macOS: `brew install sccache`
- Scoop: `scoop install sccache`
- Chocolatey: `choco install sccache`

`pnpm dev:sccache` remains as a compatibility alias for `pnpm dev`; a separate
launch command is no longer necessary.

After a build, inspect effectiveness with:

```sh
sccache --show-stats
```

The first compilation populates the cache. Hits matter most after target cleanup,
branch switches, or another build with the same inputs. The shared launcher sets
`CARGO_INCREMENTAL=0` because sccache cannot cache incrementally compiled Rust
crates. This also prevents Cargo's incremental directory from growing into the
largest part of the development target. Set `CARGO_INCREMENTAL=1` explicitly only
for a short, local comparison; it trades away reusable compiler-cache hits.

## 3) Required fast Linux linking with mold

Linux development and CI install `mold`:

```sh
sudo apt-get install mold
pnpm -C app setup:check
```

Package scripts add `-C link-arg=-fuse-ld=mold` without changing release
optimization. Set `KOLBOO_DISABLE_MOLD=1` only while diagnosing a linker-specific
problem.

### Optional knobs

You can optionally set (system/user env vars):

- `SCCACHE_DIR` (put it on a fast SSD)
- `SCCACHE_CACHE_SIZE` (example: `20G`)

## 4) Development profile and parallelism

The shared launcher reserves two logical CPUs and caps Cargo at eight jobs unless
`CARGO_BUILD_JOBS` is explicitly set. Full debug information is disabled for the
large dependency graph, while latency-sensitive audio DSP crates retain
optimization. This keeps useful line-level diagnostics without optimizing every
Tauri dependency during a clean development build.

## 5) Fast installable Linux test package

Build a non-release Debian package with embedded frontend assets:

```sh
pnpm -C app build:linux:fast
```

Build and copy it to the standard IdeaPad test host:

```sh
pnpm -C app deploy:ideapad
```

For an interactive build, copy, and replacement of the installed package:

```sh
pnpm -C app deploy:ideapad:install
```

The final command prompts for sudo on the remote host. This debug-profile package
is for rapid native testing only; official releases still use the optimized,
signed release workflow.

## 6) Reclaim Cargo target disk space safely

Use `pnpm -C app clean:rust-cache` to preview the exact known Cargo target directories and their sizes. Use `pnpm -C app clean:rust-cache:apply` only when you are ready to discard those rebuildable outputs. See [Windows v1 release operations](RELEASE_OPERATIONS.md#cargo-cache-disk-usage) for the deletion boundary and rebuild impact.
