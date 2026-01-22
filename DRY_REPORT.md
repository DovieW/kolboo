# DRY_REPORT.md — DRY findings (Kolboo)

This is a practical “what should we extract?” report.

Key note: **exact copy/paste in production code is fairly low** in this repo. The main DRY opportunities are “same structure, different provider/settings keys”.

## Summary (where duplication clusters)

There are currently **no high-impact DRY hotspots** worth tackling without changing architecture.

Recent refactors extracted the common “safe” pieces (HTTP client builders, base URL trimming/joining, and tolerant numeric settings reads) into shared helpers, so new duplication should be rare.

## Low-risk DRY TODOs (small, safe extractions)

- (Later) Consider a tiny crate-level URL join helper for non-STT providers too (LLM/embeddings), if we see more repeats.

## “Do not refactor” list (duplication that’s OK)

- The long list of explicit Tauri `invoke(...)` wrappers in `app/src/lib/tauri/commands.ts` is repetitive, but it also serves as a clear UI↔backend contract. DRY-ing it too much can hide argument shapes and make refactors harder.
- Provider implementations often *look* similar but have important differences (timeouts, auth headers, endpoints, retries, error mapping). Prefer extracting only the truly shared pieces (client creation, base URL joining) rather than forcing full inheritance.
- Test duplication is sometimes intentional and can keep scenarios readable.
