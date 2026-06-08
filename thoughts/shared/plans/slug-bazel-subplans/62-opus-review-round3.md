# Plan 62: Opus Review Findings — Round 3

> Created: 2026-06-08
> Model: anthropic/opus4-8 (3 parallel subagents)
> Scope: Full audit of lockfile.rs, extension_execution_dice.rs, parser.rs, globals.rs, types.rs

## Summary

Round 3 audited code not previously covered and found 1 CRITICAL and several HIGH issues.
The CRITICAL and key HIGH issues have been fixed.

## Issues Found and Fixed

### CRITICAL (Fixed)

1. **`include()` path traversal via `..` in label components** (parser.rs:691-714)
   - `include_label_to_path` did not validate against `..` segments in package
     or name components. A label like `//foo/../../etc:passwd.MODULE.bazel` could
     escape the module root.
   - Fix: Reject `..` and `.` segments in both package and name components, plus
     canonicalize-and-verify the resolved path stays within module_root.
   - Status: FIXED

### HIGH (Fixed)

2. **`DefaultHasher` used for DICE identity digests** (extension_execution_dice.rs:337, 359)
   - `std::collections::hash_map::DefaultHasher` has no stability guarantee across
     Rust versions. Used for `selected_extension_cache_repo_specs_digest` and
     `module_extension_replay_inputs_identity_digest` which flow into DICE key identity.
   - Also only 64-bit output width — birthday collision risk at scale.
   - Fix: Replaced with SHA-256 (`sha2::Sha256`) producing full 256-bit hex digest.
   - Status: FIXED

### HIGH (Deferred — Phase 14 scope)

3. **`write_for_purpose` ignores `_purpose` parameter** (lockfile.rs:1112)
   - The lockfile write gating is never enforced. Any code path can write.
   - Status: DEFERRED to Phase 14 (lockfile writer + mode enforcement)

4. **`LockfileMode::Error` and `LockfileMode::Refresh` never enforced** (lockfile.rs)
   - These modes have no behavioral effect on lockfile reads/writes.
   - Status: DEFERRED to Phase 14

5. **`LockfileMode::Refresh` not enforced in lockfile reads** (lockfile.rs:1638)
   - `read_lockfile_at_path` only checks for `LockfileMode::Off`.
   - Note: Refresh IS enforced at the DICE level via `validity() == false`
     (verified in round 2), so the graph IS re-resolved. The lockfile read
     path just doesn't skip the stale lockfile cache. This is a partial gap.
   - Status: DEFERRED to Phase 14

### HIGH (Deferred — feature parity, not correctness bugs)

6. **`archive_override`/`git_override` missing `patch_cmds` field** (types.rs, globals.rs)
   - Bazel 9 supports `patch_cmds` on both override types. Slug doesn't.
   - Valid Bazel MODULE.bazel files will fail with "unexpected keyword argument".
   - Status: DEFERRED (Bazel 9 feature parity, not a correctness bug)

7. **`compute_extension_input_hash` doesn't hash `imported_repos`** (extensions.rs:334)
   - Only hashes `extension_id` and `tags_by_module`. Missing `imported_repos`.
   - If `imported_repos` changes, DICE could return stale cached extension results.
   - Status: DEFERRED (requires careful analysis of whether imported_repos
     affects extension output or only visibility)

### MEDIUM (Noted)

8. **`RepoSpec.compute_hash` uses `Debug` formatting** (repo_spec.rs:103)
   - `Debug` trait has no stability guarantee. A `Debug` format change would
     cause the same RepoSpec to produce a different spec_hash.
   - Status: Noted, should use stable serialization instead.

9. **`stable_json_digest` fallback on serialization error collapses distinct values**
   - Any two values that fail JSON serialization produce the same digest
     `"<json-error>"`, causing a DICE identity collision.
   - Status: Noted, should propagate the serialization error.

10. **`override_repo`/`inject_repo` positional args skip validation** (globals.rs)
    - Non-root modules can store unvalidated repo names.
    - Status: Noted, should validate consistently.

## Remaining Work

The remaining CRITICAL/HIGH issues from round 3 are all in the "feature parity"
or "mode enforcement" categories (Phase 14 scope), not correctness bugs in the
already-implemented phases. The two correctness bugs (path traversal, DefaultHasher)
have been fixed.

For the review loop: the next round should focus on verifying the path traversal
and DefaultHasher fixes are correct, and checking if any CRITICAL/HIGH issues
remain that are NOT deferred.
