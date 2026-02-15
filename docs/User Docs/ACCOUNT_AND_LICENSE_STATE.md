# Account and License State

Kolboo works without signing in. Account login is optional and only enables managed account context.

## States you may see

- **Signed out**: baseline functionality is available; no account identity is attached.
- **Active**: account entitlement is current.
- **Grace**: entitlement refresh failed recently; account remains available during the grace window.
- **Expired**: grace window ended without a successful refresh.

## What is shown in Settings → Account

- Current account state and tier
- Signed-in email (if available)
- Organization name and ID (if your account has org context)
- Actions: Sign in, Refresh, Manage, Sign out

## Privacy and storage

- Session tokens are stored in secure OS-backed secret storage.
- Non-secret license/account snapshot state is stored in local settings.
- Baseline app behavior remains usable when signed out.
