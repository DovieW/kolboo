# Account Login and Entitlement Troubleshooting

Use this guide when account state appears stuck or degraded.

## Common symptoms

- Sign-in appears successful but state does not update
- State remains in **Grace**
- State moved to **Expired** after being offline

## Quick checks

1. Open **Account** and click **Refresh access**.
2. Verify network connectivity.
3. If you see **Restore your Kolboo session**, use **Send sign-in code** on that
   page. The saved session was rejected; repeatedly refreshing cannot restore it.
4. If still degraded without a sign-in warning, collect the safe diagnostic
   category before signing out so a service/network failure can be investigated.

## Expected behavior

- If refresh fails temporarily, state may move to **Grace**.
- Rejected sessions are distinct from offline grace and show **Restore your Kolboo session**.
- Managed model loading refreshes expiring access tokens automatically. A 401
  receives one refresh/retry, without changing the selected model or payment source.
- If refresh does not succeed before grace ends, state becomes **Expired**.
- Signed-out mode should still keep baseline non-account functionality available.

## Support notes

When reporting issues, include:

- Current account status badge shown in UI
- Approximate time of last successful sign-in
- Whether issue reproduces after sign out/in
- Any user-visible error message text (do not include secrets)
- The local `Account operation failed` stage/code/HTTP status, if present. These
  fields contain no email, token, transcript, prompt or provider response body.
- Whether startup says Sentry initialized or disabled. Handled account failures
  are explicitly reported only in builds configured for error reporting.
