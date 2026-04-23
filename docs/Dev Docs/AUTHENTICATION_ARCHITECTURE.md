# Authentication Architecture (Desktop + Edge)

This doc explains the target authentication architecture in Kolboo, where the trust boundaries are, and how state moves through the app.

> Scope: end-state behavior for `app/**` (Tauri desktop + React UI) and edge-enforced policy.

## TL;DR

- Identity/session lifecycle is desktop-owned in Tauri (`app/src-tauri/src/commands/licensing.rs`).
- Session secrets are stored in OS secure storage (`app/src-tauri/src/secrets.rs` via licensing helpers).
- UI reads auth state via typed command wrappers in `app/src/lib/tauri/license.ts`.
- Managed request deny outcomes are mapped to user-facing guidance in `app/src/lib/queries.ts`.
- Sign-in uses browser-based Authorization Code + PKCE (loopback callback) with desktop-owned session persistence.

## Trust boundaries

```mermaid
flowchart LR
	U[User] --> UI[React UI<br/>app/src/**]
	UI --> TAURI[Tauri commands<br/>commands/licensing.rs]
	TAURI --> SECURE[OS Secure Storage<br/>secrets.rs]
	TAURI --> STORE[Tauri Store<br/>settings.json]
	TAURI --> SB[Supabase Auth API]
	UI --> MG[Managed Gateway<br/>optional runtime URL]
	MG --> EDGE[api-edge policy+metering]

	classDef trust fill:#1f2937,color:#fff,stroke:#6b7280
	classDef local fill:#0f766e,color:#fff,stroke:#0d9488
	classDef cloud fill:#1d4ed8,color:#fff,stroke:#3b82f6

	class U trust
	class UI,TAURI,SECURE,STORE local
	class SB,MG,EDGE cloud
```

## Core data objects

- **`LicenseState`** (persisted in `settings.json`)
  - tier, status, email/user/org snapshot, usage+limits, timestamps
- **`SessionMaterial`** (secure storage only)
  - access token, refresh token
- **`LicenseAuthContext`** (computed command response)
  - authenticated boolean, `secure_session_present`, policy status, reason code, subject/org context

## Login + session persistence flow

```mermaid
sequenceDiagram
	autonumber
	participant User
	participant UI as AccountSettings.tsx
	participant API as tauriLicenseAPI<br/>license.ts
	participant Cmd as license_start_login<br/>Rust
	participant Browser as System Browser
	participant IdP as Auth Provider
	participant Loopback as Local Callback
	participant Sec as Secure Storage
	participant Store as settings.json

	User->>UI: Click Sign in
	UI->>API: startLogin(request)
	API->>Cmd: invoke("license_start_login")
	Cmd->>Browser: open authorize URL (PKCE challenge)
	Browser->>IdP: authorize request
	IdP-->>Loopback: redirect with auth code
	Loopback-->>Cmd: auth code received
	Cmd->>IdP: token exchange (code + verifier)
	IdP-->>Cmd: access_token + refresh_token + user
	Cmd->>Sec: persist_session_material(...)
	Cmd->>Store: save LicenseState(active)
	Cmd-->>API: LicenseState
	API-->>UI: LicenseState
	UI-->>User: Signed-in state shown
```

## Startup refresh flow

On app startup, backend does a best-effort silent refresh if secure session material exists.

```mermaid
sequenceDiagram
	autonumber
	participant Boot as App bootstrap<br/>lib.rs
	participant Cmd as license_refresh_entitlement
	participant Sec as Secure Storage
	participant Supa as Supabase Auth
	participant Store as settings.json

	Boot->>Sec: load_session_material()
	alt session exists
		Boot->>Cmd: spawn async refresh(simulate_failure=false)
		Cmd->>Supa: refresh token exchange
		alt refresh success
			Cmd->>Sec: persist refreshed session
			Cmd->>Store: save LicenseState(active)
		else refresh failure
			Cmd->>Store: save degraded state<br/>grace/expired
		end
	else no session
		Boot-->>Boot: skip refresh
	end
```

## License status state machine

```mermaid
stateDiagram-v2
	[*] --> signed_out
	signed_out --> active: login success
	active --> grace: token expired + within grace window
	grace --> expired: grace deadline passed
	active --> signed_out: logout
	grace --> signed_out: logout
	expired --> signed_out: logout
	grace --> active: refresh success
	expired --> active: refresh success
```

## Managed request error mapping (UI behavior)

`toManagedInferenceMessage(...)` in `app/src/lib/queries.ts` resolves the user message in this order:

1. If explicit `reason_code` exists, show reason-specific guidance first.
2. Else map by coarse category (`unauthorized`, `ineligible`, `over_quota`, fallback).

```mermaid
flowchart TD
	E[Managed error payload] --> RC{reason_code present?}
	RC -->|yes| MSG1[Use authReasonCodeToMessage]
	RC -->|no| CAT{category}
	CAT -->|unauthorized| M1[Session expired<br/>Sign in again]
	CAT -->|ineligible| M2[Account/org not eligible]
	CAT -->|over_quota| M3[Limit reached<br/>Use BYOK or wait]
	CAT -->|other| M4[Temporary unavailable<br/>Retry or BYOK]
	MSG1 --> OUT[Display actionable message]
	M1 --> OUT
	M2 --> OUT
	M3 --> OUT
	M4 --> OUT
```

## Auth context command and why it exists

`license_get_auth_context` gives the UI a normalized, backend-owned snapshot:

- `authenticated`
- `secure_session_present`
- `policy_status` (`allow`/`deny`)
- `reason_code` (`reauth_required`, `token_invalid`, etc.)

This keeps frontend logic thin and avoids duplicating auth-state derivation in React.

## Runtime environment knobs

- `TAURI_SUPABASE_URL`
- `TAURI_SUPABASE_PUBLISHABLE_KEY`
- `TAURI_AUTH_PROVIDER` (required browser provider, e.g. `google`)
- `TAURI_AUTH_ISSUER` (optional issuer hint for auth context)

If Supabase vars are missing, auth commands return `auth_not_configured`.

## File map

- Backend auth commands: `app/src-tauri/src/commands/licensing.rs`
- Backend auth types/helpers: `app/src-tauri/src/licensing.rs`
- Secure secrets: `app/src-tauri/src/secrets.rs`
- Startup refresh trigger: `app/src-tauri/src/lib.rs`
- Frontend wrappers: `app/src/lib/tauri/license.ts`
- Frontend command surface: `app/src/lib/tauri/commands.ts`
- UI account page: `app/src/components/settings/AccountSettings.tsx`
- Error-to-message mapping: `app/src/lib/queries.ts`
