# Security Notes

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
