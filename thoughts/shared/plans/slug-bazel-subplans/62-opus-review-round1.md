# Plan 62: Opus Review Findings — Round 1

> Created: 2026-06-08
> Model: anthropic/opus4-8
> Scope: Audit of Phases 7-10 implementation + overall bzlmod DICE integration

## Overall Verdict: **Legit, not slop, but not fully Bazel-9-parity-correct**

Competent, well-structured Rust code with real DICE integration. The plan
document is remarkably honest about what's missing. "Serious attempt with real
gaps" — closer to production-quality than prototype, but with semantic
correctness holes that matter.

## Findings by Area

### CRITICAL

None found in reviewed code.

### HIGH

1. **Staging dir delete-before-rename gap** (Phase 8)
   - `finalize_staging_dir()` does `remove_dir_all(canonical_dir)` then
     `rename(staging_dir, canonical_dir)`. Between the two, the canonical path
     doesn't exist — concurrent readers see a missing directory.
   - On Linux, `rename()` atomically replaces an existing directory, so
     `remove_dir_all` is unnecessary and creates the gap.
   - Fix: Just `rename(staging, canonical)` without the `remove_dir_all`.
   - Status: NEEDS FIX

2. **Swallowed errors in tar extraction** (Phase 7/15 overlap)
   - Line ~1892: `std::io::copy` result discarded via `.ok()` — truncated
     file writes silently accepted. Data integrity risk.
   - Line ~1917: Symlink creation result discarded via `let _ =` — failed
     symlinks mean silently incomplete repos.
   - Status: NEEDS FIX (Phase 15 covers this but it's HIGH severity now)

3. **Multiple-version canonical module identity broken** (Phase 3)
   - Two selected versions of the same module overwrite graph entries in
     `build_resolved_graph` because keys are collapsed to plain names.
   - Not yet implemented; Phase 3 is still pending.
   - Status: PENDING (Phase 3)

4. **Lockfile writer has no production caller** (Phase 14)
   - The lockfile is effectively read-only. `write_for_purpose` is gated to
     test-only callers.
   - Status: PENDING (Phase 14)

### MEDIUM

5. **Fixpoint loop comment is misleading** (Phase 10)
   - Comment says "matches Bazel 9's Discovery + Selection loop" but it
     doesn't. Bazel's Discovery loop is for **nodep edges**, not post-selection
     rediscovery. Slug's loop is functionally useful but structurally different.
   - Fix: Update comment to accurately describe what the loop does.
   - Status: NEEDS FIX

6. **Fixpoint loop determinism** (Phase 10)
   - `find_modules_needing_rediscovery` iterates over `HashMap<String, Version>`
     which has non-deterministic iteration order. This shouldn't affect MVS
     correctness (always picks max version) but could cause subtle differences
     in resolution order across runs.
   - Fix: Sort the `needs_rediscovery` vector before processing.
   - Status: NEEDS FIX

7. **Extension unique-name disambiguation missing** (Phase 4)
   - Colliding extension IDs produce wrong repo prefixes.
   - Status: PENDING (Phase 4)

### LOW

8. **Headers parameter still just warns** (Phase 9)
   - `headers` on download/download_and_extract produces a warning but doesn't
     reach the HTTP request. Acceptable for now — headers don't cause silent
     auth failures.
   - Status: Acceptable, documented gap.

9. **Non-Unix symlink handling** (Phase 7)
   - Symlinks in tar archives silently skipped on non-Unix platforms.
   - Status: Acceptable, matches Bazel behavior.

## Things Done Well

1. DICE decomposition is thoughtful — separate keys for policy, graph identity,
   marker-based reuse show real understanding of incremental computation.
2. Staging dir pattern is the right idea (Bazel's `_tmp` → rename model).
3. Error type hierarchy is clean — separate typed errors with `slug_error`
   tagging, no stringly-typed errors in critical paths.
4. Lockfile integration is deep — registry file hashes, recorded inputs,
   yanked versions, lockfile mode all wired into DICE keys.
5. Plan document is brutally honest about what's wrong and missing.

## Action Items

- [ ] Fix staging dir: remove `remove_dir_all` before `rename`
- [ ] Fix swallowed errors: propagate `std::io::copy` and symlink creation errors
- [ ] Fix fixpoint loop comment and add determinism sort
- [ ] Continue with Phase 3 (multiple-version identity)
- [ ] Continue with Phase 14 (lockfile writer)
