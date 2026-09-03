# Desktop product roadmap

**Status:** Active

**Last reviewed:** 2026-09-03

This document translates the cross-repository Kolboo roadmap into desktop-owned work. Company priorities and deferrals are controlled by `kol-software/ROADMAP.md`; this file owns implementation detail for the Tauri application.

Development uses focused subsystem checks by default. Full desktop and platform gates run at milestones, before delivery to real users, or asynchronously; browser automation is reserved for a few critical cross-surface journeys. See [Testing and validation](Dev%20Docs/TESTING.md).

## Current priorities

### Reliability and Sentry

- Capture actionable React, Tauri, and Rust failures across every desktop surface.
- Attach environment, release, platform, architecture, surface, request ID, and support-safe account correlation.
- Preserve the strict no-transcript, no-prompt, no-audio, no-OCR, and no-secret telemetry boundary.
- Make crash/panic, stuck recording, failed auth, failed update, and managed-inference failures reproducible from safe diagnostics.
- Maintain a code-grounded bug backlog from Sentry and user reports; prioritize crashes, data loss, security, and core dictation failures.

Detailed integration and rehearsal guidance lives in [Sentry integration](Dev%20Docs/SENTRY_INTEGRATION.md).

### Linux and macOS

- Maintain and learn from the public x86_64 Linux Community/BYOK beta beginning with [`v0.2.5-beta.1`](https://github.com/DovieW/kolboo/releases/tag/v0.2.5-beta.1); keep its support status provisional.
- Keep Linux beta updates manual until the channel has enough upgrade and rollback evidence for signed automatic updates.
- Continue building the default application in macOS CI while native acceptance is deferred until a Mac is available.
- Isolate platform behavior for audio, shortcuts, text insertion, overlays, permissions, startup, tray/window behavior, secure storage, notifications, and updates.
- Add explicit capability detection and deterministic fallbacks, especially for Linux Wayland and macOS Accessibility/Screen Recording permissions.
- Define per-platform packaging and manual acceptance matrices before marking a platform supported.
- Keep Windows behavior covered while portable seams are introduced.

The detailed platform matrix is in [Cross-platform compatibility](Plans/CROSS_PLATFORM_COMPATIBILITY.md).

### Managed inference and models

- Keep Community/local/BYOK usable without an account or Kolboo cloud availability.
- Route Personal managed requests only through authenticated private APIs; the desktop never receives Kolboo-managed gateway/provider secrets.
- Represent provider/model capabilities explicitly so settings and runtime behavior cannot drift.
- Add model choices with request-shape, timeout, error, cancellation, usage, and fallback tests.
- Surface managed quota, availability, and errors without misleading users into changing BYOK keys.

### Architecture

- Preserve the module ownership rules in [Architecture guardrails](Dev%20Docs/ARCHITECTURE_GUARDRAILS.md).
- Refactor when feature work exposes repeated invariants or platform coupling, not merely because a file is large.
- Continue narrowing Windows-only code behind tested platform interfaces.
- Keep generated Rust/TypeScript schemas, event names, command wrappers, and defaults synchronized.
- Move unresolved, code-grounded follow-ups into `docs/Refactors/`; do not create another parallel general-purpose TODO file.

### Product and private cohort

- Keep account-first Community behavior and API-key management dependable.
- Make Personal managed mode, sync, entitlement refresh, and recovery clear in the UI.
- Improve diagnostics and support-safe evidence for real private users.
- Fix user-reported bugs before expanding the cohort.

## Definition of supported platform

A platform is supported only when:

- CI builds and automated checks pass on that platform;
- install/uninstall and first-run permissions are documented;
- recording, transcription, output insertion, shortcuts, tray/window behavior, local/BYOK, managed mode, logs, and secure storage pass manual acceptance;
- platform limitations have deterministic user-visible fallbacks;
- packaging and rollback ownership are documented.

Source code compiling conditionally is not sufficient.

## Deferred desktop work

Deferred without a date:

- stable Linux promotion, signed automatic Linux updates, and non-x86_64 Linux packages;
- public stable multi-platform release promotion;
- broad OSS/community launch promotion beyond the limited Linux beta;
- public legal/marketing link promotion;
- enterprise-specific desktop UX, SSO, or SLA claims.

The source repository and a limited Linux Community/BYOK prerelease are public. Neither fact makes a platform stable/supported, opens managed signup, or completes a product launch. Updater, signing, license, and publication code may be maintained for correctness, but public-launch completion is not a current priority.

## Backlog rule

New work belongs in one of three places:

1. this roadmap for current desktop outcomes;
2. `docs/Refactors/` for specific architecture debt tied to files and demonstrated pain;
3. an issue or focused implementation spec for an executable feature slice.

Do not recreate broad overlapping testing, TODO, review, or ideal-state plans.
