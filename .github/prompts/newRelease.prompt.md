---
agent: agent
---

You are helping prepare and publish a new Kolboo release.

Goal: determine the new release version + changelog fragment under `changelog.d/`, verify the version was updated everywhere it must be (including UI), ensure nothing obvious is missed, then create and push the git tag to trigger the release workflow.

## 0) Safety + context

- Work in the repo root.
- Do not run tests/builds unless I explicitly ask.
- Prefer small, reviewable commits.

## 1) Find the new changelog fragment

1. Check `git status --porcelain`.
2. Identify the new/modified changelog fragment(s) in `changelog.d/`.
   - If **no** new `changelog.d/*.md` exists (excluding `changelog.d/README.md`), stop and ask me to add one.
   - If **multiple** new fragments exist, list them and ask whether we’re releasing all of them now.

## 2) Determine the intended version

Use the version from `app/package.json` as the canonical version.

- Read `app/package.json` and extract `version` (call it `$VERSION`).
- Expected tag name is `v$VERSION`.

## 3) Verify version is updated everywhere (must-pass)

Confirm these files all match `$VERSION`:

- `app/package.json` → `version`
- `app/src-tauri/tauri.conf.json` → `version`
- `app/src-tauri/Cargo.toml` → `[package].version`

Also verify the UI displays the new version:

- `app/src/App.tsx` footer version label shows `v$VERSION` (and ideally links to the correct releases page).
   - Do **not** change the footer URL (keep it pointing at the general Releases page). Only update the displayed version text.

Run the repository’s version consistency script:

- `node .github/scripts/verify-versions.mjs`
  - If it fails, fix versions until it passes.

## 4) Search for old version strings (sanity sweep)

1. Identify the previous version by looking at git history (e.g., last tag or last release commit).
2. Search the repo for the previous version string and update any remaining UI/docs references.
   - Avoid touching lockfiles unless needed.

## 5) Release notes behavior

Release notes are derived from `changelog.d/`.

- The release workflow will generate release notes from the fragment(s).
- The generator must output changelog fragment content verbatim.
- Do **not** add or commit `RELEASE_NOTES.md` (it is a generated artifact).

## 6) Repo hygiene checks (quick)

- Confirm `origin` points to `git@github.com:DovieW/kolboo.git`.
- Confirm the working tree is clean _except_ for intentional release changes.
- Confirm `.github/workflows/release.yml` exists and triggers on tags `v*`.

## 7) Commit the release prep

Stage and commit the changes required for the release:

- Commit message convention:
  - `chore(release): v$VERSION`
  - (Alternative acceptable: `release: v$VERSION`)

Do not include unrelated refactors.

## 8) Tag and push

1. Create an annotated tag:
   - `git tag -a v$VERSION -m "Kolboo v$VERSION"`
2. Push the commit and tag:
   - `git push origin <branch>`
   - `git push origin v$VERSION`

If the branch is protected and direct pushes are blocked:

- Create a PR for the release prep commit.
- After merge, tag `v$VERSION` on the merge commit (or on `main/master` HEAD), then push the tag.

## 9) Confirm release workflow started

- After pushing the tag, confirm GitHub Actions has a running workflow for the tag.
- If it didn’t start, check tag pattern, workflow permissions, and repo default branch settings.

## Output format

When you finish, summarize:

- The changelog fragment(s) used
- `$VERSION` and the tag name
- Which files were updated/verified
- The exact commit SHA and tag SHA
- Whether the release workflow started successfully
