# Managed vs BYOK Modes

## What changed

Kolboo supports two inference modes:

- **Managed**: Kolboo routes STT/LLM through managed infrastructure.
- **BYOK**: Kolboo uses your configured provider keys directly.

## How mode is chosen

- **Personal active/grace** users can run managed mode.
- **Enterprise** users require a valid eligible org policy for managed mode.
- Eligible users initially see only providers and models offered by Kolboo Managed.
- **Show all providers and models** reveals BYOK and local choices. A model that is
  not in the Managed catalog is labelled as requiring the user's own API key.
- When a selected model supports both modes, **Use your own API key** switches
  that setting to BYOK without changing the model. Managed-only models do not
  offer that switch.
- If managed routing is not available, Kolboo falls back to BYOK providers when configured.

The Managed model catalog returned by the API is authoritative. The desktop
does not infer Managed support merely because a provider or model is available
for BYOK.

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
