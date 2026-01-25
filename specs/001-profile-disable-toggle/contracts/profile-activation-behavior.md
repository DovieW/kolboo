# Contract: Profile Activation Behavior

This contract describes how profiles are considered for activation.

## Eligibility rule

A profile is eligible for activation only if:

- It matches the foreground program according to `program_paths`, AND
- `disabled !== true`

## Immediate deactivation

If a profile is currently active and the user toggles `disabled` to `true`:

- The system MUST immediately stop applying that profile.
- The system MUST fall back to either:
	- no profile applies, OR
	- the next eligible profile (if there is a deterministic rule for that in the existing system).

## Reset profile behavior (rename only)

The action labeled **Reset profile** MUST:

- Reset per-profile override fields back to baseline/inherit values.
- NOT delete the profile.
- NOT change `program_paths`.
- NOT change `disabled`.
