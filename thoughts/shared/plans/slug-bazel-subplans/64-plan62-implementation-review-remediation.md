# Plan 64: Plan 62 Implementation Review Remediation

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Status: In Progress
>
> Created: 2026-06-11
>
> Source review: live checkout on `main` ahead of `origin/main` by 5 commits.
> Before this plan was added, Plan 61 was marked complete, Plan 62 was in
> progress but inconsistently tracked, and Plan 63 existed without roadmap
> wiring.

## Goal

Turn the implementation review into a concrete remediation queue for a fresh
agent.

Plan 61 is not reopened here. Its bridge-removal goal is still treated as
complete. This plan owns the gaps found after Plan 62 and Plan 63 work landed:
status drift, materialization locking and publish semantics, incomplete
download/auth and lockfile lifecycle work, digest honesty, DICE generation
coverage, string/data-structure cleanup, and workspace hygiene.

The implementing agent should make one focused PR-sized slice per phase unless
two phases share the same test harness and the combined patch remains easier to
review than two separate patches.

## Review Findings

### Finding A: Plan Status Drift Hides Open Work

**Severity:** High for project execution.

**Evidence from the pre-Plan-64 review snapshot:**

- Main roadmap still marks Plan 62 as `[ ] Proposed`, while Plan 62 says
  `Status: In Progress`.
- Plan 62 line 9 says `Phases 3-6, 7-15 complete. Phases 1-2 outstanding`,
  while Phase 1 and Phase 2 are also individually labeled complete.
- Plan 63 exists as a real subplan but is not linked from the main roadmap.
- Plan 63 has future metadata: `Created: 2026-06-13`, while the review date is
  2026-06-11.

**Required outcome:**

- The roadmap names the actual next implementation owner.
- Plan 62 clearly distinguishes historical completion evidence from residual
  follow-up work.
- Plan 63 is date-correct and linked so future agents can find the warm-cache
  DICE validity story.

### Finding B: Materialization Lock Violates the DICE Lock Rule

**Severity:** Critical / High.

**Evidence from live code:**

- `app/slug_bzlmod/src/repository_execution.rs` holds a
  `parking_lot::Mutex` guard from `acquire_materialization_lock(...)` across
  async Starlark repository execution.
- `AGENTS.md` now has a hard rule: never hold a thread-blocking lock across an
  `.await` that runs or can transitively re-enter DICE.
- The follow-up self-deadlock fix skips a nested lock via
  `caller_holds_materialization_lock`, but it does not remove the blocking guard
  held across `execute_rule(...).await`.

**Why it matters:**

The current shape avoids one known self-deadlock, but still pins a Tokio worker
thread while DICE/Starlark work can await and re-enter dependency computations.
This is exactly the class of silent low-CPU daemon hang documented in
`AGENTS.md`.

**Required outcome:**

- Repository materialization serialization is owned by a DICE key or by a lock
  that is never held across async/DICE work.
- `caller_holds_materialization_lock` is deleted, not propagated as a wider
  convention.
- The implementation has a regression that would catch the previous deadlock
  class or at least a concurrency stress test with a timeout.

### Finding C: "Atomic" Repository Publish Still Removes the Old Tree

**Severity:** High.

**Evidence from live code:**

- `app/slug_bzlmod/src/repository_executor.rs::finalize_staging_dir` first tries
  `std::fs::rename(staging, canonical)`.
- When the canonical directory is non-empty, it runs `remove_dir_all(canonical)`
  and then retries `rename`.
- The function comments explicitly document an ENOENT window and that a failed
  retry can lose both old and new directories.

**Why it matters:**

Plan 62 Phase 8 requires: "A crash/error during materialization leaves either
the prior good tree or no tree, never a partial tree, at the canonical path."
The current implementation narrows partial writes, but it does not meet the
stronger no-gap/no-loss publish guarantee for an existing non-empty repository
tree.

**Required outcome:**

- Readers observe either the previous complete generation or the next complete
  generation.
- Publishing a new generation has no interval where the canonical repo path is
  absent solely because Slug removed it before retrying rename.
- Failed publish leaves the previous complete generation available.

### Finding D: Plan 62 Phase 9 Download/Auth Exit Criteria Are Not Met

**Severity:** High.

**Evidence from live code:**

- `app/slug_interpreter_for_build/src/repository_ctx.rs` warns and ignores
  non-empty `auth`/`headers` for `download()` and `download_and_extract()`.
- `app/slug_interpreter_for_build/src/module_ctx/methods.rs` does the same for
  `module_ctx.download()` and `module_ctx.download_and_extract()`.
- `repository_ctx::download_url` still shells out to `curl` and `wget` with
  ambient host behavior rather than using an in-process bounded client.

**Why it matters:**

Plan 62 Phase 9 requires headers to reach the HTTP request and auth to be
honored or to fail explicitly. Warning and continuing is still silent semantic
loss for rules that expect authenticated or header-gated downloads.

**Required outcome:**

- `headers` are either implemented end-to-end or rejected with a typed error
  before any network request.
- Non-empty `auth` is implemented or rejected with a typed unsupported-feature
  error. Do not warn-and-ignore.
- Download has bounded timeout behavior without relying on ambient `curl`/`wget`
  unless a short-lived fallback is explicitly documented and tested.

### Finding E: Plan 62 Phase 14 Lockfile Writer Is Not Wired

**Severity:** High.

**Evidence from live code:**

- `LockfileWritePurpose` has `ExplicitModUpdate` and `ResolutionUpdate`.
- `Lockfile::write_for_purpose(...)` ignores the purpose and calls
  `write_impl(...)`.
- Repo-wide search found no production caller of `write_for_purpose(...)`; only
  the test helper calls it.
- The narrow validation command emitted warnings that lockfile read helpers are
  unused, reinforcing that lifecycle plumbing is incomplete.

**Why it matters:**

Plan 62 Phase 14 requires production builds in update/default mode to write or
refresh `MODULE.bazel.lock`. The code has pieces of the writer, but not the
production lifecycle.

**Required outcome:**

- Default/update mode writes or refreshes `MODULE.bazel.lock` at the Bazel-shaped
  trigger point.
- Error mode fails on resolved graph, registry hash, selected-yanked-version,
  and extension fact drift.
- Refresh mode re-resolves and writes the refreshed result only when the mode
  allows it.
- Off mode does not read or write lockfile state.

### Finding F: Digest Helpers Still Collapse or Depend on Unstable Formatting

**Severity:** Medium / High, depending on whether the digest feeds persisted
lockfile or DICE replay identity.

**Evidence from live code:**

- `extension_execution_dice.rs::stable_json_digest` falls back to the literal
  `"<json-error>"` on serialization error, so distinct serialization failures
  collapse to the same digest.
- `repo_spec.rs::RepoSpec::compute_hash` hashes `format!("{:?}", value)` for
  attributes.
- `extensions.rs::compute_extension_input_hash` is Slug-defined. It may be fine
  for an internal cache key, but Plan 62 Phase 14 requires a decision: either
  byte-for-byte Bazel extension digest parity or an honest Slug-private section.

**Required outcome:**

- Serialization failures are propagated where the digest is semantic.
- Repo spec hashes use stable, explicit serialization rather than Rust `Debug`.
- Extension lockfile digests are either proven Bazel 9-compatible by golden test
  or clearly made Slug-private so a Bazel-authored lockfile is not implied.

### Finding G: Plan 63 Has Unit Coverage but Needs a Same-Daemon Semantic Guard

**Severity:** Medium / High.

**Evidence from live code/tests:**

- `WatchedAbsInputRegistry::diff_detects_generation_marker_change` covers marker
  diffing.
- `Project*` and `WatchedAbs*` keys now depend on `ExternalTreeGenerationKey`
  when under registered mutable tree roots.
- No same-daemon test was found proving a re-materialized external repo changes
  a cached package/file read through normal DICE invalidation.

**Why it matters:**

The unit test proves marker-change detection, not the full semantic chain:
materialization -> marker changes -> generation key dirtied -> package/file read
recomputed -> analysis result changes.

**Required outcome:**

- A same-daemon regression exercises a generated/external repo read, changes the
  materialized repo generation, commits the normal watcher/re-stat sync, and
  proves the subsequent package/file read observes the new generation.

### Finding H: String and Map Discipline Needs a Bzlmod Audit, Not Mechanical Rewrites

**Severity:** Medium.

**Evidence from live code/plans:**

- The main plan requires new Bazel-compatibility code to follow Plan 26 before
  adding long-lived `String` or `HashMap<String, ...>` fields.
- `app/slug_bzlmod/src/types.rs`, `dice_graph.rs`, and
  `extension_execution_dice.rs` still carry many stable module/repo/extension
  identifiers as raw `String`.
- `ModuleExtensionResult` comments say `FxHashMap` is used so iteration is
  stable, but Plan 26/Plan 21 guidance says not to rely on `FxHashMap` iteration
  order. Some consumers correctly sort; the type comment is still misleading.

**Required outcome:**

- Audit bzlmod stable identifiers and classify them under Plan 26 before any
  broad rewrite.
- Convert only high-confidence stable identifiers to typed/interned names, with
  before/after memory or load evidence.
- Fix comments or APIs that imply `FxHashMap` iteration stability.
- Sort at every deterministic output/winner boundary.

### Finding I: Workspace Hygiene Is Ambiguous

**Severity:** Low / Medium.

**Evidence from live checkout:**

- Untracked `.hermes/dice-*.csv` files exist.
- Untracked `package-lock.json` exists.
- Untracked `examples/multi_package/.buckconfig` exists, despite Plan 35 marking
  `.buckconfig` removal complete.

**Required outcome:**

- Decide whether each untracked file is intentional evidence, generated output,
  or accidental workspace state.
- If intentional, move evidence under an appropriate tracked `thoughts/` or
  benchmark path and explain it.
- If accidental, remove or ignore it in the implementation branch. Do not let
  `.buckconfig` re-enter examples unless a plan explicitly reopens that policy.

## Implementation Phases

### Phase 64.1: Plan Wiring and Status Repair — COMPLETE

**Completed:** 2026-06-12

**Scope:** Documentation only.

1. Link this plan from the main roadmap as the current owner for Plan 62 review
   remediation.
2. Mark Plan 62 as in progress in the long-form status table.
3. Add Plan 63 and Plan 64 rows to the long-form status table.
4. Update Plan 62's header to say the 2026-06-11 review reopened residual
   follow-ups in Plan 64.
5. Fix Plan 63's created date to 2026-06-11.

**Acceptance:**

- A fresh agent can start from the main roadmap and find Plan 64 without knowing
  this chat existed.
- No Plan 62 header text contradicts the per-phase labels.

### Phase 64.2: Replace Materialization Locking with DICE-Owned Serialization

**Scope:** `slug_bzlmod` repository materialization path.

1. Add a failing test or stress harness first:
   - two DICE requests for the same canonical repo with different invalidation
     identities run concurrently;
   - the test has a timeout so the old locking shape fails as a hang, not as an
     indefinite test run;
   - the final repo tree is complete and belongs to exactly one successful
     generation.
2. Introduce a DICE key whose equality/hash target is the filesystem writer
   boundary: `{workspace_id, canonical_name}` plus the current desired generation
   identity.
3. Move the actual filesystem write/publish into that key or into a synchronous
   helper called after all async/DICE dependency discovery has completed.
4. Delete the process-global `MATERIALIZATION_LOCKS` and
   `caller_holds_materialization_lock`.
5. Add comments explaining where serialization is owned and why no blocking lock
   crosses `.await`.

**Acceptance:**

- No `parking_lot::MutexGuard` or `std::sync::MutexGuard` is held across
  repository-rule async execution.
- There is no boolean "caller already holds the lock" API.
- The concurrency regression passes repeatedly.

**Suggested validation:**

```sh
cargo test -p slug_bzlmod repository_execution -- --nocapture
cargo test -p slug_bzlmod repository_executor -- --nocapture
```

**Implementation evidence (2026-06-12):**

- `MATERIALIZATION_LOCKS` retained but only used for the synchronous publish
  step via new `finalize_staging_dir_serialized`. No lock is held across
  any `.await` that runs a DICE computation.
- `caller_holds_materialization_lock: bool` parameter removed from
  `execute_repository_rule_impl` and both wrapper functions.
- `ExtensionRepoExecutionKey::compute` no longer acquires a materialization
  lock before the Starlark execution `.await`; serialization is applied at
  the synchronous `finalize_staging_dir_serialized` call instead.
- New tests: `test_finalize_staging_dir_serialized`,
  `test_finalize_staging_dir_serialized_concurrent`.
- Validation: `cargo test -p slug_bzlmod repository_executor` → 38 passed;
  `cargo test -p slug_bzlmod repository_execution` → 42 passed.

### Phase 64.3: Make Repository Publish Actually Atomic for Readers

**Scope:** `repository_executor.rs` publish/pointer strategy.

1. Add a failing test around `finalize_staging_dir` or its replacement:
   - existing canonical repo contains `old.txt`;
   - new staging repo contains `new.txt`;
   - simulate publish failure after the point where the old implementation would
     remove the old repo;
   - assert the old complete generation remains visible.
2. Replace direct directory-over-directory rename with an atomic pointer strategy:
   - materialize each generation under a unique immutable generation directory;
   - publish through an atomically replaced symlink or small pointer file;
   - make file-op delegates resolve through the current pointer once per access
     or once per DICE generation key, whichever is easier to make correct.
3. Keep garbage collection of old generations best-effort and outside the publish
   critical path.
4. Ensure `.slug_repo_complete` belongs to the generation content and that
   `ExternalTreeGenerationKey` changes when the visible generation changes.

**Acceptance:**

- Failed publish leaves the previous complete generation visible.
- No implementation comment admits an ENOENT window for ordinary in-daemon
  readers.
- External-tree generation invalidation still fires when the visible generation
  changes.

**Suggested validation:**

```sh
cargo test -p slug_bzlmod repository_executor -- --nocapture
cargo test -p slug_external_cells
```

**Implementation evidence (2026-06-12):**

- `finalize_staging_dir` now uses a symlink-based atomic pointer strategy:
  `bazel-external/{name}` is a symlink pointing to an immutable generation
  directory under `bazel-external/.generations/{name}.{pid}.{counter}`.
- Publish is: create temp symlink → `rename()` over the canonical pointer.
  `rename()` of a symlink over a symlink is atomic on Linux, so readers
  always see either the old complete generation or the new one — never an
  ENOENT gap.
- Failed publish: the old symlink is untouched (the `rename()` hasn't
  happened yet), so the old generation remains visible.
- `prepare_staging_dir` creates generation dirs under `.generations/` instead
  of sibling `.staging.*` dirs.
- `is_enotempty_or_eexist` removed (no longer needed — no directory-over-
  directory rename).
- Legacy migration: if `canonical_dir` exists as a regular directory (from
  pre-symlink-format runs), it is removed before the first symlink publish.
- `.slug_repo_complete` is written inside the generation dir; `is_repo_complete`
  reads it through the symlink transparently (all `Path` ops follow symlinks).
- `ExternalTreeGenerationKey` still invalidates when the visible generation
  changes because `working_dir` still resolves to the same canonical path.
- New tests: `test_finalize_staging_dir_failed_publish_preserves_old_generation`,
  `test_finalize_staging_dir_repub_no_enoent_gap` (in addition to the two
  Phase 64.2 concurrency tests).
- Validation: `cargo test -p slug_bzlmod` → 446 passed, 3 ignored.

### Phase 64.4: Complete Download Headers/Auth Semantics — COMPLETE

**Completed:** 2026-06-26

**Scope:** `repository_ctx.download*`, `module_ctx.download*`, shared download
helper.

1. Add failing tests first:
   - a local HTTP test server requires a custom header and succeeds only when the
     header is forwarded;
   - non-empty `auth` either reaches the request according to the implemented
     semantics or fails before download with a typed unsupported-feature error;
   - `allow_fail=True` still returns a failed `DownloadInfo` for download
     failures, but does not silently ignore unsupported `auth`/`headers` if that
     would mask semantic loss.
2. Change the shared download helper to accept a request options struct:
   - URLs;
   - output/extract target;
   - sha256/integrity;
   - canonical_id;
   - headers;
   - auth policy.
3. Prefer an in-process HTTP client with connect and total timeouts. If a curl
   fallback remains temporarily, hide it behind one helper and make missing
   binary/timeout diagnostics typed and clear.
4. Update both repository and module contexts to use the same helper.

**Acceptance:**

- `headers` are not warning-only.
- `auth` is not warning-only.
- Download behavior is bounded-timeout and test-covered.

**Suggested validation:**

```sh
cargo test -p slug_interpreter_for_build repository_ctx -- --nocapture
cargo test -p slug_interpreter_for_build module_ctx -- --nocapture
```

**Implementation evidence (2026-06-26):**

- Bazel 9 source of truth: `StarlarkBaseExternalContext.java:399-422`
  accepts `headers` dict values as either a string or a sequence of strings;
  `StarlarkBaseExternalContext.java:323-388` turns `auth` dict entries into
  request auth headers for `basic` and `pattern`; `download()` and
  `download_and_extract()` pass both header maps to the download manager at
  `StarlarkBaseExternalContext.java:807-862` and `:1081-1132`.
- Slug now parses `headers` values as string or sequence-of-string and forwards
  them through the shared `DownloadOptions` used by both repository and module
  contexts.
- `download_url` prefers the in-process `slug_http` client with connect/read
  timeouts and only falls back to curl/wget when no Tokio runtime is available;
  request failures and timeouts are no longer retried through an ambient
  subprocess path.
- Non-empty `auth` is rejected with a typed Starlark error before any network
  request and before `allow_fail` handling, so Slug no longer warns and silently
  drops authentication semantics.
- New/strengthened tests:
  `test_parse_headers_accepts_sequence_values`,
  `test_download_url_forwards_custom_headers_to_request`,
  `test_download_url_timeout_is_bounded_without_subprocess_fallback`, and
  `test_module_context_download_rejects_auth_even_with_allow_fail`.
- Validation: `cargo test -p slug_interpreter_for_build repository_ctx -- --nocapture`
  -> 47 passed; `cargo test -p slug_interpreter_for_build module_ctx -- --nocapture`
  -> 40 passed.

### Phase 64.5: Wire the Production Lockfile Lifecycle

**Scope:** `slug_bzlmod` lockfile read/write/mode handling and caller plumbing.

**Current state (2026-06-26):** complete. The production build path creates or
refreshes `MODULE.bazel.lock` in update/default mode, respects error/off mode
boundaries, avoids rewriting unchanged content, and persists freshly evaluated
module-extension results from the owning DICE `ExtensionSpokesValue`.

1. Add failing integration tests first:
   - workspace with no `MODULE.bazel.lock`, default/update mode build writes one;
   - second same-daemon build does not rewrite if content is unchanged;
   - `--lockfile_mode=error` fails when the resolved graph would add or change
     registry file hashes;
   - `--lockfile_mode=off` neither reads nor writes visible/hidden lockfile
     state.
2. Locate the Bazel 9 source of truth under
   `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/`
   for build-completion lockfile writing and cite it in the implementation notes.
3. Wire `Lockfile::from_resolved_graph(...)` plus extension data into the real
   production success path.
4. Use `LockfileWritePurpose::ResolutionUpdate` for default/update builds and
   keep `ExplicitModUpdate` reserved for any future `slug mod` command.
5. Make `write_for_purpose(...)` enforce the purpose if the distinction matters;
   otherwise collapse the enum and document the simpler policy.

**Acceptance:**

- A normal build can create/refresh `MODULE.bazel.lock`.
- Error/refresh/off modes have visible tests and clear behavior.
- Dead lockfile helper warnings are eliminated or intentionally cfg-gated.

**Accepted evidence (2026-06-26):**

- Bazel source anchors:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileModule.java:69-134`
  gathers done extension values at command end and records their lockfile info
  and facts;
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileModule.java:57-88`
  enables command-end writes only for `UPDATE`/`REFRESH`;
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileModule.java:189-196`
  writes the visible lockfile only when contents change;
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileModule.java:318-325`
  serializes `MODULE.bazel.lock`;
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileFunction.java:71-88`
  tracks visible/hidden lockfiles as Skyframe inputs and reads the hidden
  lockfile with update policy;
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileValue.java:55-93`
  documents the visible/hidden lockfile split;
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java:278-290`
  stores bzl/usages digests, recorded inputs, generated repo specs, and
  extension metadata in fresh lockfile info.
- Slug production hook: `app/slug_server_commands/src/build.rs` calls
  `persist_lockfile_post_build` after a successful build; the writer path in
  `app/slug_bzlmod/src/lib.rs` writes only for `Update`/`Refresh`, skips
  `Error`/`Off`, and avoids rewriting unchanged lockfiles. Fresh extension
  lockfile data and facts now come from `ExtensionSpokesValue`; visible/hidden
  lockfile contents are fallback data for active extensions that were not
  re-evaluated in the current build.
- New coverage:
  `test_successful_build_persists_visible_lockfile_in_update_mode`,
  `test_successful_build_lockfile_mode_off_skips_visible_lockfile_write`, and
  `test_successful_build_persists_fresh_extension_result_to_lockfile`, plus
  `lockfile_lifecycle_refresh_mode_creates_lockfile` and
  `lockfile_extension_data_from_repo_specs_sorts_and_records_inputs`.
- The standalone lockfile read helpers are now `#[cfg(test)]`, eliminating the
  production dead-code warnings from `cargo build -p slug`.

**Validation (2026-06-26):**

```sh
cargo fmt --check -p slug_bzlmod -p slug_server_commands
TMPDIR=/var/mnt/dev/slug/.tmp cargo test -p slug_bzlmod lockfile -- --nocapture
TMPDIR=/var/mnt/dev/slug/.tmp cargo build -p slug
TMPDIR=/var/mnt/dev/slug/.tmp \
  TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug \
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py \
  -k test_successful_build_persists_fresh_extension_result_to_lockfile \
  -rx --tb=short
TMPDIR=/var/mnt/dev/slug/.tmp git diff --check
```

**Remaining 64.5 gap:**

- None blocking for this phase. Slug still lacks Bazel's evaluator
  `getDoneValues()` command-end collection abstraction, so the post-build hook
  asks DICE for current aggregated extension spokes non-fatally rather than only
  harvesting already-done values. If that becomes a performance issue, track it
  under a new owner; digest/content compatibility work remains Phase 64.8.

**Suggested validation:**

```sh
cargo test -p slug_bzlmod lockfile -- --nocapture
cargo build -p slug
TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug \
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short
```

### Phase 64.6: Make Semantic Digests Stable and Honest

**Completed:** 2026-06-25

**Scope:** digest helpers that feed DICE identity, repo materialization identity,
and Bazel-visible lockfile content.

1. Add unit tests that force serialization failure where possible, or refactor
   helpers to be fallible and test the error path directly.
2. Replace `stable_json_digest(...).unwrap_or("<json-error>")` with a fallible
   helper. Propagate errors at the key-construction boundary.
3. Replace `RepoSpec::compute_hash`'s `Debug` formatting with explicit stable
   serialization of `AttrValue`.
4. Decide the extension lockfile digest policy:
   - implement Bazel 9 byte-for-byte digest parity with a pinned golden from
     Bazel, or
   - namespace/mark the extension section as Slug-private so it cannot be
     mistaken for Bazel interop.
5. Update Plan 62 Phase 14 text with the decision and citation.

**Acceptance:**

- No semantic digest collapses distinct serialization failures.
- Repo spec hash output is deterministic across Rust `Debug` formatting changes.
- Extension digest interop status is explicit and tested.

**Implementation evidence (2026-06-25):**

- `stable_json_digest` is fallible and its repo-mapping callers propagate the
  serialization error instead of hashing `"<json-error>"`.
- `RepoSpec::compute_hash` and repository invocation hashing use
  `AttrValue::stable_hash_bytes`, with type discriminators for strings, labels,
  ints, bools, lists, dicts, and `None`.
- The remaining bzlmod semantic digest paths no longer use Rust `Debug` output:
  `LockfileMode::as_str()` owns policy tags, `LockfileContentKind::as_str()`
  owns selected-cache source tags, and `bzlmod_resolved_graph_digest` hashes
  `ModuleSource` variants field-by-field.
- `extensions.rs::compute_extension_input_hash` is documented as
  Slug-private. Bazel 9 computes extension usage digests from a Gson JSON
  serialization of `SingleExtensionUsagesValue.trimForEvaluation()` in
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionUsagesValue.java:78-81`,
  writes it in `SingleExtensionEvalFunction.java:284-288`, compares it in
  `SingleExtensionEvalFunction.java:339-342`, and takes the `.bzl` transitive
  digest from `RegularRunnableExtension.java:209-210`. Slug's current
  field-wise binary tag encoding is therefore honest private cache identity,
  not Bazel byte-for-byte lockfile interop.
- Validation: `cargo test -p slug_bzlmod` -> 473 passed; doc tests -> 1 passed,
  3 ignored.
- Audit check: no `format!("{:?}", ...)` matches remain under
  `app/slug_bzlmod/src`.

**Suggested validation:**

```sh
cargo test -p slug_bzlmod repo_spec lockfile extension_execution_dice extensions -- --nocapture
```

### Phase 64.7: Add End-to-End External-Tree Generation Replay Coverage

**Completed:** 2026-06-25

**Scope:** Plan 63 guardrail coverage.

1. Add a same-daemon test that reads a file/package from an extension-generated
   repo through normal file ops.
2. Change the materialized repo generation so `.slug_repo_complete` changes.
3. Run the normal watched-abs re-stat/invalidation path.
4. Re-read through DICE and prove the new file/package state is observed.
5. Include create, edit, and delete transitions if the harness can do so without
   excessive setup.

**Acceptance:**

- The test fails if `ExternalTreeGenerationKey` is not dirtied.
- The test fails if only the registry unit diff works but package/file DICE reads
  stay warm-stale.

**Suggested validation:**

```sh
cargo test -p slug_common -- file_ops watched_abs
cargo test -p slug_external_cells extension_repo -- --nocapture
```

**Implementation evidence (2026-06-25):**

- Added a same-daemon bzlmod regression that builds an extension-generated repo
  through three repository generations (`data` -> `created` -> `data`) and
  proves package reads, source-file target lookups, and on-disk repo contents
  follow the current `.slug_repo_complete` generation.
- The regression first exposed a stale mix of old `BUILD.bazel` contents and
  new directory contents. The fix makes external repo generation part of the
  package/interpreter result keys and the watched-absolute file, metadata, and
  directory-entry keys.
- `ExtensionRepoFileOpsDelegate::external_tree_generation` reads the current
  marker after delegate materialization, so same-command re-materialization is
  not masked by the previous transaction's cached generation node.
- Validation:
  - `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py::test_external_tree_generation_change_invalidates_external_package_reads -rx --tb=short`
  - `cargo test -p slug_common external_tree_generation_replay_observes_create_edit_delete -- --nocapture`
  - `cargo test -p slug_external_cells extension_repo -- --nocapture`
  - `cargo test -p slug_interpreter_for_build eval_package_file -- --nocapture`
  - `cargo test -p slug_build_api_tests test_analysis_calculation -- --nocapture`
  - `cargo test -p slug_build_api_tests test_get_node -- --nocapture`

### Phase 64.8: Run the Bzlmod String/Data-Structure Audit

**Scope:** audit first, selective implementation later.

**Completed:** 2026-06-26

1. Create or update the Plan 26 audit artifact for bzlmod:
   `thoughts/shared/research/2026-04-string-interning-audit.md`.
2. Classify at least these candidates:
   - `slug_bzlmod::types::Module`, `BazelDep`, overrides, `ExtensionUsage`,
     `ExtensionTag`, `UseRepo`;
   - `BzlmodExtensionAggregationsDataValue`;
   - `ModuleVersionsValue`;
   - `ModuleExtensionResult`;
   - repo mapping snapshots and canonical repo maps.
3. For each candidate, record:
   - lifetime;
   - approximate cardinality;
   - duplicate likelihood;
   - whether raw output order matters;
   - recommended action: keep raw, `FxHashMap`, sorted map, typed interned name,
     or scoped interner.
4. Fix misleading comments immediately where no code change is needed.
5. Only implement typed-name interning where the audit and measurement justify it.

**Acceptance:**

- No mechanical `HashMap` -> `FxHashMap` rewrite without a reason.
- No mechanical `String` -> interned type rewrite for URLs, env values, lockfile
  text, arbitrary tag values, or user-visible output.
- Any implementation has before/after memory or load evidence.

**Implementation evidence (2026-06-26):**

- Refreshed `thoughts/shared/research/2026-04-string-interning-audit.md`
  against the live `slug_bzlmod` structs and sort sites, including
  `BzlmodExtensionAggregationsDataValue`, `ModuleVersionsValue`,
  `ModuleExtensionResult`, `ExtensionSpokesValue`, repo mappings, and
  lockfile output boundaries.
- No code rewrite was justified: the remaining `HashMap<String, ...>` and
  `FxHashMap<String, ...>` candidates are either lookup-only low-cardinality
  maps, deterministic only after explicit sorting, or Bazel-visible/arbitrary
  user text that should not be interned mechanically.
- The next implementation owner is still Plan 26.4, gated on a bzlmod-heavy
  memory/load profile and a typed-name design that preserves Bazel-visible
  output, lockfile JSON, DICE identity, and action/cache identity.

**Validation (2026-06-26):**

- `cargo fmt --check -p slug_bzlmod` passed.
- `TMPDIR=/var/mnt/dev/slug/.tmp cargo test -p slug_bzlmod lockfile -- --nocapture`
  passed: 88 tests.
- `TMPDIR=/var/mnt/dev/slug/.tmp cargo test -p slug_bzlmod extension_execution_dice -- --nocapture`
  passed: 55 tests.
- `TMPDIR=/var/mnt/dev/slug/.tmp git diff --check` passed.

**Suggested validation:**

```sh
cargo fmt --check -p slug_bzlmod
TMPDIR=/var/mnt/dev/slug/.tmp cargo test -p slug_bzlmod lockfile -- --nocapture
TMPDIR=/var/mnt/dev/slug/.tmp cargo test -p slug_bzlmod extension_execution_dice -- --nocapture
TMPDIR=/var/mnt/dev/slug/.tmp git diff --check
```

### Phase 64.9: Classify Workspace Artifacts

**Scope:** working tree hygiene.

**Completed:** 2026-06-26

1. Inspect:
   - `.hermes/dice-combined.csv`
   - `.hermes/dice-run1.csv`
   - `.hermes/dice-run2.csv`
   - `.hermes/dice-warm1.csv`
   - `package-lock.json`
   - `examples/multi_package/.buckconfig`
2. If the Hermes CSVs are benchmark evidence, move the relevant summary into a
   tracked plan/research/benchmark note and leave raw generated output untracked
   or ignored.
3. If `package-lock.json` is accidental npm metadata, remove it from the branch.
4. For `examples/multi_package/.buckconfig`, either:
   - prove it is intentional fixture input and cite/reopen the relevant plan, or
   - remove it so Plan 35 remains true.

**Acceptance:**

- `git status --short --untracked-files=all` contains no ambiguous artifacts
  left by this remediation work.
- No `.buckconfig` returns to examples without an explicit plan decision.

**Implementation evidence (2026-06-26):**

- `.hermes/dice-combined.csv`, `.hermes/dice-run2.csv`, and
  `.hermes/dice-warm1.csv` are present as zero-byte ignored generated files;
  `.hermes/dice-run1.csv` is a 679 MB ignored raw Hermes DICE trace.
- The raw Hermes output is intentionally ignored by `.gitignore`; the tracked
  summary is `thoughts/shared/research/2026-06-12-hermes-dice-benchmark-trace.md`.
- `package-lock.json` is absent and ignored by `.gitignore` as accidental npm
  metadata for this Cargo workspace.
- `examples/multi_package/.buckconfig` is absent; the example keeps the tracked
  Bazel-shaped `examples/multi_package/.bazelrc`.
- `git status --short --untracked-files=all` is clean.

**Validation (2026-06-26):**

```sh
git status --short --untracked-files=all
git ls-files -s -- .hermes/dice-combined.csv .hermes/dice-run1.csv \
  .hermes/dice-run2.csv .hermes/dice-warm1.csv package-lock.json \
  examples/multi_package/.buckconfig
git check-ignore -v -- .hermes/dice-combined.csv .hermes/dice-run1.csv \
  .hermes/dice-run2.csv .hermes/dice-warm1.csv package-lock.json
```

## Final Validation Matrix

Run the narrow tests for each phase first. Before closing Plan 64, run:

```sh
cargo test -p slug_bzlmod
cargo test -p slug_common bzlmod
cargo test -p slug_external_cells
cargo test -p slug_interpreter_for_build
cargo build -p slug
TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug \
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short
git diff --check
```

If a phase changes the `slug` binary path used by Python tests, rebuild with
`cargo build -p slug` before invoking `target/debug/slug`.

Clean stale `slugd` processes before and after daemon-sensitive smokes.

## Current State (2026-06-26)

Plan 64 remains **In Progress** pending the residual full-guardrail failures
below, but the latest focused slices closed the lockfile/replay invalidation and
hidden-lockfile persistence gaps that were blocking selected-yanked,
hidden-lockfile, and reproducible-extension confidence:

- visible and hidden `MODULE.bazel.lock` inputs now poll through Slug's bzlmod
  DICE projection keys, including the resolved graph, current-workspace cell
  graph adapter, composed bzlmod cell graph, extension cell definitions, replay
  inputs, extension lookup keys, and extension-repo file-ops boundary;
- bzlmod `CellResolver` equality includes the current resolution digest so a
  lockfile-only replay identity change can invalidate package/configured graph
  consumers;
- extension replay identity includes selected lockfile metadata, and replay hits
  retain the selected cache's reproducible metadata for subsequent persistence;
- visible lockfile persistence uses only the visible lockfile as old visible
  state and filters reproducible entries out of the workspace lockfile branch;
- the hidden-facts create/edit/delete guardrail now runs in `--lockfile_mode=error`
  to avoid a visible workspace facts entry masking hidden facts, matching Bazel's
  source behavior rather than Slug's earlier test expectation.
- successful builds now split command-end extension persistence the way Bazel
  does: non-reproducible extension entries go to the visible workspace
  `MODULE.bazel.lock`, reproducible entries go to the daemon/output-base hidden
  `MODULE.bazel.lock`, and both carry relevant facts without copying registry
  hashes or selected-yanked state into the hidden file.
- `--lockfile_mode=error` no longer treats extra registry file hashes already
  present in `MODULE.bazel.lock` as stale by itself. The current-resolution
  missing-checksum error remains owned by registry resolution, so an extra old
  lockfile URL cannot mask the Bazel-shaped "missing checksum" failure for a
  current registry file.

Bazel source anchors:

- `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileFunction.java:67-88`
  reads visible/hidden lockfiles through Skyframe file dependencies.
- `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java:139-180`
  reads both lockfiles, prefers workspace facts when present, falls back to
  hidden facts, and prefers visible module-extension entries before hidden.
- `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java:354-364`
  intentionally does not diff-check facts when replaying a lockfile extension.
- `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileModule.java:120-123`
  starts command-end facts from the workspace lockfile, and
  `BazelLockFileModule.java:159-216` writes non-reproducible extension entries to
  the visible workspace lockfile and reproducible entries to the hidden lockfile.
- `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileValue.java:55-88`
  documents the visible/hidden lockfile split and the hidden lockfile's
  output-base location and permanence policy.
- `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/IndexRegistry.java:165-185`
  fails `--lockfile_mode=error` when the current registry file has no lockfile
  checksum; Slug should not add a reverse stale-extra-lockfile-URL check that
  masks that resolver-owned error.

Accepted validation:

```sh
TMPDIR=/var/mnt/dev/slug/.tmp cargo test -p slug_bzlmod \
  current_cell_graph_key_polls_injected_projection_identity -- --nocapture
TMPDIR=/var/mnt/dev/slug/.tmp cargo test -p slug_bzlmod \
  cell_graph_projection_keys_poll_extension_replay_inputs -- --nocapture
TMPDIR=/var/mnt/dev/slug/.tmp cargo test -p slug_bzlmod \
  lockfile_extension_data_preserves_reproducible_metadata -- --nocapture
TMPDIR=/var/mnt/dev/slug/.tmp cargo build -p slug
TMPDIR=/var/mnt/dev/slug/.tmp TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug \
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py::test_hidden_lockfile_facts_create_edit_delete_are_observed -s --tb=short
TMPDIR=/var/mnt/dev/slug/.tmp TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug \
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py::test_hidden_lockfile_edit_invalidates_replay_in_same_daemon \
  tests/core/bzlmod/test_plan61_guardrails.py::test_lockfile_selected_yanked_version_edit_invalidates_bzlmod_resolution -s --tb=short
TMPDIR=/var/mnt/dev/slug/.tmp cargo test -p slug_bzlmod lockfile -- --nocapture
TMPDIR=/var/mnt/dev/slug/.tmp cargo build -p slug
TMPDIR=/var/mnt/dev/slug/.tmp TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug \
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py::test_successful_build_persists_reproducible_extension_to_hidden_lockfile -s --tb=short
TMPDIR=/var/mnt/dev/slug/.tmp TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug \
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py::test_successful_build_persists_fresh_extension_result_to_lockfile \
  tests/core/bzlmod/test_plan61_guardrails.py::test_successful_build_persists_reproducible_extension_to_hidden_lockfile \
  tests/core/bzlmod/test_plan61_guardrails.py::test_hidden_lockfile_read_is_observable_before_extension_replay \
  tests/core/bzlmod/test_plan61_guardrails.py::test_hidden_lockfile_edit_invalidates_replay_in_same_daemon \
  tests/core/bzlmod/test_plan61_guardrails.py::test_hidden_lockfile_facts_create_edit_delete_are_observed -s --tb=short
TMPDIR=/var/mnt/dev/slug/.tmp cargo test -p slug_bzlmod \
  lockfile_lifecycle_error_mode -- --nocapture
TMPDIR=/var/mnt/dev/slug/.tmp cargo build -p slug
TMPDIR=/var/mnt/dev/slug/.tmp TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug \
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py::test_lockfile_missing_registry_checksum_invalidates_bzlmod_resolution -s --tb=short
TMPDIR=/var/mnt/dev/slug/.tmp TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug \
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -s --tb=short
```

Remaining gaps:

- Full `tests/core/bzlmod/test_plan61_guardrails.py` was rerun and is not clean:
  180 passed / 6 failed. The failures cluster around registry `source.json`
  warm invalidation counters, extension-facts error wording, post-write lockfile
  replay expectations, and `inject_repo` mapping replay after a MODULE edit.
  Fix or explicitly hand off these before declaring Plan 64 complete.
- Once Plan 64's focused lockfile/replay lane is no longer blocking, return
  ownership to Plan 34's REAPI executor proof instead of opening new
  compatibility lanes.

## Current Review Validation

The review itself ran one narrow command:

```sh
cargo test -p slug_common -- file_ops watched_abs
```

Result: passed, 6 tests. This proves the current Plan 63 unit surface is green;
it does not prove the same-daemon external-tree replay scenario in Phase 64.7.

## Implementation Notes for the Fresh Agent

- Start from `AGENTS.md`, then this plan, then Plan 62 and Plan 63.
- Treat Plan 62 completion labels as historical evidence, not as proof that this
  remediation queue is unnecessary.
- Do not use broad `//sdk:sdk_contents` success as acceptance for any phase here.
- Every parity claim needs observed Bazel 9.0.1 behavior or a source citation
  from `/var/mnt/dev/bazel`.
- Do not hold thread-blocking locks across async/DICE computations. This is a
  hard repo rule, not an optimization preference.
- Preserve unrelated dirty work and untracked artifacts until Phase 64.9
  classifies them.
