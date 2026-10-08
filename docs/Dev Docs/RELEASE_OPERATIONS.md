# Release operations

> **Channel scope:** Stable Windows distribution remains gated. Versioned desktop releases also include explicitly experimental, unnotarized universal macOS downloads and Linux Community/BYOK packages. Signed automatic updates are configured for versioned releases on all three platforms; Linux-only prereleases retain manual updates. Publication still requires the applicable exact-package acceptance or an explicitly recorded deferral; none of these channels opens managed signup or constitutes a broad product launch.

Windows remains the stable release target. Linux has a separate x86_64 Community/BYOK beta channel with manual updates and explicit native acceptance. macOS is an experimental download, not yet a supported or native-accepted platform.

Linux-only beta tags use `vX.Y.Z-beta.N` and are handled only by `Linux Community Beta Release`; stable Windows release jobs exclude those tags. Versioned desktop releases include updater-signed Linux Community/BYOK packages with the same exact-package acceptance requirements. See [Linux development and beta releases](../How%20Tos/LINUX_DEVELOPMENT.md) for package verification, acceptance, installation, and rollback.

## Release gates

A stable Windows release tag is allowed only after all of these are true:

- the repository is public and the unauthenticated GitHub release endpoint works;
- `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` are configured;
- the legal pages are publicly hosted and their links have been checked;
- the desktop-to-operator and support rehearsal is complete for the exact release commit.

Windows publisher signing is optional. If both `WINDOWS_CERTIFICATE` (base64-encoded publisher `.pfx`) and `WINDOWS_CERTIFICATE_PASSWORD` are configured, the workflow imports the certificate into the ephemeral Windows runner, configures SHA-256 Authenticode with a timestamp, and rejects any collected `.exe` or `.msi` whose signature is not valid. If neither is configured, it builds unsigned Windows installers and labels the GitHub release accordingly; Windows may show an Unknown publisher warning. If only one credential is configured, the release fails rather than silently producing an unsigned installer. Ordinary branch and local development builds use `--no-sign` and remain available.

The release builds only the standard Windows bundle. The extra `local-whisper` Windows variant is disabled by default, including for manual Windows builds; it remains available as an explicit manual opt-in while the app feature is retained.

Windows checks must pass before installer packaging starts. A failing check therefore prevents an expensive installer build rather than leaving two independent jobs running. The macOS workflow finds Cargo's hashed architecture-specific dSYMs by UUID and verifies them against the finished executable before upload; missing or unusable release symbols still fail the build.

The release also builds a universal macOS DMG, app ZIP and updater-signed app archive with ad-hoc code signing. The workflow verifies the app bundle's code signature and both CPU architectures; it does **not** claim Apple Developer ID signing, notarization, or native Mac acceptance. The release notes disclose these limits and possible Gatekeeper warnings. Do not call this a supported Mac release until the native acceptance pass in [macOS development](../How%20Tos/MACOS_DEVELOPMENT.md) is complete.

## 0.3.1 maintenance-release deferrals

The maintainer explicitly deferred public legal-link verification, the full
Windows/operator rehearsal and Windows/macOS native acceptance for 0.3.1 only.
All three platform downloads must still be built automatically. Automated CI,
updater signing and exact-package Linux acceptance remain required; the deferred
checks must not be described as passed. Managed-service launch remains deferred.

The IdeaPad's unexplained `SIGKILL` on October 1 is a known open reliability
investigation. The maintainer chose to defer that investigation rather than
block this maintenance release. No cause or resolution has been established.

## 0.3.2 Community/BYOK packaging

The maintainer explicitly deferred backend/signup launch for 0.3.2. The Windows,
Linux and Mac workflows build Community/BYOK packages with
`TAURI_CLOUD_ENV=community` and `VITE_CLOUD_SERVICE_ENABLED=false`. Node preflight
and Rust build validation reject any inherited public account-service origins or
Supabase key. This is an explicit channel, not the internal missing-endpoint
bypass. Account signup, managed models and cloud sync remain unavailable; cached
sessions are not erased. Sentry and disclosed analytics remain configured.

On October 6 the maintainer also carried forward the public legal-link and full
Windows/operator rehearsal deferrals for this Community-only 0.3.2 release.
These checks are deferred, not passed. Automated CI, updater signatures and
exact-package Linux acceptance remain required. Managed-service launch stays
disabled. Windows native acceptance is separately waived as recorded below;
macOS remains an explicitly untested experimental download.

Local preparation validation on October 6 passed `check:ci` (925 frontend tests,
977 ordinary Rust tests, 14 explicitly opt-in Rust fixtures), `coverage:patch`
(100% of 784 changed executable lines, only the two approved counterless Tauri
email-command metadata exceptions), isolated Linux/Wry startup/layout/close
acceptance, both dependency audit gates, version consistency and 13 release
configuration/signing-helper tests. These are source/fixture results, not
acceptance of a release package. The signed preparation commit and its initial
PR checks passed; PR #108 remains open. Its account-session review fix also
passed local coverage, including stalled-lookup recovery and concurrent logout.
Release packages and exact-package Linux acceptance remain pending; do not
describe 0.3.2 as published.

To enable service packages in a later release, configure the five
`KOLBOO_PROD_*` public GitHub Actions variables documented in Authentication
Architecture and switch both mode flags together. Production validation remains
strict. Live service acceptance is still required before opening signup.

Release titles follow the simple `vX.Y.Z` convention. Unsigned Windows publisher
identity and experimental macOS limitations are disclosed in the release body,
not appended to its title.

## Windows startup regression (0.3.2)

The overlay layout helper previously acquired its layout mutex before resolving
the target monitor. Wry monitor getters synchronously wait for the GUI thread;
GUI-thread startup also applies the initial layout. Concurrent renderer IPC
could therefore deadlock startup before tray creation. Monitor/DPI/work-area
reads now happen before the layout mutex. Size/position writes and the rectangle
cache remain serialized; do not add synchronous native getters inside that lock.

The isolated Linux/Wry fixture overlaps worker and GUI layouts for 16 rounds,
checks that the worker cannot own the mutex while waiting for monitor lookup,
and checks the resulting compact geometry. This is not Windows acceptance.
Before declaring the Windows failure resolved, test the patched Windows package
with JavaScript enabled, both fresh and existing completed-setup settings,
repeated launches, tray activation and a second-instance launch. Do not delete a
user's settings or persist browser diagnostic flags to make this check pass.

The regression test was also run with the old getter-under-lock ordering and
failed at the mutex/GUI-wait assertion. Restoring the fix passed the same test.

On October 6 the maintainer explicitly waived patched Windows package/native
testing for 0.3.2. Record it as untested, not passed. This does not waive production
signup configuration, service rollout validation or the remaining release gates.

## Signed updates

Versioned Windows, Linux and Mac release builds opt into `VITE_SIGNED_UPDATER_ENABLED=true`. Rust checks
at startup and every six hours, including while the main webview is closed to
the tray. It downloads and verifies newer releases in the background; a failed
check/download does not block Community/BYOK use. Checks/downloads are bounded
by a two-minute network timeout and cancelled on shutdown. No update can
interrupt recording, transcription or other work on an idle timer: installation
is deferred until explicit **Quit** (tray Quit or an exit-program close action),
or **Settings → UI → Updates → Install update**. Manual installation acquires
the pipeline's exclusive lease and refuses while recording, transcription,
import or retry owns it. **Check for updates** uses the same backend owner as
background checks, reports status, and downloads/verifies a newer package.
Closing to tray does not install, and OS shutdown is not delayed. The standard
Windows updater handoff restarts the updated application. Staged downloads are
memory-only; if the process is killed or the machine shuts down before Quit,
the next launch checks/downloads again.

Linux AppImage updates replace the writable installed image. Debian updates
use one OS-owned `pkexec /usr/bin/dpkg --install` approval prompt; cancellation
does not fall through to password collection or terminal sudo. Missing polkit,
unwritable images or package-install errors require a retry or manual download.
Mac updates stage the signed app archive in the application's parent folder,
then replace the bundle with rollback protection on the same filesystem. The
application must be in a writable folder (for example `~/Applications`), not
running from its read-only DMG. A failed rollback preserves the private backup.
Linux/Mac successful installation closes Kolboo; reopen it to run the update.
Updater signatures do not remove Gatekeeper, OS approval or publisher warnings.
Failed manual installs retain the verified download and leave Kolboo running.

Tauri creates updater signatures with the private updater key, while the
application contains only `app/src-tauri/updater.pubkey`. The release workflow
requires exactly one signed NSIS, MSI, AppImage, deb and universal Mac archive,
verifies their files exist, and publishes installer-specific manifest entries so MSI
installations receive MSI upgrades. The legacy generic Windows entry still
points to NSIS. The NSIS setup also skips the uninstall page for newer NSIS
versions run manually; app data and start-at-login registration are retained.
First installation, same-version repair and explicit installer-format migration
remain separate cases. See `app/src-tauri/windows/README.md` for template
provenance and Windows acceptance cases. Administrator/UAC approval for
machine-wide installs is not bypassed.

The universal Mac archive serves both `darwin-aarch64` and `darwin-x86_64`.
Linux deb and AppImage clients receive their own format; the generic Linux
fallback uses AppImage. RPM and unbundled development executables do not enable
native updating. Missing any platform signature fails feed generation rather
than silently omitting that platform. Automated Linux/Wry and filesystem tests
do not substitute for real Windows/Mac upgrade and rollback acceptance.

Existing 0.3.2 clients do not acquire this automatic-check behavior remotely:
users need one upgrade to the first release containing it, either through that
client's signed updater link or by running the new installer. Do not publish
an unverified installer as proof of upgrade acceptance.

Updater checks stay disabled in ordinary builds and in the manual-update Linux beta channel. Enable them only for a release channel that has completed signed update and rollback acceptance. Never rotate or lose the updater private key without a migration plan: installed clients trust its committed public counterpart.

## Cut and verify a release

1. Run `pnpm -C app check:ci`, `pnpm -C app coverage:patch`, and `pnpm -C app run audit`. Use `run audit` explicitly: bare `pnpm audit` invokes pnpm's JavaScript-only command, not the package script that also audits Rust.
2. Confirm package, Tauri, and Cargo versions match the intended `vX.Y.Z` tag.
3. Push the tag and inspect the Release workflow. It builds all three desktop platforms and prepares a **draft**, not a public release. Missing updater-key credentials are a launch blocker. Check whether the Windows publisher certificate is present; the workflow signs and verifies installers when it is, and clearly discloses unsigned installers when it is not.
4. Download and verify the draft artifacts, including Linux checksums and build evidence. Complete exact-package native acceptance before explicitly publishing the draft. Record any maintainer-approved platform-testing deferrals; never report them as passed. After publication, confirm unauthenticated downloads work and both Mac assets carry the experimental notice.
5. If publisher credentials were configured, verify Authenticode in PowerShell with `Get-AuthenticodeSignature <installer>`. Otherwise confirm the release title and notes disclose that the Windows installers are unsigned.
6. Install, launch, check for updates, and confirm that altered or updater-unsigned artifacts are rejected.
7. Record the workflow run, commit SHA, installer hash, updater result, request IDs, and support-safe correlation hashes in the launch evidence.

## Rollback

Do not overwrite a published tag. Mark the affected release as withdrawn, preserve its hashes and incident record, fix forward with a higher version, and publish a new updater-signed release. Metadata travels over HTTPS; clients verify the downloaded artifact against the committed updater key. Publisher Authenticode signing remains optional.

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
