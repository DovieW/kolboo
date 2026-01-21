# DRY_REPORT.md — Duplicate / Repeated Logic Report (Kolboo)

Generated for you, Dovie, on **2026-01-21**.

This report focuses on **repeated logic** that is likely to cause drift/bugs or slow edits. It includes both:

- **Stage 1**: token/text clone detection (jscpd)
- **Stage 2**: lightweight TS AST normalization similarity (to catch “same logic, different names”)

Machine-readable clusters live in `dry_clusters.json`.

## Repo inventory + guardrails

### Languages

- TypeScript/TSX (React/Vite): `app/src/**`
- Rust (Tauri backend): `app/src-tauri/src/**`

### Approx LOC by top-level folder (code-ish files)

From a quick scan excluding build/artifact dirs:

| Folder | Files | Lines |
|---|---:|---:|
| `app/` | 2335 | 130,885 |
| `docs/` | 28 | 5,109 |
| `.github/` | 27 | 1,874 |

### Ignore list used

- `**/node_modules/**`
- `**/dist/**`
- `**/build/**`
- `**/target/**`
- `**/coverage/**`
- `**/tmp/**`
- `**/.git/**`
- `**/*.generated.*`

### Priority modules (where DRY actually matters)

- `app/src/lib/**` (shared logic, Tauri contract, settings normalization)
- `app/src/components/settings/**` (high-churn UI logic)
- `app/src/overlay/**` (consistency-sensitive overlay behavior)
- `app/src-tauri/src/**` (window/payload plumbing, provider request builders)

## Stage 1 results (jscpd, minTokens=70)

- Found **156 exact clone clusters** (token/text duplicates).
- jscpd summary (from its report): **3955 duplicated lines (6.2%)** across **271 files**.

### Top clusters (by impact ≈ lines × occurrences)

| Cluster ID | Format | Occurrences | Lines | Example locations |
|---|---|---:|---:|---|
| `jscpd:f2d616...` | JS/TSX | 2 | 201 | `app/src/components/settings/ProvidersSettings.tsx:400-575` and `:501-701` |
| `jscpd:c9c312...` | JS/TSX | 2 | 136 | `app/src/components/settings/QuickReplaceSettings.tsx:131-240` and `:261-396` |
| `jscpd:76f69f...` | JS/TSX | 2 | 135 | `app/src/components/settings/prompt/RewriteSettingsSection.tsx:355-490` and `:610-744` |
| `jscpd:0cf1c4...` | Rust | 2 | 79 | `app/src-tauri/src/llm/gemini.rs:411-490` and `:529-607` |
| `jscpd:faec70...` | Rust | 2 | 69 | `app/src-tauri/src/commands/config.rs:21-89` and `app/src-tauri/src/llm/prompts.rs:9-77` |

*(Full membership + snippets are in `dry_clusters.json`.)*

## Stage 2 results (TS AST normalization)

This stage scans TS/TSX functions, normalizes identifiers/literals, and clusters near-duplicates using a coarse SimHash bucket.

- Scanned: **75 files**, **212 functions** (only function-ish nodes above the size threshold).
- Found: **31 similarity clusters**.

### Top clusters (most actionable)

| Cluster ID | Occurrences | Why it’s interesting |
|---|---:|---|
| `ts-ast:exact:6e4f97...` | 6 | The `useUpdate*Hotkey` hooks in `app/src/lib/queries.ts` appear to share the same logic structure with only small variation points. |
| `ts-ast:sim:8a9b:12` | 2 | `DataSettings` and `ProvidersSettings` components share large structural similarity (likely repeated UI/validation patterns). |
| `ts-ast:sim:ee4b:5` | 3 | Multiple large, similar logic blocks inside `app/src/overlay/AudioWave.tsx` (candidate for extraction). |

## Ranked refactor candidates (with DRY value scoring)

Scoring rubric (0–5, with coupling penalty 0..−5): Frequency + Complexity + Churn + Safety + Penalty.

| Candidate | Cluster(s) | Recommended abstraction | Effort | Risk | Score (breakdown) |
|---|---|---|---|---|---|
| Hotkey update hooks share same mutation pattern | `ts-ast:exact:6e4f97...` (and related `ts-ast:*:...` in same area) | Extract helper: `makeUpdateHotkeyMutation(kind)` or `createHotkeyUpdater({key})` | S | Low | **17** (F=5, C=2, Churn=4, Safe=5, Penalty=−0) |
| Repeated settings UI sections (providers/data/rewrite/quick-replace) | `jscpd:f2d616...`, `jscpd:c9c312...`, `jscpd:76f69f...`, `jscpd:d59f0c...` | Extract reusable components (e.g. `LabeledSelectWithHint`, `RewritePresetRow`) + parameterize variation points | M | Medium | **12** (F=3, C=3, Churn=4, Safe=3, Penalty=−1) |
| Rust Gemini request construction duplication | `jscpd:0cf1c4...` | Extract builder helper(s) for shared request payload shape | S/M | Medium | **11** (F=2, C=3, Churn=3, Safe=3, Penalty=−0) |
| Prompt constants duplicated between config + prompts modules | `jscpd:faec70...` | Single source of truth module + re-export | S | Low | **10** (F=2, C=1, Churn=3, Safe=5, Penalty=−1) |
| Overlay AudioWave repeated logic blocks | `ts-ast:sim:ee4b:5` | Extract helper(s) for derived waveform state / rendering decisions | M | Medium | **10** (F=3, C=3, Churn=2, Safe=3, Penalty=−1) |

### Patch-ready refactor notes (top candidates)

#### 1) `app/src/lib/queries.ts` — consolidate `useUpdate*Hotkey` hooks

**What’s duplicated**

- Multiple hooks that likely:
  - create a mutation
  - call a “set hotkey” command with a different hotkey field
  - invalidate similar query keys / refresh settings

**Proposed abstraction**

- A small helper that takes the variation points:
  - which hotkey field (toggle/hold/paste_last/retry/quick_ask_hold/quick_ask_toggle)
  - the command/wrapper used

**Safety**

- Very safe because these are UI-side mutation helpers and already tested behavior can be preserved with table-driven tests.

#### 2) Settings UI duplication — extract reusable render components

**What’s duplicated**

- Large repeated UI blocks inside:
  - `ProvidersSettings.tsx`
  - `QuickReplaceSettings.tsx`
  - `RewriteSettingsSection.tsx`
  - `PresetEditorModal.tsx`

**Proposed abstraction**

- Extract small, obvious components first (lowest risk):
  - “dropdown with a hint + tooltip + default option handling”
  - “reset icon button with same styling/tooltip”
- Then consider a parameterized “settings section” component.

**Risk**

- Medium: UI refactors can accidentally change layout/behavior. Keep diffs small.

#### 3) Rust provider request building duplication

**What’s duplicated**

- Similar request payload construction patterns inside specific provider modules (e.g. Gemini).

**Proposed abstraction**

- Extract a `build_*_request_common_parts(...)` helper.
- Keep provider-specific auth/endpoint differences in each module.

## “Do not DRY” list (things to leave duplicated)

- **Generated files** (`*.generated.*`): duplication is expected.
- **Tests that intentionally mirror similar scenarios**: e.g. schema tests may look repetitive but encode different contract expectations.
- **Small UI wrappers with clear readability**: sometimes repeating 3 lines is better than creating a generic component that hides intent.

## Notes

- There is already an existing manual audit in `docs/Refactors/REPEATED_CODE_AUDIT.md` with high-value semantic duplicates that won’t always show up as exact clones (e.g. “priming payload” construction across Rust modules).
- `tmp/` contains historical snapshots that can create noisy search hits; keep it ignored for DRY scanning.

