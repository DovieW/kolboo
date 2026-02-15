# Account Login and Entitlement Troubleshooting

Use this guide when account state appears stuck or degraded.

## Common symptoms

- Sign-in appears successful but state does not update
- State remains in **Grace**
- State moved to **Expired** after being offline

## Quick checks

1. Open **Settings → Account** and click **Refresh**.
2. Verify network connectivity.
3. If still degraded, click **Sign out** and sign back in.

## Expected behavior

- If refresh fails temporarily, state may move to **Grace**.
- If refresh does not succeed before grace ends, state becomes **Expired**.
- Signed-out mode should still keep baseline non-account functionality available.

## Support notes

When reporting issues, include:

- Current account status badge shown in UI
- Approximate time of last successful sign-in
- Whether issue reproduces after sign out/in
- Any user-visible error message text (do not include secrets)
