# Contracts parity notes

This feature keeps three contract surfaces aligned:

1. Spec API contract: `admin-phase5-readiness.openapi.yaml`
2. Spec CLI contract: `kolops-cli.contract.md`
3. Runtime API contract in private repo: `kolboo-private/apps/api-edge/contracts/admin-dashboard.openapi.yaml`

## Parity checks completed

- Added `/v1/platform-admin/actions` to both OpenAPI files.
- Aligned CLI actor/auth and exit-code behavior in `kolops-cli.contract.md`.
- Synced manual verification guidance in `quickstart.md` and runbook docs.

## Drift policy

When API/CLI behavior changes, update all three contract surfaces in the same change.
