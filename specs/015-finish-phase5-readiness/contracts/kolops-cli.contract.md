# `kolops` CLI Contract (Phase 5B)

## Scope

`kolops` covers platform admin actions only (not deploy orchestration):

- org lifecycle and policy actions
- entitlement/tier actions
- key rotation actions
- usage/audit reporting actions

Current implementation behavior is deterministic by default (no network calls required).
An optional live audit sink is supported through environment configuration for non-production verification.

## Command Groups

## 1) `kolops org`

- `kolops org create --name <name> [--owner <user-id>]`
- `kolops org update --org <id> [--name <name>] [--status <active|disabled>]`
- `kolops org disable --org <id> --reason <text>`

Expected behavior:
- Validate actor authorization.
- Emit auditable action record.
- Return deterministic success/failure output.
- `create` defaults owner to `unassigned` when `--owner` is omitted.

## 2) `kolops policy`

- `kolops policy set --org <id> --file <policy.json>`
- `kolops policy publish --org <id>`
- `kolops policy get --org <id> [--version <n|latest>]`

Expected behavior:
- Validate policy schema before apply.
- Enforce publish authorization.
- Return policy version and status metadata.

Current deterministic output fields:
- `set` returns `status: draft`
- `publish` returns `status: published`
- `get` defaults `--version latest`

## 3) `kolops keys`

- `kolops keys rotate --org <id> --provider <name> [--label <text>]`
- `kolops keys list --org <id>`
- `kolops keys disable --org <id> --provider <name> --key-id <id>`

Expected behavior:
- Never print raw secret material.
- Emit key action audit record.
- Return key metadata only.

Current deterministic output fields:
- `rotate` returns `secret: "[redacted]"`
- deterministic `keyId` is derived from `org` + `provider`

## 4) `kolops entitlement`

- `kolops entitlement set-tier --user <id> --tier <community|personal|enterprise>`
- `kolops entitlement get --user <id>`

Expected behavior:
- Validate tier value.
- Enforce elevated authorization.
- Emit entitlement-change audit record.

## 5) `kolops usage`

- `kolops usage report --org <id> --period <YYYY-MM>`

Expected behavior:
- Return deterministic table/json output.
- Include generated-at timestamp and scope metadata.

## 6) `kolops audit`

- `kolops audit export --org <id> --from <datetime> --to <datetime> [--format json|csv]`

Expected behavior:
- Enforce valid time window (`from <= to`).
- Return export artifact path/handle and metadata.

## Common Exit/Output Contract

- Exit code `0`: success
- Exit code `1`: validation or authorization failure
- Exit code `2`: transient service/dependency failure

All command responses MUST include:

- `command`
- `status`
- `timestamp`
- `request_id` (if available)
- `target_scope`

Additional response fields used by current implementation:
- `message` (`ok` or error code)
- `data` (command-specific deterministic payload on success)

## Actor/Auth Contract

- Actor identity is required. Missing/blank actor maps to `AUTH_REQUIRED` (exit `1`).
- Allowed actor roles: `operator | admin`; other roles map to `AUTH_FORBIDDEN` (exit `1`).

## CLI Environment Contract

- `KOLOPS_ACTOR_USER_ID` (required for successful execution)
- `KOLOPS_ACTOR_ROLE` (`operator | admin | viewer`)
- `KOLOPS_REQUEST_ID` (optional)

Optional live audit forwarding (non-production verification path):
- `KOLOPS_AUDIT_API_BASE_URL`
- `KOLOPS_AUDIT_BEARER_TOKEN`
- `KOLOPS_ORG_ROLE_HEADER` (defaults to `admin`)

When live forwarding is enabled and the audit endpoint is unavailable, CLI returns `TRANSIENT_AUDIT_SINK_UNAVAILABLE` with exit `2`.

## Audit Contract

Every mutating command MUST emit an audit record containing at least:

- actor identity
- command group
- action name
- target scope
- execution timestamp
- result status
- redacted error/details (if failed)
