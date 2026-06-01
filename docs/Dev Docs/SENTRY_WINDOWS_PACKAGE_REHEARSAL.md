# Windows packaged Sentry rehearsal

Last updated: 2026-05-29

This runbook covers the **desktop packaged-app** rehearsal path for frontend Sentry.

Use it after a Windows bundle build when you want proof that:

- the packaged desktop app can initialize frontend Sentry in a real Tauri shell
- the uploaded browser source maps line up with the packaged build release
- the resulting Sentry issue/event can be linked back to the exact GitHub Actions run and bundle artifact

This is intentionally separate from the browser-only artifact proof in
`SENTRY_INTEGRATION.md`.

## What this runbook uses

From `kolboo/.github/workflows/windows-build.yml`:

- `kolboo-windows-bundles`
- `kolboo-windows-bundles-local-whisper` (optional)
- `windows-build-evidence`

The `windows-build-evidence` artifact is the anchor. It records:

- `sentry_project`
- `sentry_environment`
- `sentry_release`
- `sentry_upload_enabled`
- bundle artifact names
- build run URL / commit SHA

For local operator rehearsals that first build/upload the frontend browser
artifact manually, the repo also now includes:

- `app/tauri.packagerehearsal.json`

That config overrides `beforeBuildCommand` so Tauri can package the exact
already-built `app/dist` bundle instead of silently rebuilding a different
frontend artifact before bundling.

## Preconditions

- Use a **non-production** Windows build for smoke rehearsal.
  - The desktop smoke helper intentionally blocks in `production` / `prod`.
  - Manual workflow runs and branch builds are the right target for this proof.
- You need a valid public-project DSN for the target environment.
  - Example for dev rehearsal: `kolboo-public-dev`
- You need the matching uploaded browser source maps for the release recorded in
  `windows-build-evidence.txt`.
- Run on a Windows machine or VM where you can launch the installed app from a
  PowerShell session that inherits the rehearsal env vars.

## 1. Trigger or pick the build

Use a successful Windows build workflow run that produced:

- the Windows bundle artifact you want to test
- `windows-build-evidence`

Prefer the exact run you intend to cite in release evidence.

## 2. Download the evidence + bundle artifacts

Download at minimum:

- `windows-build-evidence`
- `kolboo-windows-bundles`

If you specifically want the local-whisper variant, also download:

- `kolboo-windows-bundles-local-whisper`

## 3. Read the release metadata first

Open `windows-build-evidence.txt` and record:

- `sentry_project`
- `sentry_environment`
- `sentry_release`
- `run_url`

Do **not** guess the release string. Use the exact value from the evidence file.
That is the release whose uploaded source maps must resolve the packaged event.

## 4. Locate the packaged Windows artifact

The bundle artifact contains the Tauri release outputs copied from:

- `app/src-tauri/target/release/bundle/**`
- `app/src-tauri/target/release/*.exe`

Because `tauri.conf.json` uses `"bundle": { "targets": "all" }`, expect one or
more installer formats under `bundle/` plus the loose release executable.

For rehearsal, prefer this order:

1. use an installer from `bundle/` when available
2. fall back to the loose `kolboo.exe` only when installer validation is blocked

## Optional local operator shortcut

When you are running the rehearsal locally instead of consuming a GitHub Actions
artifact, and you already produced a specific browser bundle with something like
`vite build --sourcemap hidden` plus manual `sentry sourcemaps upload`, package
that exact `dist` directory with:

- `pnpm exec tauri build --no-sign --config tauri.packagerehearsal.json`

That avoids accidentally rerunning the default frontend build and drifting away
from the exact browser artifact whose source maps were uploaded to Sentry.

## 5. Set the packaged-app rehearsal env

In the same PowerShell session you will use to launch the installed app, set:

```powershell
$env:TAURI_SENTRY_DSN = "<public project dsn>"
$env:TAURI_SENTRY_ENV = "<non-production env from windows-build-evidence.txt>"
$env:TAURI_SENTRY_RELEASE = "<exact release from windows-build-evidence.txt>"
$env:TAURI_SENTRY_SMOKE = "1"
```

Optional cloud/runtime vars when the rehearsal also needs sign-in or cloud flows:

```powershell
$env:TAURI_API_BASE_URL = "<preview/dev api origin>"
$env:TAURI_MANAGED_INFERENCE_GATEWAY_URL = "<preview/dev api origin>"
$env:TAURI_SUPABASE_URL = "<supabase url>"
$env:TAURI_SUPABASE_PUBLISHABLE_KEY = "<publishable key>"
```

Why this matters:

- `TAURI_SENTRY_DSN` enables frontend/backend runtime Sentry in the packaged app
- `TAURI_SENTRY_RELEASE` must exactly match the uploaded release metadata
- `TAURI_SENTRY_SMOKE=1` triggers the explicit non-production frontend smoke path
  inside the real Tauri shell

## 6. Install and launch from the same shell

Install the package using the downloaded Windows installer.

Then launch the installed executable **from the same PowerShell session** so it
inherits the rehearsal env vars.

Common installed path on Windows is often similar to:

```text
%LocalAppData%\Programs\Kolboo\Kolboo.exe
```

But do not assume the exact location blindly—verify the installed path first.

If the installer path is blocked for some reason, launch the loose bundled exe as
an interim fallback and record that deviation in the rehearsal note.

## 7. Verify the Sentry event

Expected smoke characteristics:

- `surface=main`
- `action=smoke_test`
- `smoke_test=true`
- `smoke_trigger=runtime-env`
- context includes `runtime_env=TAURI_SENTRY_SMOKE`
- release equals the `sentry_release` from `windows-build-evidence.txt`

Capture these evidence items:

- Sentry issue ID / URL
- event ID
- release
- mapped source locations
- GitHub Actions run URL from `windows-build-evidence.txt`
- bundle artifact name used for the install
- whether installer or loose exe was used

If the event does not appear immediately, inspect the desktop rolling log at
`%AppData%\com.kolboo.app\logs\*.log` for frontend Sentry breadcrumbs emitted
through the log bridge (`scope=sentry`). These lines are the fastest way to tell
which phase failed:

- `runtime config unavailable ... retrying` — the renderer still cannot read
  `get_runtime_config`; keep investigating startup bridge timing
- `init skipped ... reason=no_dsn` — runtime config loaded, but no frontend DSN
  reached the renderer
- `initialized ... smoke_requested=true` — frontend Sentry was configured
- `smoke capture ...` followed by `smoke flushed ...` — the renderer queued and
  flushed the explicit smoke event, so any remaining gap is downstream of the
  local capture path
- `transport send status=200 ...` — the packaged webview successfully reached
  Sentry ingest; if the event is still not visible in the UI, the remaining gap
  is upstream search/permissions/operator verification rather than app delivery
- `transport send failed Failed to fetch` — the packaged renderer could not
  deliver to Sentry at all; for the 2026-05-29 rehearsal this was caused by the
  packaged Tauri CSP omitting Sentry ingest hosts from `connect-src`

For reference, the 2026-05-29 packaged rehearsal moved from repeated
`transport send failed Failed to fetch` on release
`kolboo@packagerehearsal.2026-05-29-d` to repeated
`transport send status=200 ...` on release
`kolboo@packagerehearsal.2026-05-29-e` after patching
`app/src-tauri/tauri.conf.json`.

## 8. Cleanup

After the rehearsal:

```powershell
Remove-Item Env:TAURI_SENTRY_DSN -ErrorAction SilentlyContinue
Remove-Item Env:TAURI_SENTRY_ENV -ErrorAction SilentlyContinue
Remove-Item Env:TAURI_SENTRY_RELEASE -ErrorAction SilentlyContinue
Remove-Item Env:TAURI_SENTRY_SMOKE -ErrorAction SilentlyContinue
```

Also remove any temporary cloud auth/runtime env vars you added for the session.

## Current limitation

This runbook creates a **supported packaged-app smoke path**, but it is still a
manual rehearsal:

- the workflow preserves the release/build evidence
- the app now supports an env-driven packaged smoke trigger
- the final proof still requires a human-operated Windows install/launch and a
  real captured issue/event in Sentry

That is acceptable for now because the missing gap is operational rehearsal, not
release metadata or source-map wiring.
