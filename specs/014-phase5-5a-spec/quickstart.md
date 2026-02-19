# Quickstart: Phase 5 Dashboard + 5A Admin Testing Path

This quickstart describes the target validation flow for implementation in `kolboo-private`.

## Prerequisites

- Access to `kolboo-private` repository
- Cloudflare and Supabase credentials configured for non-production and production
- CI permissions for preview deployments and manual production approval

## 1) Local validation (developer loop)

1. Implement/adjust API handlers and role guards for:
   - member management
   - policy draft/publish
   - usage/audit filtering
2. Run local lint/typecheck/tests and ensure deterministic test fixtures pass.
3. Validate that no secrets are logged and no production bypass flags are introduced.

## 2) Preview validation (required before prod)

1. Open a PR to trigger preview deployment.
2. Run preview seed for context key (e.g., `pr-<number>`).
3. Confirm fixture/test-access routes are available in preview and return success for authorized admin requests.
4. Execute admin validation path in preview:
   - member lifecycle action
   - policy edit + publish
   - usage/audit query/filter checks
5. Run preview reset and verify only context-tagged fixture rows are removed.
6. Record preview evidence (run id, context key, status, fixture seed/reset confirmation).

## 3) Production promotion (manual approval required)

1. Confirm CI is green and preview evidence is present.
2. Trigger manual production deploy approval.
3. After deployment, run production smoke checks:
   - `health`
   - `ready`
   - auth-guarded endpoint rejection/acceptance behavior
   - fixture/test-access routes return `404 NONPROD_ONLY_ROUTE`
   - policy draft→publish roundtrip check
4. Record smoke outcome and rollback reference if failed.

## 4) Rollback path (on smoke failure)

1. Mark release as failed.
2. Execute documented rollback for the impacted deployment.
3. Re-run smoke checks on restored version.
4. Attach failure + rollback evidence to release record.

## Completion Criteria

A change is phase-ready when:
- preview seed/validate/reset works deterministically,
- admin RBAC behavior is enforced (`owner/admin/viewer`),
- production deploy was manually approved,
- smoke evidence is recorded,
- and rollback instructions are verified executable.
