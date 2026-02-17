# Feature Specification: Phase 3 Managed Inference Proxy

**Feature Branch**: `013-managed-inference-proxy`
**Created**: 2026-02-15
**Status**: Draft
**Input**: User description: "Implement the entirety of Phase 3 managed inference proxy from the readiness plan, including any additional required scope not explicitly listed."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Personal user runs without API keys (Priority: P1)

A paid personal user signs in and can immediately use voice features without manually creating or entering provider API keys.

**Why this priority**: This is the core value proposition for paid personal plans and the main conversion driver.

**Independent Test**: Can be fully tested by signing in as a personal user on a clean install and successfully completing voice workflows without entering provider credentials.

**Acceptance Scenarios**:

1. **Given** a signed-in personal user with active entitlement, **When** they submit a voice or rewrite request, **Then** the request is accepted and completed without requiring local provider keys.
2. **Given** a signed-in personal user near usage limits, **When** they continue using managed features, **Then** the app shows remaining usage clearly and warns before hard limits are reached.
3. **Given** a signed-in personal user over quota, **When** they submit another managed request, **Then** the request is blocked with a clear actionable message and downgrade/fallback guidance.

---

### User Story 2 - Enterprise admin chooses inference mode (Priority: P1)

An enterprise admin can choose org-managed inference or org BYOK, and users in that org get consistent behavior that matches the selected mode.

**Why this priority**: Enterprise adoption depends on explicit operational control and predictable behavior by policy.

**Independent Test**: Can be fully tested by switching an enterprise org between managed and org BYOK modes and validating user request paths and user-facing messaging in each mode.

**Acceptance Scenarios**:

1. **Given** an enterprise org set to managed mode, **When** a member submits inference requests, **Then** requests are handled as managed usage and counted against org-managed quotas.
2. **Given** an enterprise org set to org BYOK mode, **When** a member submits inference requests, **Then** requests proceed as org-provided usage and are not treated as managed-credit consumption.
3. **Given** an admin changes inference mode, **When** members use the app after sync, **Then** member behavior reflects the updated mode without manual reconfiguration.

---

### User Story 3 - Operations team can control abuse/cost risk (Priority: P2)

An operator can prevent abuse, contain spend, and diagnose incidents without exposing user content.

**Why this priority**: Managed inference is financially and operationally risky without protections and observability.

**Independent Test**: Can be fully tested by simulating invalid auth, quota overage, and traffic spikes, then confirming deterministic denials, cost protection behavior, and content-safe telemetry.

**Acceptance Scenarios**:

1. **Given** a burst of suspicious or excessive traffic, **When** requests exceed protection thresholds, **Then** the system applies controls and returns deterministic, user-safe errors.
2. **Given** an incident involving managed inference failures, **When** operators investigate, **Then** they can identify affected tenants and failure classes using metadata-only traces.

---

### User Story 4 - User continuity during outages (Priority: P2)

Users retain a predictable experience when cloud dependencies are partially unavailable.

**Why this priority**: Reliability and trust require graceful behavior instead of silent failure.

**Independent Test**: Can be fully tested by simulating upstream or gateway failures and confirming graceful fallback paths and user guidance.

**Acceptance Scenarios**:

1. **Given** managed inference is temporarily unavailable, **When** a user has a valid non-managed path configured, **Then** the app offers or applies a fallback path with clear notice.
2. **Given** managed inference is temporarily unavailable and no fallback path exists, **When** a request is submitted, **Then** the user receives a clear recovery message and next steps.

### Edge Cases

- Enterprise admin switches inference mode during active user sessions.
- Token expires mid-request or during streamed response.
- Quota service is delayed or temporarily unreachable while auth is still valid.
- Provider upstream accepts request but returns delayed partial failures.
- Duplicate retries could double-count usage if idempotency protections are missing.
- Single tenant traffic spike should not degrade unrelated tenants.
- Regional degradation should not cause total outage where alternate route is available.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST support two inference modes for eligible signed-in users: managed mode and BYOK mode.
- **FR-002**: The system MUST route personal-tier signed-in inference requests through managed mode by default.
- **FR-003**: The system MUST allow enterprise orgs to select inference mode at the org level (managed or org BYOK).
- **FR-004**: The system MUST enforce org-selected inference mode for all active members after policy/entitlement sync.
- **FR-005**: The system MUST validate user/session authorization before processing managed inference requests.
- **FR-006**: The system MUST enforce entitlement and usage limits before executing managed inference requests.
- **FR-007**: The system MUST return deterministic failure categories for denied/failed managed requests (at minimum: unauthorized, ineligible, over-quota, temporarily unavailable).
- **FR-008**: The system MUST meter managed inference usage per subject and/or organization with idempotent counting semantics.
- **FR-009**: The system MUST provide users an in-app view of managed usage progress and limit proximity.
- **FR-010**: The system MUST notify users before hitting hard usage limits using configurable threshold warnings.
- **FR-011**: The system MUST hard-stop managed requests when applicable limits are exceeded.
- **FR-012**: The system MUST ensure org BYOK requests are represented separately from managed-credit consumption.
- **FR-013**: The system MUST preserve strict content privacy boundaries: no transcript, prompt text, OCR content, or raw audio is persisted by default in cloud logs/telemetry.
- **FR-014**: The system MUST capture operational telemetry for managed inference using metadata-only signals sufficient for incident diagnosis.
- **FR-015**: The system MUST apply abuse and spend protection controls (rate limiting, anomaly safeguards, and hard quota cutoffs) before cost-incurring downstream execution.
- **FR-016**: The system MUST provide graceful user behavior when managed inference is unavailable, including fallback guidance and transparent status messaging.
- **FR-017**: The system MUST preserve existing BYOK behavior for community users and all users not actively in managed mode.
- **FR-018**: The system MUST support enterprise key-rotation continuity so org BYOK updates propagate without requiring end-user manual key entry.
- **FR-019**: The system MUST expose operators to tenant-scoped health and failure visibility without exposing user content.
- **FR-020**: The system MUST ensure that temporary observability outages do not block inference processing.

## Assumptions

- Personal plan includes managed inference quotas and associated user-facing usage visibility.
- Enterprise managed quotas are configurable per org contract.
- Enterprise org BYOK remains a first-class mode and is never forced into managed mode.
- Provider/model allowlists are governed by existing policy infrastructure.
- No new content retention policy is introduced in this phase.

## Dependencies

- Existing account authentication and entitlement sync must remain available.
- Existing policy distribution and org settings enforcement must remain available.
- Provider integrations required for voice and rewrite flows must remain operational.
- Existing support diagnostics surface must remain available for metadata-only troubleshooting.

### Key Entities *(include if feature involves data)*

- **Inference Mode Selection**: Declares whether a user/org should run managed inference or BYOK behavior.
- **Managed Entitlement State**: Captures eligibility to use managed inference plus relevant validity window.
- **Usage Counter**: Tracks managed usage totals against time-bounded limits for users and/or organizations.
- **Quota Policy**: Defines allowed usage ceilings, warning thresholds, and hard-stop behavior.
- **Managed Request Record**: Metadata-only lifecycle record for a managed request (status class, timing, tenant scope, non-content diagnostics).
- **Org Key Rotation State**: Represents active and recently rotated org BYOK key states needed for smooth cutover.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: At least 95% of signed-in personal users can complete first managed inference request in under 2 minutes from app launch without entering API keys.
- **SC-002**: 100% of over-quota managed requests are denied with deterministic, user-actionable messaging.
- **SC-003**: In enterprise pilot orgs, inference mode changes are reflected for active members within 5 minutes of policy/entitlement sync.
- **SC-004**: 99% of managed requests are metered exactly once (no double-counting) during normal and retry conditions.
- **SC-005**: During simulated managed-path outages, at least 95% of affected users receive a clear fallback or recovery path without app crash.
- **SC-006**: 100% of sampled operational telemetry for this feature excludes transcript/audio/prompt/OCR content and raw credentials.
- **SC-007**: Support can identify impacted tenant, failure class, and time window for a managed incident within 10 minutes using telemetry and request metadata.
