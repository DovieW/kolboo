# Linux development

**Status:** Active engineering; not yet a supported release platform

**Last reviewed:** 2026-08-09

Kolboo is being built and validated natively on Linux. The current goal is a dependable private development build, followed by explicit X11 and Wayland acceptance—not a public Linux release.

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

Install JavaScript dependencies and build the native binary:

```sh
pnpm -C app install --frozen-lockfile
pnpm -C app tauri build --no-bundle
```

For an interactive development launch, run `pnpm -C app dev`. The repository uses
Cargo's sparse registry so a clean Linux checkout does not download the full Git
index before compiling.

The `Linux Build` GitHub workflow performs the same native build on Ubuntu and retains the development binary plus its shared-library report for seven days.

## X11 and Wayland behavior

Kolboo detects the Linux session using `XDG_SESSION_TYPE`, then falls back to `WAYLAND_DISPLAY` or `DISPLAY`.

- X11 keeps the current automatic clipboard-and-keyboard paste path.
- Wayland does not promise global synthetic keyboard insertion. Completed output that requested automatic paste is copied to the clipboard once, and the UI shows an explicit fallback notification.
- Streaming live output is disabled on Wayland so partial chunks are not repeatedly copied to the clipboard. The final completed transcript uses the clipboard fallback.
- Global shortcuts remain best-effort while compositor-specific support is evaluated. Registration failures must remain visible in diagnostics rather than preventing app startup.

The fallback retains the transcript and avoids reporting a paste that did not happen. It does not make Wayland globally injected text a supported capability.

## Focused validation

For a Linux platform change, prefer:

```sh
pnpm -C app cargo:fmt:check
pnpm -C app cargo:test
pnpm -C app tauri build --no-bundle
```

Manual acceptance should separately record the desktop session and verify:

- launch and tray lifecycle;
- microphone enumeration and recording;
- X11 automatic paste or Wayland clipboard fallback;
- shortcut registration behavior;
- secure-storage availability and failure messaging;
- overlay visibility and monitor placement;
- Sentry release/environment/platform tags.

Linux remains unsupported until this matrix passes on representative native systems and a private installable bundle has a documented rollback path.
