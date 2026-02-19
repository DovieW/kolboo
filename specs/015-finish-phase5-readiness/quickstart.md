# Quickstart — Finish Remaining Phase 5/5A/5B Readiness

This guide validates the remaining scope in independently testable slices.

## Prerequisites

- Access to `kolboo` and `kolboo-private`
- Non-production credentials for preview/staging verification
- Required secrets configured for preview/prod workflows

## Slice A — Phase 5 admin workflows

Goal: Confirm org admin can complete core account-control workflows.

1. Sign in as org admin in dashboard.
2. Validate member lifecycle actions (invite, role change, revoke/restore).
3. Validate policy draft and publish flow with version metadata.
4. Validate usage/audit queries by time/category/actor filters.
5. Validate shared profile create/update/archive flow.
6. Validate billing access state and authorized billing management entry.

Pass criteria:
- Admin actions succeed with expected state updates.
- Viewer/non-admin actions are denied where required.

## Slice B — Phase 5A deterministic testing path

Goal: Confirm repeatable non-production validation without production backdoor risk.

1. Seed deterministic fixtures for context key (`local`, `pr-<id>`, staging persona key).
2. Run dashboard validation flows on seeded context.
3. Reset same context and verify no cross-context deletions.
4. Execute scripted staging persona smokes for:
   - BYOK org
   - managed org
   - mixed-policy org
5. Validate non-prod test-access session creation and audit evidence.
6. Confirm production hard-stop behavior for test-access/fixture routes (`NONPROD_ONLY_ROUTE`).

Pass criteria:
- Seed/reset deterministic and context-scoped.
- Persona smokes emit pass/fail evidence.
- Production test-access remains hard-blocked.

## Slice C — Phase 5B `kolops` operator CLI

Goal: Confirm internal operators can run audited platform actions.

1. Run org lifecycle commands (`org create|update|disable`).
2. Run policy commands (`policy set|publish|get`).
3. Run key commands (`keys rotate|list|disable`).
4. Run entitlement commands (`set-tier|get`).
5. Run reporting commands (`usage report`, `audit export`).
6. Verify deterministic CLI outputs and audit records for mutating commands.

### Concrete manual verification (preview/staging, real auth required)

Use this when validating end-to-end operator readiness:

1. Ensure API edge is reachable in preview/staging and obtain a valid admin bearer token (allowlisted user).
2. Set CLI actor identity via environment (`KOLOPS_ACTOR_USER_ID`, `KOLOPS_ACTOR_ROLE`).
3. Enable live audit forwarding by setting:
   - `KOLOPS_AUDIT_API_BASE_URL`
   - `KOLOPS_AUDIT_BEARER_TOKEN`
   - optional `KOLOPS_ORG_ROLE_HEADER` (default `admin`)
4. Run at least one mutating command from each group (`org`, `policy`, `keys`, `entitlement`).
5. Call `GET /v1/platform-admin/actions` with the same bearer token and verify:
   - records exist for each command group
   - `actorUserId` maps to authenticated user
   - key rotation payload digest contains redacted secret marker (`[redacted]`)
6. Capture evidence artifacts:
   - CLI JSON outputs with `request_id`
   - API response snippets for corresponding audit records
   - timestamp alignment between command execution and stored records

Pass criteria:
- Command groups execute with authorization enforcement.
- No secrets are printed.
- Audit records exist for mutating operations.
- Live audit forwarding records are persisted and queryable with authenticated admin context.

## Desktop integration checkpoints (`kolboo`)

1. Validate enterprise UI touchpoints reflect org-admin/account-control context.
2. Validate non-production persona/test-access context indicators are shown when active.
3. Validate any added settings/event touchpoints remain synchronized and deterministic.

Pass criteria:
- Desktop surfaces reflect admin/test persona state accurately.
- No command/event contract drift between desktop and cloud-facing wrappers.

## Final readiness gate

A change is ready when:

- Phase 5 workflows pass for authorized admins.
- Phase 5A deterministic path passes across local/preview/staging with production hard-stop intact.
- Phase 5B `kolops` command groups pass with audit evidence.
- Security/logging checks pass (no secret leakage).
- Final CI gate is green for touched repositories.
- Manual preview/staging evidence for live `kolops` audit forwarding is attached.
