# Public repository publication audit

**Audit date:** 2026-08-09

**Audited commit:** `59bdd36` (audit documentation follows in a later commit)

**Visibility during audit:** Private

This audit records technical publication evidence. It does not authorize changing visibility; the company launch checklist remains the controlling gate.

## Full-history credential scans

The complete 700-commit history (about 14.4 MB of decoded content) was scanned with two independent engines:

| Scanner | Version | Scope | Result |
| --- | --- | --- | --- |
| Gitleaks | 8.30.1 | Git history, `--all`, 100% redaction | Eight duplicate detector hits, all adjudicated false positives |
| TruffleHog | 3.96.0 | Git history, verified/unknown/unverified detectors | Seven “verified” Lob hits, all adjudicated false positives |

Gitleaks’ AWS detector matched JWT query parameters on four historical `private-user-images.githubusercontent.com` screenshot URLs in `README.md` at commit `32515fd0`. They are GitHub-generated image delivery URLs, not AWS access credentials or Kolboo secrets. The duplicates were decoder/occurrence duplicates.

TruffleHog’s Lob detector interpreted 40-character Rust test-function identifiers beginning with `test_` as Lob test-mode API keys. Source metadata confirmed every finding was a test symbol in provider/contract tests, not a string credential. No unknown or unverified result remained.

No private updater key or password was written to the repository. The updater private values were sent directly to GitHub Actions secrets; only `app/src-tauri/updater.pubkey` is committed.

## Infrastructure and binary review

Historical URL-domain extraction found loopback/test/example domains and the intended public service origin `kolboo.dovie.dev`. Supabase references were placeholders (`example.supabase.co` and `your-project-ref.supabase.co`). No concrete Supabase project host or private Worker/Pages hostname was found.

The only historical blob at or above 1 MiB was `app/src-tauri/icons/icon.icns` at about 1.6 MiB, an expected application icon. No archives, installers, database dumps, audio recordings, coverage output, Rust targets, `node_modules`, environment files, or customer artifacts were found as oversized history blobs.

The locked production JavaScript inventory contained 200 package entries and no missing/unknown license metadata. Cargo metadata contained 724 packages; the two entries without a Cargo license field were the in-repository `xtask` helper and pinned `tauri-nspanel`. The latter ships upstream Apache-2.0 and MIT license files, which are called out in `THIRD_PARTY_NOTICES.md`.

## Copyright and provenance

`git shortlog -sne --all` identified two human author groups: Kingston Kuan (the inherited Tambourine history) and Dovie Weinstock, plus automated Dependabot, Ralph, and GitHub Actions commits. No contributor assignment or CLA was found. Therefore:

- the project must not claim a sole copyright holder;
- `NOTICE` credits the upstream fork and all contributors;
- commercial licensing can cover only rights Kol Software owns or is separately authorized to license; and
- third-party dependency and font notices must be preserved.

## Repository governance review

- Root license: AGPL-3.0, recognized by GitHub.
- Contribution policy: contributions remain AGPL-3.0 and do not silently transfer copyright.
- Security reporting: private email and GitHub Security Advisories are documented; public issues are prohibited for vulnerabilities.
- Issue intake: structured bug and feature forms exist and direct security reports away from public issues.
- Actions: third-party actions are immutable SHA pins; audit jobs have read-only contents permission.
- Releases: Windows-only, updater-signed, and fail-closed on missing Authenticode credentials; ordinary development builds remain unsigned and available.
- Metadata: description and topics identify Windows, Tauri, Rust, dictation, and speech-to-text without making unsupported cross-platform claims.

## Remaining publication blockers

- Acquire and provision the Windows Authenticode certificate.
- Publish legal pages at stable unauthenticated URLs and wire those links into app/checkout.
- Activate and rehearse the real Merchant-of-Record flow.
- Complete the exact-SHA Windows desktop-to-operator support rehearsal.
- Re-run both history scanners on the final visibility-change commit and retain redacted reports outside the public repository.
