# Third-party notices

Kolboo is a fork of [Tambourine Voice](https://github.com/kstonekuan/tambourine-voice) by Kingston Kuan. Inherited copyright remains with the original contributors. Subsequent contributions remain owned by their contributors; this project does not claim sole copyright ownership of the repository.

Kolboo depends on open-source JavaScript and Rust packages. The exact resolved inventory is recorded in:

- `app/pnpm-lock.yaml` for JavaScript packages;
- `app/src-tauri/Cargo.lock` for Rust crates; and
- source/package manifests for optional and platform-specific dependencies.

Prominent runtime/build ecosystems include Tauri, React, Mantine, TanStack Query, Sentry SDKs, Tokio, Reqwest, and their transitive dependencies. They retain their own copyright and license terms. Package metadata is the authority for the version actually resolved.

The pinned `tauri-nspanel` revision includes `LICENSE_APACHE-2.0` and `LICENSE_MIT` upstream even though its Cargo package metadata omits an SPDX `license` field. Preserve those upstream license files when distributing the macOS-only source dependency.

Bundled Outfit and Sora font packages are distributed under the SIL Open Font License 1.1 by their respective authors. Application icons, screenshots, and Kolboo-specific artwork are not relicensed for standalone use merely because they appear beside AGPL source.

The root `LICENSE` applies to Kolboo-covered source and does not replace a third party’s license. Distributors must preserve applicable notices, provide corresponding source as required by AGPL-3.0, and review the locked dependency inventory for the exact release they ship.

If an attribution is missing or incorrect, report it through the issue tracker or email [licensing@kol.software](mailto:licensing@kol.software).
