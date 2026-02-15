# Feature Specification: Phase 2 Cloud Policy Packs

**Feature Branch**: `012-cloud-policy-packs`
**Created**: 2026-02-14
**Status**: Draft
**Input**: User description: "create now a spec for the next phase"

## User Scenarios & Testing *(mandatory)*


### User Story 1 - Org admin publishes enforceable policy pack (Priority: P1)

As an enterprise admin, I can publish an organization policy pack and have enrolled desktop apps apply it consistently.

**Why this priority**: Central policy control is the core value of this phase and the main enterprise buying reason.

**Independent Test**: Publish a policy pack for a test org, trigger client sync, and verify the same effective restrictions appear on multiple enrolled clients.

**Acceptance Scenarios**:

1. **Given** an org has an active policy pack, **When** an enrolled desktop client syncs policy, **Then** the client applies all valid enforced settings from that pack.
2. **Given** the policy pack is updated, **When** clients perform their next sync, **Then** clients apply the new version and mark the previous version as superseded.
3. **Given** a user belongs to no org policy scope, **When** policy sync is attempted, **Then** the app remains in non-cloud-policy mode without blocking baseline usage.

---

### User Story 2 - End users understand enforced settings (Priority: P2)

As an end user, I can clearly see which settings are enforced by policy and why I cannot change them.

**Why this priority**: Visibility prevents confusion, support tickets, and accidental policy workarounds.

**Independent Test**: Load a client with enforced policy fields and verify blocked controls, reason labels, and effective values are visible without developer tools.

**Acceptance Scenarios**:

1. **Given** a setting is policy-enforced, **When** the settings screen renders, **Then** that control is visibly restricted and shows an explanatory policy indicator.
2. **Given** a user attempts to modify an enforced setting, **When** they interact with the control, **Then** the app prevents the change and preserves the policy value.
3. **Given** policy no longer enforces a field, **When** the next policy sync is applied, **Then** that control becomes editable again.

---

### User Story 3 - Clients stay reliable during policy service outages (Priority: P3)

As an enterprise user, I can keep working when policy services are temporarily unreachable because the last valid policy is cached and bounded by expiry.

**Why this priority**: Enterprise rollouts require predictable behavior even with intermittent cloud availability.

**Independent Test**: With a previously synced policy, simulate cloud unavailability and verify cached policy remains effective until expiry and degrades predictably afterward.

**Acceptance Scenarios**:

1. **Given** a client has a valid cached policy, **When** cloud policy fetch fails temporarily, **Then** cached policy remains active until its validity window ends.
2. **Given** cached policy has expired and cannot be refreshed, **When** the app reevaluates policy state, **Then** it enters a degraded policy state with clear user-visible diagnostics.
3. **Given** cloud connectivity recovers, **When** sync succeeds, **Then** the app returns to normal cloud-policy state and records updated metadata.

---

### Edge Cases

- Policy payload is structurally valid but contains unsupported fields.
- Policy signature/integrity check fails while payload content appears otherwise valid.
- Client clock skew causes near-boundary policy expiry evaluations.
- User logs out or org membership changes while policy sync is in progress.
- Multiple windows receive policy-updated events simultaneously and race to re-render settings.
- Cloud returns an older policy version than the one currently cached.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST support organization-scoped policy packs that define enforceable app behavior.
- **FR-002**: The system MUST fetch policy packs for eligible signed-in org users and maintain explicit policy source state (`none`, `cloud`, or cached/degraded cloud state).
- **FR-003**: The system MUST verify policy integrity before applying any fetched policy.
- **FR-004**: The system MUST reject invalid or unverifiable policy updates and keep the last valid applied policy active when available.
- **FR-005**: The system MUST persist the last valid policy pack and associated metadata required for offline continuity.
- **FR-006**: The system MUST apply enforced policy values consistently to effective runtime settings.
- **FR-007**: The system MUST prevent users from editing policy-enforced settings while policy is active.
- **FR-008**: The system MUST clearly indicate in the UI which settings are enforced and why.
- **FR-009**: The system MUST emit settings-change signals after policy application so all active windows stay consistent.
- **FR-010**: The system MUST show policy diagnostics including source, version, last update time, expiry, and enforcement summary.
- **FR-011**: The system MUST allow export of a policy diagnostic artifact that excludes secrets and user content.
- **FR-012**: The system MUST continue baseline non-policy functionality when policy services are unavailable or policy eligibility is absent.
- **FR-013**: The system MUST bound cached policy usage with an explicit validity window and move to a degraded state once expired.
- **FR-014**: The system MUST recover from degraded policy state automatically after a subsequent valid sync.
- **FR-015**: The system MUST keep policy application deterministic for identical policy input and client state.

### Key Entities *(include if feature involves data)*

- **PolicyPack**: Organization policy definition containing enforceable constraints, defaults, metadata version, and validity window.
- **PolicyState**: Client-side representation of policy source, applied version, last update timestamp, expiry, and diagnostics status.
- **PolicyEnforcementRecord**: Per-setting outcome describing whether a field is enforced, normalized, rejected, or user-editable.
- **PolicyDiagnosticsExport**: User-shareable support artifact containing policy metadata and enforcement outcomes without secrets or user content.

### Assumptions

- This phase targets desktop policy consumption/application behavior and diagnostics in `kolboo`.
- Login/org enrollment baseline from spec `011-login-org-enrollment` is already available.
- Enterprise policy service provides policy payload plus integrity metadata required for trust validation.
- Baseline OSS/non-account usage remains available regardless of cloud policy status.

### Dependencies

- Existing account/org state to determine policy eligibility.
- Existing settings normalization and runtime sync pipeline.
- Existing multi-window settings refresh/event mechanisms.
- Cloud policy service availability for policy retrieval.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In validation scenarios, 100% of enrolled clients apply the same effective policy constraints for the same policy version.
- **SC-002**: 95% of successful policy sync operations complete and apply within 30 seconds of manual refresh initiation.
- **SC-003**: 100% of tested enforced settings display a visible enforcement indicator and prevent conflicting user edits.
- **SC-004**: 100% of tested invalid/unverifiable policy payloads are rejected without corrupting last known valid policy state.
- **SC-005**: During simulated cloud outages inside the validity window, 100% of tested clients continue using cached policy without losing baseline app functionality.
- **SC-006**: In pilot support runs, median time to identify policy-related misconfiguration drops by at least 40% using the policy diagnostics export.
