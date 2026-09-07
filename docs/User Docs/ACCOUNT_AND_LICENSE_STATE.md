# Account and License State

Kolboo works without signing in. Account login is optional and only enables managed account context.

## States you may see

- **Signed out**: baseline functionality is available; no account identity is attached.
- **Active**: account entitlement is current.
  A freshly validated entitlement with no expiration date is Active, not Grace.
- **Grace**: entitlement refresh failed recently; account remains available during the grace window.
- **Expired**: grace window ended without a successful refresh.

## What is shown in Settings → Account

- Current account state and tier
- Signed-in email (if available)
- Organization name and ID (if your account has org context)
- Actions: Sign in, Refresh, Manage, Sign out

**Access level** describes your account entitlement, not the provider used for
each request. Personal managed access does not prevent using your own API keys:
choose those routes in AI settings. Meeting recordings have their own model
and managed/BYOK selection. These per-task choices do not sign you out or change
your plan; there is not yet a device-wide managed-inference off switch.

## Privacy and storage

- Session tokens are stored in secure OS-backed secret storage.
- Non-secret license/account snapshot state is stored in local settings.
- Baseline app behavior remains usable when signed out.
