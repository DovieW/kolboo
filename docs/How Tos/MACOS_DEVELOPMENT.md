# macOS development

**Status:** Active engineering; not yet a supported release platform

Kolboo's private macOS test build is produced by the manually triggered
`macOS Development Build` GitHub Actions workflow. It does not run for pushes or
pull requests, so macOS runner usage remains an explicit decision.

## Build an artifact

1. Open **Actions → macOS Development Build → Run workflow**.
2. Choose `apple-silicon`, `intel`, or `universal`.
3. Download the `kolboo-macos-*` artifact after the job succeeds.
4. Prefer the DMG for normal installation testing. The application ZIP is also
   retained for bundle inspection and troubleshooting.

The development artifact uses ad-hoc signing. It is not notarized, does not
enable signed updater delivery, and is not a public release. macOS may require
the tester to approve the application from **System Settings → Privacy &
Security** after the first download.

## Deferred native-validation checkpoint

Native acceptance is paused until a Mac is available. This pauses the manual
test pass, not the macOS implementation work or the ability to produce another
artifact.

The first Apple Silicon development bundle was produced on 2026-08-09:

- desktop commit: `cb99f1f67ea3584aa7487dae0de5bd7cdfa6786e`;
- workflow run:
  [31347333237](https://github.com/DovieW/kolboo/actions/runs/31347333237);
- build target: `aarch64-apple-darwin`;
- CI evidence: the application and DMG were produced, the application passed
  strict `codesign` verification, and the executable was confirmed as ARM64.

The artifact expires and can be rebuilt from the manual workflow. No one has
launched this bundle on a Mac, granted its permissions, or exercised its native
behavior. Resume with the acceptance pass below and retain the Mac model,
architecture, macOS version, workflow run, commit, and observed results.

## First native acceptance pass

Record the macOS version, Mac architecture, workflow run, and commit SHA, then
check:

- launch, close, reopen, tray lifecycle, and single-instance behavior;
- microphone permission copy, allow/deny behavior, device selection, and a full
  recording;
- F3 toggle registration and any shortcut-conflict guidance;
- compact and expanded overlay placement, including over a fullscreen app;
- transcript insertion into a native text field and a browser text field;
- explicit Accessibility guidance when input automation is unavailable;
- account sign-in/session recovery and Keychain-backed secret storage;
- Community/BYOK continuity when managed services are unavailable;
- logs and Sentry metadata without transcript, audio, prompt, or secret content.

Do not describe macOS as supported based on a successful bundle alone. Fix
compile and first-run blockers in focused slices, then retain native evidence for
the behaviors actually exercised.
