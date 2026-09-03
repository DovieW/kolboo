# Linux development

**Status:** public x86_64 Community/BYOK beta; provisional support

**Last reviewed:** 2026-09-03

Kolboo's public Linux Community beta began with [`v0.2.5-beta.1`](https://github.com/DovieW/kolboo/releases/tag/v0.2.5-beta.1). The channel is limited to x86_64 Ubuntu/Kubuntu and account-free Community/BYOK use. It is not a managed-service launch, a stable-platform declaration, or a promise of native Wayland feature parity.

## Ubuntu and Kubuntu prerequisites

The Rust/Tauri build requires native GTK, WebKit, audio, tray, TLS, and input development packages:

```sh
sudo apt-get update
sudo apt-get install -y \
  libasound2-dev \
  libayatana-appindicator3-dev \
  libglib2.0-dev \
  libgtk-3-dev \
  libjavascriptcoregtk-4.1-dev \
  librsvg2-dev \
  libsoup-3.0-dev \
  libssl-dev \
  libwebkit2gtk-4.1-dev \
  libxdo-dev \
  patchelf \
  pkg-config
```

Install JavaScript dependencies and build the installable packages:

```sh
pnpm -C app install --frozen-lockfile
TAURI_API_BASE_URL=https://kolboo.dovie.dev \
TAURI_MANAGED_INFERENCE_GATEWAY_URL=https://kolboo.dovie.dev \
VITE_SIGNED_UPDATER_ENABLED=false \
pnpm -C app tauri build --no-sign --bundles deb,appimage
```

For an interactive development launch, run `pnpm -C app dev`. The repository uses
Cargo's sparse registry so a clean Linux checkout does not download the full Git
index before compiling.

The `Linux Build` GitHub workflow builds on Ubuntu 22.04 for a conservative glibc baseline. It retains the `.deb`, AppImage, SHA-256 checksums, dependency report, package contents, and commit/run evidence for 14 days. The updater is intentionally disabled for this beta channel.

## Install, update, and remove

Use one package format at a time.

Debian package:

```sh
sudo apt install ./Kolboo_*_amd64.deb
sudo apt remove kolboo
```

Portable AppImage:

```sh
chmod +x ./Kolboo_*.AppImage
./Kolboo_*.AppImage
```

Remove the AppImage by deleting that file. User settings and secure-store entries are not silently deleted by either uninstall path.

Updates are manual during the beta: download a newer prerelease, verify it against `SHA256SUMS`, and install or launch it. To roll back, download the previous prerelease and reinstall its `.deb` or run its AppImage. Never overwrite an existing release tag or asset; withdraw a defective release and fix forward with a higher version.

## Release procedure

1. Push the release commit and run `Linux Build` manually.
2. Download its exact artifact and verify `sha256sum -c SHA256SUMS`.
3. Complete the packaged acceptance matrix below on the `.deb` and do a launch check with the AppImage.
4. Record the commit SHA, workflow run, package hashes, desktop session, and outcomes.
5. Confirm the repository secret `TAURI_SENTRY_DSN` contains the beta project's public client DSN. Tagged builds fail closed without it so released crashes are observable.
6. Only after acceptance, create and push the matching `vX.Y.Z-beta.N` tag. `Linux Community Beta Release` verifies the tag/version, rebuilds the packages, attests their provenance, and creates a GitHub prerelease.
7. Download the public assets without authentication and recheck their hashes and launch behavior.

Stable Windows releases ignore `-beta.N` tags, so a Linux beta cannot accidentally enter the signed Windows release workflow.

## X11 and Wayland behavior

Kolboo detects the Linux session using `XDG_SESSION_TYPE`, then falls back to `WAYLAND_DISPLAY` or `DISPLAY`.

- X11 keeps the current automatic clipboard-and-keyboard paste path.
- Standard Wayland `xdg-shell` windows do not have a global coordinate system, so a normal application cannot reliably place an overlay at screen bottom-center. When a Wayland session also exposes XWayland through `DISPLAY`, Kolboo automatically runs its GTK/Tauri windows through XWayland so anchored overlay placement remains deterministic. Kolboo uses XSettings fractional DPI for the native overlay rectangle; WebKit inherits that same desktop scale itself, so no additional webview zoom is applied. This does not change the session classification used for input safety.
- Set `KOLBOO_LINUX_WINDOW_BACKEND=wayland` to test the native Wayland window path, or `KOLBOO_LINUX_WINDOW_BACKEND=x11` to require X11/XWayland. Native Wayland uses compositor-selected placement until Kolboo adopts a broadly supported shell protocol capable of anchored utility surfaces.
- Wayland does not promise global synthetic keyboard insertion. Completed output that requested automatic paste is copied to the clipboard once, and the UI shows an explicit fallback notification.
- Streaming live output is disabled on Wayland so partial chunks are not repeatedly copied to the clipboard. The final completed transcript uses the clipboard fallback.
- Native Wayland sessions register shortcuts through the compositor-owned XDG Global Shortcuts portal. This prevents a shortcut such as F3 from also reaching the focused Wayland application. The desktop may show a confirmation dialog the first time a binding is requested or after the binding changes, and the desktop remains authoritative over the final assigned trigger.
- X11 sessions use the Tauri global-shortcut backend. Portal or X11 registration failures remain visible in diagnostics rather than preventing app startup.

The fallback retains the transcript and avoids reporting a paste that did not happen. It does not make Wayland globally injected text a supported capability.

## Focused validation

For a Linux platform change, prefer:

```sh
pnpm -C app cargo:fmt:check
pnpm -C app cargo:test
TAURI_API_BASE_URL=https://kolboo.dovie.dev \
TAURI_MANAGED_INFERENCE_GATEWAY_URL=https://kolboo.dovie.dev \
VITE_SIGNED_UPDATER_ENABLED=false \
pnpm -C app tauri build --no-sign --bundles deb,appimage
```

The release artifact, not a dev-server build, must pass and record:

- launch and tray lifecycle;
- microphone enumeration and recording;
- X11 automatic paste or Wayland clipboard fallback;
- shortcut registration behavior, including that F3 does not also trigger the focused application's action on Wayland;
- recording-overlay mapping without moving focus away from the previously focused input;
- secure-storage availability and failure messaging;
- overlay visibility and monitor placement, including work-area bottom-center placement without drift across repeated recordings;
- opt-in Sentry release/environment/platform tags without sensitive content;
- `.deb` install, clean launch, removal, reinstall, and rollback;
- AppImage launch and second-launch persistence;
- Community/BYOK remains useful while signed out and when Kolboo cloud is unavailable.

The initial beta may be published after this matrix passes on the current native Kubuntu x86_64 system for the exact GitHub-built package. Broader Linux support remains provisional until representative X11/Wayland and display-scaling combinations accumulate acceptance evidence.
