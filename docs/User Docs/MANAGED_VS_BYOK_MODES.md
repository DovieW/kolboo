# Managed vs BYOK Modes

## What changed

Kolboo supports two inference modes:

- **Managed**: Kolboo routes STT/LLM through managed infrastructure.
- **BYOK**: Kolboo uses your configured provider keys directly.

## How mode is chosen

- **Personal active/grace** users can run managed mode.
- **Enterprise** users require a valid eligible org policy for managed mode.
- If managed routing is not available, Kolboo falls back to BYOK providers when configured.

## User-facing recovery behavior

If managed inference is temporarily unavailable:

1. Retry once (many outages are brief).
2. Switch Speech/Rewrite providers to BYOK to continue working.
3. Refresh entitlement/policy if org mode changed recently.

## Error categories

- `unauthorized`
- `ineligible`
- `over_quota`
- `temporarily_unavailable`

These categories are deterministic and safe to use for support triage.