# Phase 0 Research: Identity-Policy Boundary for Desktop Auth

## Decision 1: Default desktop sign-in flow

- **Decision**: Use browser-based OAuth/OIDC-style flow with PKCE semantics for desktop sign-in (Supabase Auth as current IdP implementation).
- **Rationale**: Avoids collecting/storing user passwords in app context; aligns with modern desktop security posture and enterprise expectations.
- **Alternatives considered**:
  - Password grant in desktop app: rejected due to higher credential-handling risk and weaker long-term posture.
  - Device code flow as default: viable fallback, but not preferred baseline for current product UX.

## Decision 2: Identity vs authorization boundary

- **Decision**: Keep IdP responsible for identity proof; enforce org membership, entitlements, policy, and metering at `api-edge`.
- **Rationale**: Preserves clean separation of concerns and supports both managed and BYOK modes without coupling business policy to IdP specifics.
- **Alternatives considered**:
  - Push org/policy logic into IdP claims only: rejected because policy and billing logic are product-owned and change frequently.
  - Make desktop directly decide entitlement/policy: rejected because centralized control is required for consistency and abuse prevention.

## Decision 3: Session material storage

- **Decision**: Store refresh/session material in OS secure storage only; store non-secret auth state in Tauri settings as needed.
- **Rationale**: Minimizes secret exposure while preserving deterministic app state behavior.
- **Alternatives considered**:
  - Store tokens in settings.json: rejected because settings store is not the right boundary for sensitive credentials.
  - Keep all auth state in memory only: rejected due to poor persistence and user experience.

## Decision 4: Edge token validation requirements

- **Decision**: `api-edge` must validate JWT using JWKS and standard claims (`iss`, `aud`, `exp`; plus robust handling for skew/rotation).
- **Rationale**: Establishes auditable and standards-based acceptance criteria for managed access.
- **Alternatives considered**:
  - Opaque pass-through of bearer token without verification: rejected due to unacceptable authorization risk.
  - Introspection-only dependency per request: rejected for unnecessary latency/coupling as default.

## Decision 5: Token exchange timing

- **Decision**: Token exchange is not mandatory for MVP but becomes recommended once predefined triggers are met.
- **Rationale**: Prevents overbuilding now while preserving a clear and intentional enterprise path.
- **Implementation note**: Desktop currently persists a normalized trigger set plus explicit decision, and the session-exchange surface remains a readiness placeholder rather than an active dependency.
- **Alternatives considered**:
  - Implement token exchange immediately: rejected for added scope before trigger conditions are present.
  - Never implement token exchange: rejected because it blocks clean multi-IdP abstraction and stronger revocation semantics.

## Decision 6: Trigger criteria for token exchange adoption

- **Decision**: Trigger token exchange when any of these become true:
  1. WorkOS/SSO multi-IdP support is required,
  2. revocation/kill-switch semantics are required,
  3. edge-signed org/tier claims are needed to reduce repeated lookups,
  4. desktop needs rapid IdP-agnostic behavior.
- **Rationale**: Converts architectural debate into objective gating rules.
- **Implementation note**: The desktop trigger evaluator now returns `direct_idp_token` unless at least one trigger bit is enabled.
- **Alternatives considered**:
  - Time-based trigger (e.g., “after N months”): rejected as arbitrary.
  - Volume-only trigger: rejected because security/compliance concerns can precede scale.

## Decision 7: Tier behavior boundary

- **Decision**: Keep Community/BYOK inference path login-optional and non-proxied; enforce auth+policy for managed operations only.
- **Rationale**: Preserves product promise for BYOK while protecting managed/billed pathways.
- **Alternatives considered**:
  - Require login for all modes: rejected because it adds friction and violates BYOK intent.
  - Allow managed operations without edge authorization: rejected due to billing and policy control gaps.

## Implementation Decision Log (Token-Exchange Trigger Reviews)

Use this log to record each trigger-review checkpoint and the resulting architecture decision.

| Review Date | Trigger(s) Observed | Decision (`direct_idp_token` \| `adopt_token_exchange`) | Owner | Notes / Follow-ups |
| ----------- | ------------------- | ---------------------------------------------------------- | ----- | ------------------ |
| 2026-04-22  | None                | direct_idp_token                                            | Desktop auth boundary | PKCE desktop flow shipped; `token_exchange_trigger_set` defaults all false and `/v1/session/exchange` remains a placeholder readiness path. |

## Implementation Notes (2026-04-22)

- Browser-based PKCE sign-in is now the desktop default, using a loopback callback on `127.0.0.1` and avoiding in-app password collection.
- Secure logout/startup refresh behavior is wired through the existing secrets lifecycle and cached auth/policy snapshot invalidation.
- Token-exchange readiness is intentionally split into:
  - a frontend evaluator for trigger booleans,
  - persisted normalized trigger state in settings,
  - a backend placeholder session-exchange command/contract for future activation.
