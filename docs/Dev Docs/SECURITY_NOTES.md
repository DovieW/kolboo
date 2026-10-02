# Security Notes

## Desktop input approval — 2026-09-30

Wayland paste/copy identifies its own D-Bus connection as `com.kolboo.app`, requests
keyboard-only RemoteDesktop access with persistent restoration, and never falls
back to anonymous XWayland input. Linux package metadata includes the matching
hidden desktop entry. Desktop-issued restore tokens are backend-only OS keyring
entries, not API keys, settings, sync data, or renderer-readable secrets. Rotated
tokens replace old values; missing persistence clears obsolete values. Rejection
or unsupported capabilities latch clipboard fallback for the current launch;
partial key delivery is cleaned up, never automatically replayed. No desktop-wide
pre-authorization or security-policy change is made.

The adapter is tested with a real ashpd client on isolated D-Bus services (requires
`dbus-daemon`), with synthetic tokens only. Native desktop approval, restart,
revocation, installed identity lookup and secure-store availability still require
manual acceptance. Windows keeps its UIA/SendInput path and OS integrity rules;
macOS checks actual Accessibility trust and limits automatic prompting to once
per launch. Neither OS's security controls are bypassed.

## Build dependency audit — 2026-09-29

The Sentry Vite build-tool dependency chain (`glob` → `minimatch` →
`brace-expansion`) is pinned to patched `brace-expansion` 5.0.12 for the 4/5
release line. This fixes the reported recursive-pattern denial-of-service
advisories without changing desktop runtime code or bypassing the dependency
release-age policy. The separate moderate Vitest advisory remains an existing
test-tool issue, not a desktop autostart dependency.

## Cloud policy pack handling

### Trust boundary

Policy validation and acceptance are backend-owned in Rust.

### Rejection rules

The app rejects candidate policy updates that are:
- structurally invalid
- using unsupported constraint keys
- expired at receipt time
- regressive in version (older than currently active)

### Redaction policy

Policy diagnostics export must not leak secrets.

Current redaction behavior strips effective values for fields whose path includes:
- `api_key`
- `token`
- `password`
- `secret`
- `credential`

The current support bundle may include request IDs, app/Sentry release metadata,
and hashed operator-correlation targets, but it must not include raw org names,
raw internal IDs, transcript content, or provider credentials.

### Failure-mode behavior

When sync fails:
- preserve last valid policy where applicable (`cached`)
- do not apply invalid candidate payloads
- transition to `degraded_expired` after expiry and continued failure

### Logging guidance

- Never log raw API keys or auth headers.
- Use redacted diagnostics payloads for support workflows.
- Keep policy failure reasons terse and non-sensitive.
