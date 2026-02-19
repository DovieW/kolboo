# Feature Specification: Finish Remaining Phase 5/5A/5B Readiness

**Feature Branch**: `015-finish-phase5-readiness`
**Created**: 2026-02-18
**Status**: Draft
**Input**: User description: "Finish up the remaining Phase 5 enterprise dashboard foundation and Phase 5A SaaS admin testing path readiness work with verification evidence, and include Phase 5B platform admin CLI readiness."

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Complete enterprise admin workflows (Priority: P1)

As an organization admin, I can complete all core enterprise dashboard workflows (member management, policy management, usage/audit review, shared profile management, and billing management) from one place so I can operate my team without engineering support.

**Why this priority**: This is the core value of Phase 5. Without these workflows, enterprise admins cannot fully manage their organization.

**Independent Test**: Can be fully tested by signing in as an admin in a seeded non-production organization and completing each workflow end-to-end, including success and permission-denied paths.

**Acceptance Scenarios**:

1. **Given** an admin is signed in to an enterprise organization, **When** they perform member lifecycle actions (invite, role update, revoke/restore as permitted), **Then** member state updates are visible in the dashboard and auditable.
2. **Given** an admin edits organizational policy and publishes it, **When** the publish completes, **Then** the dashboard shows the new active policy version and publication metadata.
3. **Given** an admin applies usage/audit filters, **When** they submit the filters, **Then** only records matching the selected time window and filter criteria are returned.
4. **Given** an admin manages shared profiles, **When** they create/update/archive a shared profile, **Then** the resulting shared profile catalog reflects the change and is available to authorized team members.
5. **Given** an admin opens billing management, **When** they request billing actions, **Then** they can access current billing state and billing management entry points authorized for their organization.

---

### User Story 2 - Deterministic non-production admin testing path (Priority: P2)

As a SaaS operator, I can reliably validate dashboard features in local, preview, and staging environments using deterministic personas and reversible setup/reset flows so each release candidate can be tested quickly and repeatedly.

**Why this priority**: This removes release friction and enables confident validation without requiring real customer tenants.

**Independent Test**: Can be fully tested by creating a non-production test context, executing defined validation scenarios across required personas, and resetting to a clean state with no cross-context contamination.

**Acceptance Scenarios**:

1. **Given** a non-production environment and context key, **When** an operator runs seed, **Then** a deterministic baseline org dataset is created for that context.
2. **Given** seeded data exists for one context, **When** reset is executed for that same context, **Then** only context-tagged data is removed and unrelated contexts remain unchanged.
3. **Given** staging personas are defined (BYOK org, managed org, mixed policy org), **When** scripted smoke checks run, **Then** each persona scenario reports pass/fail evidence for key admin workflows.
4. **Given** urgent verification is needed, **When** an operator follows the documented database-first fallback, **Then** the operation is reversible, scoped, and logged in runbook notes.

---

### User Story 3 - Production-safe test access and release evidence (Priority: P3)

As a security-conscious operator, I can use temporary non-production test access while knowing production is hard-blocked and all elevated actions are audited, so testing speed improves without creating a production backdoor.

**Why this priority**: This protects production integrity and compliance posture while supporting fast iteration in non-production.

**Independent Test**: Can be fully tested by executing test-access flows in non-production, verifying audit evidence is recorded, and confirming production returns a deterministic hard-stop response.

**Acceptance Scenarios**:

1. **Given** a dev/preview/staging environment, **When** an authorized operator starts a short-lived test-access session, **Then** the session is time-bounded and audit evidence is created.
2. **Given** production environment, **When** any test-access or fixture route is requested, **Then** the request is rejected with a deterministic non-production-only error.
3. **Given** a release candidate progresses from preview to production, **When** preview validation and production smoke execute, **Then** release evidence captures run identifiers, status, and rollback reference when applicable.

---

### User Story 4 - Platform admin operations through kolops (Priority: P3)

As an internal operator, I can perform audited platform administration actions through a single `kolops` command-line workflow so operational tasks are consistent, fast, and role-controlled.

**Why this priority**: This completes Phase 5B readiness and reduces operational risk from ad hoc manual procedures.

**Independent Test**: Can be fully tested by running command groups for org, policy, keys, entitlements, usage, and audit export in non-production with role checks and audit evidence verification.

**Acceptance Scenarios**:

1. **Given** an authorized operator session, **When** they run organization lifecycle commands, **Then** the requested org state change is applied and audit evidence is recorded.
2. **Given** an authorized operator session, **When** they run policy, key-rotation, and entitlement commands, **Then** each command enforces scope/permissions and returns deterministic success or failure output.
3. **Given** reporting commands are executed, **When** usage/audit export is requested for a defined period, **Then** export output is generated with traceable execution metadata.

---

### Edge Cases

- A user with viewer permissions attempts admin-only actions (policy publish, billing management, member role changes).
- Seed is requested repeatedly for the same context key.
- Reset is requested for a context that has no seeded data.
- Two admins edit the same policy draft concurrently.
- A test-access session expires during an active admin workflow.
- Preview evidence exists but required smoke fields are incomplete.
- Production smoke fails after manual approval and requires rollback evidence before closeout.
- An operator runs an invalid or partially scoped `kolops` command.
- A `kolops` command is attempted without required authorization context.

## Requirements _(mandatory)_

### Functional Requirements

- **FR-001**: The system MUST provide complete member administration capabilities for organization admins, including invite, role update, and status management with role-based access enforcement.
- **FR-002**: The system MUST provide policy draft and publish workflows with visible version history metadata and deterministic conflict handling.
- **FR-003**: The system MUST provide usage and audit querying with filterable time windows and category/actor criteria.
- **FR-004**: The system MUST provide shared profile management for enterprise organizations, including create, update, archive, and visibility to authorized members.
- **FR-005**: The system MUST provide billing management access for authorized admins, including current billing state visibility and secure billing action entry points.
- **FR-006**: The system MUST provide deterministic seed and reset workflows for non-production validation contexts.
- **FR-007**: The system MUST ensure reset operations are context-scoped and do not modify data outside the selected validation context.
- **FR-008**: The system MUST provide and maintain three stable staging personas (BYOK org, managed org, mixed policy org) for scripted smoke validation.
- **FR-009**: The system MUST provide a documented and reversible database-first fallback workflow for urgent non-production verification.
- **FR-010**: The system MUST restrict test-access and fixture capabilities to non-production environments only.
- **FR-011**: The system MUST enforce short-lived test-access sessions and record an audit event for each session start and end.
- **FR-012**: The system MUST produce release evidence records that include preview validation linkage, production smoke outcome, and rollback reference when a smoke failure occurs.
- **FR-013**: The system MUST ensure no production workflow depends on permanent test backdoors.
- **FR-014**: The system MUST provide a platform admin CLI surface for organization lifecycle, policy actions, key rotation actions, entitlement/tier actions, and usage/audit reporting actions.
- **FR-015**: The system MUST enforce authorization and scope validation for every platform admin CLI action.
- **FR-016**: The system MUST emit auditable execution records for platform admin CLI actions, including actor identity, command category, target scope, timestamp, and result status.
- **FR-017**: The system MUST provide deterministic operator feedback for successful and failed CLI actions so operators can resolve issues without inspecting internal service logs.
- **FR-018**: The system MUST include enterprise desktop-app integration touchpoints for this phase, including role-aware enterprise UI surfaces (Phase 5) and non-production test persona context indicators when impersonation/test-access is active (Phase 5A).

### Assumptions

- Enterprise dashboard users authenticate through existing organization-authenticated sessions.
- Non-production includes local, preview, and staging environments.
- Billing management may redirect to an existing external billing surface, provided access is role-restricted and auditable.
- Existing privacy rules remain in force: no transcript/audio/prompt content is required for Phase 5 completion evidence.
- Platform admin CLI is intended for internal/operator use, not end-user desktop usage.

### Key Entities _(include if feature involves data)_

- **Organization Member**: Represents a user in an organization with role, status, and membership lifecycle metadata.
- **Policy Draft/Publication**: Represents organization policy state, version metadata, and publication history.
- **Usage/Audit Record**: Represents content-free operational events, actor context, category, action, and timestamp.
- **Shared Profile**: Represents a reusable team configuration owned by an organization and consumable by authorized users.
- **Billing Access State**: Represents billing plan/status summary and authorized billing action links for an organization.
- **Validation Context**: Represents deterministic non-production test scope identified by a context key.
- **Staging Persona**: Represents a stable test organization profile (BYOK, managed, mixed policy) used for smoke checks.
- **Test-Access Session**: Represents a time-limited non-production elevated session with audit trail metadata.
- **Release Evidence Record**: Represents proof of preview validation, production smoke outcome, approvals, and rollback linkage.
- **Platform Admin Action**: Represents an operator-initiated CLI action with actor, scope, command category, request metadata, and execution result.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: 100% of defined Phase 5 admin workflows (members, policy, usage/audit, shared profiles, billing management) are executable by authorized admins and rejected for unauthorized roles in validation runs.
- **SC-002**: Seed + validation + reset cycle for a non-production context completes within 15 minutes end-to-end for at least 95% of runs.
- **SC-003**: 100% of production deploy attempts include linked preview validation evidence and a recorded production smoke result.
- **SC-004**: 100% of production requests to non-production-only test-access/fixture capabilities are blocked with deterministic non-production-only responses.
- **SC-005**: 100% of non-production test-access sessions generate audit evidence with actor, start time, end/expiry time, and environment.
- **SC-006**: During two consecutive release cycles, operators can execute all required Phase 5/5A validation steps without ad hoc manual database edits outside the documented fallback procedure.
- **SC-007**: 100% of defined Phase 5B command groups (org lifecycle, policy, key rotation, entitlement/tier, usage report, audit export) are executable in non-production with deterministic pass/fail outcomes and audit evidence.
- **SC-008**: At least 90% of routine platform admin tasks in validation runs are completed through `kolops` workflows rather than ad hoc manual steps.
