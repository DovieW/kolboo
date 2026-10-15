# Account and License State

Kolboo works without signing in. Account login is optional and only enables managed account context.

The signed-out Account page opens directly to a compact, left-aligned sign-in
form. Enter your email and choose **Send sign-in code**, then enter the emailed
six-digit code. **Use password** and **Use browser** remain secondary options;
**Forgot password?** under password sign-in sends a reset email. New accounts
are invite-only. Your own API keys work without an account.

If the saved session is rejected, **Restore your Kolboo session** replaces the
account summary without implying a plan downgrade. The saved email is prefilled
and editable; nothing is sent until you submit. **Sign out** remains under
**Account details**. Refresh and management actions return after authentication.
Transient offline grace still shows the account summary and **Refresh access**.

## States you may see

- **Signed out**: baseline functionality is available; no account identity is attached.
- **Active**: account entitlement is current.
  A freshly validated entitlement with no expiration date is Active, not Grace.
- **Grace**: entitlement refresh failed recently; account remains available during the grace window.
- **Expired**: grace window ended without a successful refresh.

## What is shown in Account

- Current account state and tier
- Signed-in email (if available)
- Organization name and ID (if your account has org context)
- Actions: Sign in, Refresh, Manage, Sign out

**Advanced → Organization policy** contains read-only organization/device rules,
policy refresh, and redacted diagnostics export. It replaces the former Policy
settings tab and remains accessible without signing in, including Community
packages. Account identity details remain separate from Advanced diagnostics.

In AI settings, model controls show a compact friendly name and a **Managed** or
**Your models** badge. Hover the control or open its picker to see the provider
and full selection. Labels and controls share a row when space permits and stack
in narrow views. Browsing the picker does not change a selection or payment source.

**Access level** describes your account entitlement, not the provider used for
each request. Personal managed access does not prevent using your own API keys:
choose those routes in AI settings. Meeting recordings have their own model
and managed/BYOK selection. These per-task choices do not sign you out or change
your plan; there is not yet a device-wide managed-inference off switch.

## Privacy and storage

- Session tokens are stored in secure OS-backed secret storage.
- Non-secret license/account snapshot state is stored in local settings.
- Baseline app behavior remains usable when signed out.
