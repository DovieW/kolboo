# DRY_PLAN.md — Simple DRY scan plan (Kolboo)

This repo already has a pretty good DRY posture. The goal of this plan is to find the *few* copy/paste (and “almost copy/paste”) areas that still create bug risk or slow edits.

This plan is intentionally **simple and reproducible**:

- No embeddings
- No AST parsing
- No “smart similarity” tooling

## 1) Repo prep (what I looked at)

### Main languages

- Rust (`.rs`) — Tauri backend
- TypeScript (`.ts`) / TSX (`.tsx`) — React/Vite frontend

### Top 5 largest source directories (application code)

(Measured by file size, excluding build artifacts like `node_modules`, `target*`, `coverage`.)

1. `app/src/components/` (TSX UI)
2. `app/src/lib/` (shared UI logic + Tauri wrapper layer)
3. `app/src-tauri/src/commands/` (Tauri command layer)
4. `app/src-tauri/src/pipeline/` (core pipeline state machine / glue)
5. `app/src-tauri/src/stt/` (STT providers)

### Ignore list

I ignored (or treated as “noise”) these areas:

- `**/node_modules/**`
- `**/target/**`, `**/target-*/**`, `**/target-rust-analyzer/**`, `**/target-ci/**`
- `**/dist/**`, `**/build/**`
- `**/coverage/**`
- `**/.git/**`
- lockfiles
- generated files like `**/*.generated.*`
- minified blobs

And I focused on **application code first** (tests were only checked when they helped explain a pattern).

## 2) Find duplicates (simple methods)

I used two simple approaches:

### A) Exact duplicate blocks (token/text clones)

I ran `jscpd` (clone detector) over the app code and excluded tests to keep results high-signal.

Notes:

- This is *not* an AST parser; it’s a straightforward clone detector.
- In this repo it found only a handful of exact clones in non-test code, which is a good sign.

### B) “Almost duplicates” (manual grep + review)

Because the most important duplication here is often “same structure, different values” (especially provider code and settings parsing), I used simple searches to find repeated patterns:

- repeated constructor patterns (e.g., `reqwest::Client::builder()`)
- repeated settings parsing / clamping patterns
- repeated string/path formatting patterns
- repeated event emission patterns

## 3) Group and rank

I grouped findings into “duplicate groups” and ranked by:

1) number of repeats, then
2) size of the repeated block, then
3) closeness to core/business logic (pipeline/providers/settings)

## 4) What I did NOT do

- No AST parsing
- No embeddings / semantic similarity
- No automatic refactor commits
- No changes to CI/gating

## 5) How to re-run the same process

### A) Re-run exact clone scan (recommended)

From `app/`:

- Run `pnpm -s dlx jscpd src "src-tauri\\src" --min-lines 8 --min-tokens 70 --format "typescript,rust" --ignore "**/node_modules/**,**/target/**,**/target-*/**,**/dist/**,**/build/**,**/coverage/**,**/*.generated.*,**/*.test.*,**/__tests__/**,**/tests/**" --reporters "consoleFull"`

### B) Re-run the “almost duplicate” searches

- `rg -n "reqwest::Client::builder\(" app/src-tauri/src`
- `rg -n "get_settings_store\(" app/src-tauri/src`
- `rg -n -F "trimmed.rsplit(['\\\\', '/'])" app/src-tauri/src`
- `rg -n "errorToMessage\(" app/src`

Then spot-check the hits and update `DRY_REPORT.md`.
