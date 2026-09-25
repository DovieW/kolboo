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
- The selected provider is authoritative: **Kolboo Managed** uses the managed
  gateway, while a named cloud provider uses that provider's configured API
  key. Having Managed access never silently reroutes a BYOK selection.
- If managed routing is not available, Kolboo falls back to BYOK providers when configured.

The Managed model catalog returned by API Edge is authoritative and includes a
server-controlled catalog version. Enabling or disabling a model at API Edge
changes both discovery and request authorization without a desktop release. The
desktop does not infer Managed support merely because a provider or model is
available for BYOK.

BYOK LLM choices are refreshed from `models.dev` only for provider adapters
Kolboo already supports. The compact supported subset is cached for one day and
falls back to the catalog shipped with the desktop when refresh is unavailable.
This metadata request contains no prompts, transcripts, API keys, or account
identifiers. `models.dev` availability never enables a Managed model.

## Adding your own key

In **Settings → Providers**, paste a provider key and press Enter or leave the
field to save it. OCR uses the same control. The status underneath confirms
whether the save completed; if it fails, the replacement draft stays in the
field so you can retry.

Saved keys remain in the OS credential vault. The settings page checks whether
a key exists instead of reading it back into the form. Paste a new key to
replace it. Leaving an empty field does not delete a saved key: use **Remove**
and confirm to delete it deliberately. The show/hide control only reveals a
replacement draft you have entered.

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
