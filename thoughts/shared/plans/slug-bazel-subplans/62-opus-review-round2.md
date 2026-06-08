# Plan 62: Opus Review Findings — Round 2

> Created: 2026-06-08
> Model: anthropic/opus4-8 (delegated via 3 parallel subagents)
> Scope: Verify round 1 HIGH fixes; audit RegistryChain + DICE replay correctness

## Overall Verdict: **Significant improvement from round 1; two new HIGH issues found and fixed**

All round 1 HIGH fixes verified correct. Two new issues identified and fixed in this round.

## Round 1 Fix Verification

| Round 1 Issue | Fix Status | Notes |
|---|---|---|
| Staging dir delete-before-rename | PASS | `finalize_staging_dir` does atomic `rename()` only, no `remove_dir_all` |
| Swallowed errors in tar/zip extraction | PASS | All fallible ops use `.map_err(...)?`, not `.ok()` |
| Multi-version identity broken | PASS (with caveat) | Correct structure but had non-determinism (fixed below) |
| Fixpoint loop comment + determinism | PASS | Comment explicitly says NOT Bazel 9's loop; sorted by (name, version) |

## New Issues Found (Round 2)

### HIGH

1. **Non-deterministic multi-version dependency resolution** (resolution.rs:1259)
   - The `break` on first `HashMap` match when resolving multi-version deps
     produces non-deterministic results across runs.
   - Fix: Collect all matching candidates, sort by version ascending, pick
     the lowest compatible version (Bazel 9 parity).
   - Status: FIXED — deterministic candidate collection + `sort_by` + `first()`

2. **Cache path in RegistryFileInputsValue.digest** (cells.rs:2037, 2133)
   - `path.to_string_lossy().as_bytes()` was included in the digest, making
     it machine/configuration-dependent. Same registry content on different
     cache directories produces different digests.
   - Fix: Removed absolute cache path from digest. URL + expected_hash
     already uniquely identify the content. Project-relative-ness flag
     distinguishes in-project vs out-of-project tracking.
   - Status: FIXED — replaced `path.to_string_lossy()` with content-based key

### MEDIUM

3. **`new_local_repository` swallows symlink errors** (repository_executor.rs:2240)
   - Uses `.ok()` on symlink creation for directory entries in the
     local_repository code path. Differs from the strict error propagation
     in archive extraction paths.
   - Status: Acceptable — best-effort symlink for source tree entries,
     outside the security-critical archive extraction path.

4. **MAX_FIXPOINT_ITERATIONS was a magic number** (resolution.rs:1460)
   - Was `let max_iterations = 100` — extracted to named constant.
   - Status: FIXED — `const MAX_FIXPOINT_ITERATIONS: usize = 100`

5. **ModuleCache::new() inside DICE compute is ambient** (cells.rs:2105)
   - `ModuleCache::new()` reads `XDG_CACHE_HOME`/`$HOME` at compute time,
     but the cache base dir is not part of `RegistryFileInputsKey`.
   - Mitigated by the content-based digest fix (issue 2 above) — the digest
     no longer includes the path, so DICE replay gives the same value
     regardless of where the cache lives.
   - Status: MITIGATED — digest is now content-based. Full fix would add
     cache_base_dir to the DICE key, but that's a larger refactor.

### LOW

6. **RegistryChain falls through on ALL errors** (registry.rs)
   - All errors from non-primary registries trigger fallback, including 5xx
     transient errors. Bazel behaves the same way, so this is consistent.
   - Could be improved by distinguishing 404 from 5xx for diagnostics.
   - Status: Acceptable, matches Bazel semantics.

## Things Done Well (Round 2)

1. `BzlmodCommandPolicyKey` is thorough — all 14 fields participate in
   Hash/Eq and the explicit SHA-256 digest function is consistent with the
   derived traits.
2. `validity()` correctly returns `false` for `LockfileMode::Refresh`,
   untracked inputs, and error values.
3. `RegistryChain` is clean — first-wins, last-error fallback,
   primary-only for `bazel_registry.json`.
4. Path traversal containment is comprehensive — `contain_path` + `path_is_within`
   with lexical normalization, belt-and-suspenders in zip extraction,
   test coverage for all escape vectors.

## Action Items

- [x] Fix non-deterministic multi-version dep resolution (sort candidates)
- [x] Fix cache path in digest (content-based key)
- [x] Extract MAX_FIXPOINT_ITERATIONS to named constant
- [ ] Full fix for ModuleCache::new() ambient state (add to DICE key)
- [ ] Distinguish 404 from 5xx in RegistryChain fallback (nice-to-have)
