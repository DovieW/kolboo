# Data Model — Finish Remaining Phase 5/5A/5B Readiness

## Entity: OrganizationMember

- **Purpose**: Represents user membership in an enterprise org.
- **Core fields**:
  - `id` (string/uuid)
  - `org_id` (string/uuid)
  - `user_id` (string/uuid)
  - `email` (string)
  - `role` (`owner | admin | viewer`)
  - `status` (`invited | active | revoked`)
  - `created_at`, `updated_at` (datetime)
- **Validation rules**:
  - `role` required and constrained to enum.
  - `owner` role transitions require elevated authorization.
  - Unique active membership per (`org_id`, `user_id`).
- **State transitions**:
  - `invited -> active`
  - `active -> revoked`
  - `revoked -> active` (restore when allowed)

## Entity: PolicySnapshot

- **Purpose**: Versioned org policy draft/published state.
- **Core fields**:
  - `id` (string/uuid)
  - `org_id` (string/uuid)
  - `version` (integer)
  - `status` (`draft | published`)
  - `content` (json/object)
  - `published_at` (datetime, nullable)
  - `updated_by` (string/uuid)
- **Validation rules**:
  - `content` required for draft save.
  - Publish requires valid draft and version consistency checks.
- **State transitions**:
  - `draft -> published`
  - `published -> draft` (new version draft)

## Entity: UsageAuditRecord

- **Purpose**: Content-free event telemetry for admin reporting.
- **Core fields**:
  - `id` (string/uuid)
  - `org_id` (string/uuid)
  - `category` (`usage | audit`)
  - `action` (string)
  - `actor_user_id` (string/uuid, nullable)
  - `metadata` (json/object)
  - `occurred_at` (datetime)
- **Validation rules**:
  - Query windows require valid `from <= to`.
  - Category filters constrained to enum.

## Entity: SharedProfile

- **Purpose**: Reusable org-level profile configuration shared with members.
- **Core fields**:
  - `id` (string/uuid)
  - `org_id` (string/uuid)
  - `name` (string)
  - `config` (json/object)
  - `status` (`active | archived`)
  - `created_by` (string/uuid)
  - `created_at`, `updated_at` (datetime)
- **Validation rules**:
  - `name` required and unique within active profiles for org.
  - Archived profiles remain queryable but not assignable.
- **State transitions**:
  - `active -> archived`
  - `archived -> active` (optional restore)

## Entity: BillingAccessState

- **Purpose**: Organization billing visibility and secure action entry points.
- **Core fields**:
  - `org_id` (string/uuid)
  - `plan_tier` (string)
  - `seat_count` (integer)
  - `billing_status` (string)
  - `portal_url` (string/url, nullable)
  - `last_synced_at` (datetime)
- **Validation rules**:
  - Billing actions require `owner/admin` authorization.
  - `portal_url` must be HTTPS when present.

## Entity: ValidationContext

- **Purpose**: Deterministic fixture scope for non-production testing.
- **Core fields**:
  - `context_key` (string, e.g., `pr-123`, `staging-byok`)
  - `environment` (`local | preview | staging`)
  - `seed_status` (`seeded | already-seeded | reset | no-op`)
  - `seeded_at`, `reset_at` (datetime)
  - `seeded_by` (string)
- **Validation rules**:
  - Context key required and environment-scoped.
  - Reset may only remove data tagged to same context.

## Entity: StagingPersona

- **Purpose**: Stable named persona for smoke validation.
- **Core fields**:
  - `id` (string)
  - `persona_type` (`byok | managed | mixed-policy`)
  - `org_id` (string/uuid)
  - `policy_variant` (string)
  - `enabled` (boolean)
- **Validation rules**:
  - Exactly one active persona per `persona_type` in staging baseline.

## Entity: TestAccessSession

- **Purpose**: Short-lived non-production elevated testing session.
- **Core fields**:
  - `id` (string/uuid)
  - `environment` (`dev | preview | staging`)
  - `actor_user_id` (string/uuid)
  - `scope` (string)
  - `issued_at`, `expires_at` (datetime)
  - `status` (`active | expired | revoked`)
- **Validation rules**:
  - Never valid in production.
  - TTL required and bounded.
- **State transitions**:
  - `active -> expired`
  - `active -> revoked`

## Entity: ReleaseEvidenceRecord

- **Purpose**: Deployment validation evidence linkage.
- **Core fields**:
  - `id` (string/uuid)
  - `commit_sha` (string)
  - `preview_run_id` (string)
  - `prod_run_id` (string)
  - `manual_approved_by` (string)
  - `manual_approved_at` (datetime)
  - `smoke_status` (`pending | passed | failed`)
  - `smoke_summary` (string)
  - `rollback_reference` (string, required when failed)
  - `created_at` (datetime)
- **Validation rules**:
  - `rollback_reference` required when `smoke_status=failed`.
  - `preview_run_id` required for all production records.

## Entity: PlatformAdminAction

- **Purpose**: Auditable `kolops` command execution record.
- **Core fields**:
  - `id` (string/uuid)
  - `actor_user_id` (string/uuid)
  - `command_group` (`org | policy | keys | entitlement | usage | audit`)
  - `target_scope` (string)
  - `request_payload_digest` (string)
  - `result_status` (`success | failed`)
  - `result_message` (string)
  - `executed_at` (datetime)
- **Validation rules**:
  - Command group must be known enum.
  - Actor and target scope required.
  - Sensitive payload fields must be redacted in logs/audit output.

## Relationships (high-level)

- `OrganizationMember` belongs to one org; org has many members.
- `PolicySnapshot`, `SharedProfile`, `UsageAuditRecord`, `BillingAccessState` belong to one org.
- `ValidationContext` scopes generated `OrganizationMember`, `PolicySnapshot`, and `UsageAuditRecord` test fixtures.
- `StagingPersona` maps to one org baseline and one policy variant.
- `TestAccessSession` generates one or more `PlatformAdminAction` or `UsageAuditRecord` entries.
- `ReleaseEvidenceRecord` links preview/prod pipeline runs for a release.
