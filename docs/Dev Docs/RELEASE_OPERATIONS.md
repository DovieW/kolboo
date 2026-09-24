# Release operations

> **Channel scope:** Stable Windows distribution remains gated. Linux may publish a clearly labeled Community/BYOK prerelease after exact-package native acceptance; this does not open managed signup or constitute a broad product launch.

Windows remains the stable release target. Linux has a separate x86_64 Community/BYOK beta channel with manual updates and explicit native acceptance. macOS remains development-only.

Linux beta tags use `vX.Y.Z-beta.N` and are handled only by `Linux Community Beta Release`; stable Windows release jobs exclude those tags. See [Linux development and beta releases](../How%20Tos/LINUX_DEVELOPMENT.md) for package verification, acceptance, installation, and rollback.

## Release gates

A stable Windows release tag is allowed only after all of these are true:

- the repository is public and the unauthenticated GitHub release endpoint works;
- `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` are configured;
- the legal pages are publicly hosted and their links have been checked;
- the desktop-to-operator and support rehearsal is complete for the exact release commit.

Windows publisher signing is optional. If both `WINDOWS_CERTIFICATE` (base64-encoded publisher `.pfx`) and `WINDOWS_CERTIFICATE_PASSWORD` are configured, the workflow imports the certificate into the ephemeral Windows runner, configures SHA-256 Authenticode with a timestamp, and rejects any collected `.exe` or `.msi` whose signature is not valid. If neither is configured, it builds unsigned Windows installers and labels the GitHub release accordingly; Windows may show an Unknown publisher warning. If only one credential is configured, the release fails rather than silently producing an unsigned installer. Ordinary branch and local development builds use `--no-sign` and remain available.

## Signed updates

Release builds opt into `VITE_SIGNED_UPDATER_ENABLED=true`. Tauri creates updater signatures with the private updater key, while the application contains only `app/src-tauri/updater.pubkey`. The release workflow refuses to create `latest.json` without a signed Windows artifact and publishes the manifest with the installer.

Updater checks stay disabled in ordinary builds and in the manual-update Linux beta channel. Enable them only for a release channel that has completed signed update and rollback acceptance. Never rotate or lose the updater private key without a migration plan: installed clients trust its committed public counterpart.

## Cut and verify a release

1. Run `pnpm -C app check:ci`, `pnpm -C app coverage`, and `pnpm -C app audit`.
2. Confirm package, Tauri, and Cargo versions match the intended `vX.Y.Z` tag.
3. Push the tag and inspect the Release workflow. Missing updater-key credentials are a launch blocker. Check whether the Windows publisher certificate is present; the workflow signs and verifies installers when it is, and clearly discloses unsigned installers when it is not.
4. Download the release without authentication on a clean Windows machine.
5. If publisher credentials were configured, verify Authenticode in PowerShell with `Get-AuthenticodeSignature <installer>`. Otherwise confirm the release title and notes disclose that the Windows installers are unsigned.
6. Install, launch, check for updates, and confirm that altered or updater-unsigned artifacts are rejected.
7. Record the workflow run, commit SHA, installer hash, updater result, request IDs, and support-safe correlation hashes in the launch evidence.

## Rollback

Do not overwrite a published tag. Mark the affected release as withdrawn, preserve its hashes and incident record, fix forward with a higher version, and publish a new updater-signed release. Existing clients only accept metadata and artifacts signed by the updater key; publisher Authenticode signing remains optional.

## Cargo cache disk usage

Rust builds can occupy tens of gigabytes across `app/src-tauri/target`, `target-ci`, and `target-cli`; on 2026-08-09 the local default target alone occupied about 22 GiB. Preview the exact reclaimable paths and sizes:

```text
pnpm -C app clean:rust-cache
```

Then explicitly remove only those validated directories:

```text
pnpm -C app clean:rust-cache:apply
```

This deletes rebuildable Cargo outputs, not source, configuration, lockfiles, credentials, or JavaScript dependencies. The next Rust build will be slower and will recreate the relevant target directory.
