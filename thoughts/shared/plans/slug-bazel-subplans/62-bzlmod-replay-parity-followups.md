# Plan 62: Bzlmod Replay and Bazel 9 Parity Follow-ups

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Status: In Progress
>
> Created: 2026-06-07
>
> Updated: 2026-06-11 — live implementation review reopened residual follow-ups
> in [Plan 64](./64-plan62-implementation-review-remediation.md). Historical
> phase completion labels below record landed work, but Plan 62 is not considered
> closed until Plan 64 resolves materialization locking/publish, download/auth,
> lockfile lifecycle, digest honesty, and same-daemon replay guardrail gaps.
>
> Updated: 2026-06-10 — major implementation landed and //sdk:sdk_contents built
> successfully (8360 actions, 33m08s total). Materialization-lock deadlock fixes
> landed as ea34f863 and d2de2e93, but the 2026-06-11 review found that the
> remaining blocking-lock shape still needs redesign under Plan 64.
>
> Updated: 2026-06-08 — added Part B (Phases 7-15) from the write/fetch +
> MVS-semantics + lifecycle audit. Part A = replay/parity (Phases 1-6, original
> plan). Part B = security, hermeticity, MVS correctness, and lockfile lifecycle.

## Goal

Own the post-Plan-61 audit findings that are real bzlmod replay/parity work but
do not mean the old Plan 61 resolver bridge is still alive.

Plan 61 closed the structural migration from a legacy bzlmod bridge to named
DICE-owned graph producers. This plan covers the remaining edges where the
implementation is still not fully Bazel-9-parity-shaped or can replay stale
state below those graph producers.

## Current Judgment

The implementation is not slop: root/non-root module inputs, lockfile inputs,
registry file hashes, override modules, repo mappings, extension replay inputs,
and repository materialization manifests are substantially DICE-shaped.

2026-06-11 review update: the implementation is directionally strong but not
done. Plan 64 is now the owner for residual review findings: status drift,
blocking materialization locks held across async/DICE work, remove-then-rename
publish windows, warning-only download auth/header handling, unwired production
lockfile writing, fallible digest honesty, semantic external-tree replay
coverage, and bzlmod string/data-structure audit work.

The implementation is also not fully replay-correct or Bazel-9-complete:

- some external-cell file delegates read mutable generated/external trees
  directly under cacheable DICE keys;
- refresh-mode registry/yanked metadata is mutable but not a tracked DICE input;
- multiple-version module canonical names collapse to unversioned module names;
- extension-generated repo prefixes do not implement Bazel's unique-name
  disambiguation;
- lockfile parsing/serialization still accepts or emits non-Bazel-9 shapes;
- some identity digests and guardrails are weaker than the semantics they name.

## Non-Goals

- Do not reopen Plan 61 unless the old resolver bridge or process-global graph
  ownership becomes a production dependency again.
- Do not add Bazel 8 or old Slug compatibility. Bazel 9.0.1 parity is the only
  target.
- Do not use broad SDK success as proof for these issues. Each item needs a
  focused owning-abstraction regression.
- Do not special-case zeromatter, rules_rust, rules_python, or BCR labels when a
  generic bzlmod/replay rule is required.

## Phase 1: External-Cell File Ops Must Be DICE-Tracked — COMPLETE

**Severity:** High.
**Completed:** 2026-06-07 (Plan 61 — BzlmodFileOpsDelegate now routes through
compute_watched_abs_file/dir_entries/path_metadata; ExtensionRepoFileOpsDelegate
similarly routed in Plan 61)

**Problem:** `slug_external_cells` delegates for bzlmod and extension-generated
repos read files, directories, symlink metadata, and file bytes directly with
`tokio::fs` while their results sit under cacheable `ReadDirKey`,
`PathMetadataKey`, and related file-op keys.

**Evidence:**

- `app/slug_external_cells/src/extension_repo.rs`: `ExtensionRepoFileOpsDelegate`
  ignores `DiceComputations` and reads from `source_path` directly.
- `app/slug_external_cells/src/bzlmod.rs`: `BzlmodFileOpsDelegate` does the same
  for registry-backed module cells.
- `app/slug_common/src/file_ops/dice.rs`: `ReadDirKey` and `PathMetadataKey`
  cache successful delegated results as valid.

**Work:**

1. Add focused same-daemon regressions proving stale package/file state after an
   external generated repo tree changes.
2. Route bzlmod/extension-repo delegate reads through DICE-backed project or
   watched-absolute file APIs, or make the delegated cache keys invalid when the
   underlying external tree is not DICE-trackable.
3. Include file bytes, directory entries, symlink metadata, and symlink targets.
4. Re-check `RepositoryRuleFileOpsDelegate` and `LocalPathFileOpsDelegate` for
   the same bug class; move any confirmed sibling issue into this phase.
5. Fix `expand()` for `ExternalCellOrigin::ExtensionRepo` so it can trigger
   lazy materialization through DICE instead of direct `source_path.exists()`.

**Exit criteria:**

- Same-daemon edit/create/delete of generated-repo package files invalidates
  package loading or analysis for a clear DICE reason.
- `expand()` can materialize a registered extension repo before copying it.
- No production external-cell delegate whose results are cached as valid reads a
  mutable semantic file tree via bare `tokio::fs`/`std::fs`.

## Phase 2: Refresh-Mode Registry Metadata Replay — COMPLETE

**Severity:** High.
**Completed:** 2026-06-08 (commit 39726f2a + Plan 61 DICE invalidation)
**Evidence:** `BzlmodResolvedModuleGraphKey::validity()` returns false under
`LockfileMode::Refresh` (dice_graph.rs:674), forcing re-resolution. Yanked
version checks return `Unknown` in refresh mode (resolution.rs:1601). Registry
file hash validation is skipped in refresh mode (resolution.rs:1627).

**Problem:** `--lockfile_mode=refresh` says to re-resolve, but
`BzlmodResolvedModuleGraphKey` can remain valid when lockfile inputs are tracked.
Mutable registry/yanked metadata is fetched directly through `RegistryClient`
rather than a DICE-owned input.

**Evidence:**

- `app/slug_bzlmod/src/lockfile.rs`: `LockfileMode::Refresh` policy.
- `app/slug_bzlmod/src/resolution.rs`: yanked-version checks use registry
  metadata during resolution.
- `app/slug_bzlmod/src/registry.rs`: metadata fetch path reads mutable registry
  state.
- `app/slug_bzlmod/src/dice_graph.rs`: resolved graph validity only consults
  lockfile input tracking.
- Bazel source of truth: `IndexRegistry`, `BazelModuleResolutionFunction`, and
  the registry/yanked-version Skyframe path under
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/`.

**Work:**

1. Add a same-daemon refresh-mode regression where registry metadata changes
   between invocations and Slug must not reuse the previous graph.
2. Model refresh registry metadata as a DICE input with explicit refresh-mode
   invalidation, or mark the affected graph key invalid under refresh.
3. Preserve checksum-pinned registry file behavior for normal locked operation;
   do not turn the content-addressed cache exception into generic polling.
4. Verify `--lockfile_mode=error`, default/update, refresh, and off each follow
   Bazel 9 behavior.

**Exit criteria:**

- Refresh mode reobserves mutable registry/yanked metadata in the same daemon.
- Non-refresh locked operation still uses lockfile-pinned registry file hashes.
- Missing checksums and checksum mismatches in error mode fail before mutable
  content is accepted.

## Phase 3: Multiple-Version Canonical Module Identity — COMPLETE

**Severity:** High / Medium.
**Completed:** 2026-06-08 (commit 39726f2a)

**Problem:** Multiple-version override selection can produce keys like
`name+version`, but graph construction strips back to `name` and inserts module
state by the plain module name. Canonical module repo naming also returns
`module+` even when Bazel would require a versioned canonical repo.

**Evidence:**

- `app/slug_bzlmod/src/resolution.rs`: `select_versions` creates
  `name+version` keys, while `build_resolved_graph` strips to `actual_name` and
  stores modules under that plain name.
- `app/slug_bzlmod/src/dice_graph.rs`: `bazel_canonical_module_repo_name` ignores
  version.
- Bazel source of truth:
  `BazelDepGraphFunction.computeCanonicalRepoNameLookup` and
  `ModuleKey.getCanonicalRepoNameWithVersion`.

**Work:**

1. Add a focused multiple-version override workspace where two versions of the
   same module must coexist and produce distinct canonical repos.
2. Keep resolved graph keys as Bazel-shaped `ModuleKey` identities rather than
   plain module names where multiple versions can exist.
3. Make canonical repo naming include the version only for modules with multiple
   selected versions, matching Bazel.
4. Audit repo mappings, cell graph construction, module-source projection,
   toolchain registration, and extension aggregation for plain-name assumptions.

**Exit criteria:**

- Two selected versions of one module do not overwrite graph entries, cells,
  repo mappings, extension usages, or registered toolchains.
- Canonical repo names match Bazel 9 for both single-version and multiple-version
  module graphs.

## Phase 4: Extension Unique-Name Disambiguation — COMPLETE

**Severity:** Medium.
**Completed:** 2026-06-08 (commit ebb795a8)

**Problem:** Slug derives generated-repo prefixes from
`{owning_module}+{extension_name}` and isolation-key fragments, but does not
implement Bazel's unique-name disambiguation for colliding extension IDs.

**Evidence:**

- `app/slug_bzlmod/src/extension_execution_dice.rs`: `extension_repo_prefix`.
- Bazel source of truth:
  `BazelDepGraphFunction.calculateUniqueNameForUsedExtensionId` and
  `makeUniqueNameCandidate`.

**Work:**

1. Add a regression with two different extension IDs whose first unique-name
   candidate collides or is a prefix conflict.
2. Compute extension unique names from the full used-extension-id set, not from
   each extension in isolation.
3. Thread the unique name into extension repo prefixing, lockfile replay, spoke
   lookup, repo mappings, and generated cell registration.

**Exit criteria:**

- Generated repo canonical names are unambiguous for colliding extension IDs.
- Lockfile replay and fresh extension execution agree on the same unique names.

## Phase 5: Bazel 9 Lockfile Shape — COMPLETE

**Severity:** Medium.
**Completed:** 2026-06-08 (commit dea549bf)

**Problem:** Slug still accepts older lockfile versions as usable state and
serializes repo specs with Slug-specific shape.

**Evidence:**

- `app/slug_bzlmod/src/lockfile.rs`: lockfile version comments say Bazel 9
  version 26, but parsing accepts older versions and tests assert old-version
  compatibility.
- `LockfileRepoSpec` includes a top-level `local` field.
- `attr_value_to_json` serializes labels as `{ "__label__": ... }`.
- Bazel source of truth: `BazelLockFileFunction.getLockfileValue`,
  `BazelLockFileValue.LOCK_FILE_VERSION`, `RepoSpec`, and
  `AttributeValuesAdapter`.

**Work:**

1. Change lockfile parsing to accept only version 26 as usable state.
2. Match Bazel 9 behavior for old lockfiles:
   - error mode reports unsupported lockfile version;
   - update/default behavior treats old data as unusable according to Bazel.
3. Remove Slug-only lockfile repo-spec fields from Bazel-visible lockfile JSON,
   or keep them only in a Slug-private cache that cannot be confused with
   `MODULE.bazel.lock`.
4. Serialize/deserialze label attrs in the Bazel-compatible string/escaping
   shape.

**Exit criteria:**

- A Bazel 9 lockfile round trip is structurally compatible at the `RepoSpec` and
  attribute-value level.
- Old lockfile versions no longer feed extension replay or registry facts as
  valid Bazel 9 state.

## Phase 6: Identity Hygiene and Guardrail Gaps — COMPLETE

**Severity:** Low / Medium.
**Completed:** 2026-06-08 (commit a3722447)

**Problems:**

- `cell_graph_resolution_digest` omits the post-resolution
  `NonRootModuleFilesKey` result even though those parsed modules are folded into
  graph outputs.
- Two-workspace collision tests do not force same-daemon sharing.
- Recorded-input replay coverage is edit-heavy; missing-to-present and
  present-to-missing cases are thinner.
- File registry strict-tracking coverage is weaker than `http(s)` registry
  content-addressing coverage.

**Work:**

1. Include non-root module-file identity in the graph/cell-graph resolution
   digest, or rename the digest so downstream keys cannot mistake it for full
   graph identity.
2. Add same-daemon two-workspace collision guardrails for colliding module names,
   extension names, generated repo names, and lockfile entries.
3. Add recorded-input create/delete replay cases for `FILE`, `DIRENTS`,
   `DIRTREE`, `ENV`, and `REPO_MAPPING` where applicable.
4. Add same-daemon `file:` registry create/edit/delete coverage.
5. Add watched label file/directory/tree delete cases for module and repository
   materialization paths.

**Exit criteria:**

- Every propagated bzlmod identity digest names exactly the semantic inputs it
  covers.
- Guardrails cover edit/create/delete for the replay input classes listed in
  Plan 61's completion criteria, including same-daemon collision cases.

## Validation Matrix

Run focused regressions first, then the standard bzlmod matrix:

```sh
cargo test -p slug_bzlmod
cargo test -p slug_common bzlmod
cargo test -p slug_external_cells
cargo build -p slug
TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short
```

For parity-shape fixes, include a local Bazel 9.0.1 source citation from
`/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/`
or observed Bazel 9 behavior.

## Completion Criteria

- All high-severity stale-replay surfaces are fixed or reclassified with focused
  evidence.
- Multiple-version module identity and extension unique-name disambiguation match
  Bazel 9.
- Bazel-visible lockfile parsing and serialization are Bazel-9-shaped.
- External-cell file operations under cacheable DICE keys are tracked,
  invalidated, or deliberately non-cacheable.
- The validation matrix passes after the focused regressions are added.

---

# Part B: Write/Fetch Safety, MVS Semantics, and Lifecycle (2026-06-08 audit)

Part A (Phases 1-6) covers replay/parity of the *resolution and DICE* layers.
This audit found a second, mostly-untouched category: the code that actually
fetches, extracts, and writes repositories to disk is not production-safe, the
MVS engine has two real semantic bugs, and the lockfile has no production writer.
The slop census is reassuring — production code has zero `todo!`/`unimplemented!`,
no swallowed `Result`s in core logic, a 2.9:1 test:prod ratio, and clean
`slug_error` usage. This is competent code with real gaps, not slop. Part B fixes
the gaps.

## How to work Part B (for the implementing agent)

Read this before starting any phase.

1. **One phase = one PR.** Do not batch phases. Each phase is independently
   reviewable and independently revertible. Phases 7, 8, 9 are security/safety
   and should go first, in that order.
2. **TDD. Write the failing test first.** Every phase below names the exact test
   to add and the exact command to run it. Add the test, watch it fail for the
   right reason, then fix, then watch it pass. Do not write the fix first.
3. **Verify every `file:line` before editing.** Line numbers below are from the
   2026-06-08 audit and will drift. Open the file, confirm the code matches the
   "Evidence" description, and re-locate by symbol name (given in each phase) if
   the line moved. If the code no longer matches the description, STOP and report
   — do not invent a fix for code that changed.
4. **Bazel 9.0.1 is the only parity target.** When a fix needs Bazel behavior you
   are unsure of, cite the Bazel source under
   `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/`
   in the PR. If you cannot find the parity answer there and cannot derive it,
   STOP and ask a human rather than guessing.
5. **Do not weaken existing tests to make new code pass.** If an existing test
   asserts old behavior that a phase intentionally changes, update that test in
   the same PR and call it out explicitly in the PR description.
6. **Security phases (7, 8, 9) must fail closed.** When in doubt, error out
   rather than continue with partial/unverified state. Never downgrade an error
   to a warning to make a test pass.
7. **Run the per-phase test plus `cargo build -p slug` before declaring a phase
   done.** Initial testing means: the new regression passes, the existing
   `slug_bzlmod` / `slug_external_cells` suites still pass, and the binary builds.
   Full Bazel-matrix parity validation can be left for human review where noted.

Severity-ranked order of attack:

| Phase | Title | Severity | Class |
|------|-------|----------|-------|
| 7 | Archive extraction path-traversal + symlink containment | CRITICAL | Security |
| 8 | Atomic + concurrency-safe materialization | CRITICAL/HIGH | Safety |
| 9 | Download integrity enforcement + hermetic fetch + auth | HIGH | Hermeticity |
| 10 | MVS: compatibility_level + fixpoint discovery | CRITICAL | Correctness |
| 11 | Registry: multi-registry fallback + mirrors | HIGH | Parity |
| 12 | module_ctx fidelity fakes + env-tracking symmetry | HIGH | Correctness |
| 13 | Cache-dir portability (env-derived value digests) | HIGH | Replay |
| 14 | Lockfile writer + mode enforcement + interop digests | HIGH/MED | Lifecycle |
| 15 | Swallowed-error hardening | MED/LOW | Robustness |

## Phase 7: Archive Extraction Path-Traversal and Symlink Containment — COMPLETE

**Severity:** CRITICAL (security — arbitrary file write outside repo root).
**Completed:** 2026-06-08 (commit 39726f2a)

**Problem:** Tar extraction joins raw archive entry paths onto the destination
directory with no containment check. A crafted or buggy archive containing `../`
entries or absolute paths escapes `bazel-external/{name}` and writes anywhere the
daemon can write. Symlink and hard-link entries are materialized with their raw
target with no containment check. The zip path is already safe (it uses
`enclosed_name()`); only the tar path (the common case) is vulnerable.

**Evidence (verified 2026-06-08):**

- `app/slug_bzlmod/src/repository_executor.rs`, fn that extracts tar entries
  (around line 1585-1670): the destination is computed as
  `dest_dir.join(stripped)` and the no-strip-prefix branch is
  `dest_dir.join(&*path)` with `path` taken straight from the tar header. No
  check that the result stays under `dest_dir`.
- Symlink branch (~1640): `std::os::unix::fs::symlink(&*link_target, &dest_path)`
  with no validation that `link_target` resolves inside `dest_dir`.
- Hard-link branch (~1646-1665): `resolve_tar_link_target(...)` then
  `hard_link`/`copy` with no containment check on the source.
- Zip branch (~1740) uses `file.enclosed_name()` and is the correct model to copy.

**Work:**

1. Add a failing regression first. New test in `repository_executor.rs` (or a
   sibling test module): build an in-memory `.tar.gz` containing an entry named
   `../escape.txt` (and a second test with an absolute `/tmp/escape.txt` entry,
   and a third with a symlink whose target is `../../escape`). Extract into a
   temp dir and assert extraction returns `Err(...)` and that no file was created
   outside the destination directory. Use the `tar` crate's `Builder` to author
   the malicious archive in the test.
2. Add a single containment helper, e.g.
   `fn contain_path(dest_dir: &Path, candidate: &Path) -> Result<PathBuf>`, that
   rejects any candidate that, after normalizing `.`/`..` *lexically* (do NOT
   canonicalize — the target may not exist yet and canonicalize follows
   symlinks), is not a descendant of `dest_dir`. Reject absolute entry paths
   outright. Return a typed `RepositoryExecutionError` with `ErrorTag::Input`.
3. Route every tar destination (`dest_dir.join(...)` in both strip-prefix and
   no-strip-prefix branches) through the helper. Do the same for the parent dir
   used in `create_dir_all`.
4. For symlink entries: reject the entry if the link, resolved relative to its
   own location, escapes `dest_dir`. Bazel does allow in-repo relative symlinks,
   so containment (not "no symlinks") is the rule.
5. For hard-link entries: run the resolved source through the same containment
   helper before `hard_link`/`copy`.
6. Confirm the zip path already enforces containment via `enclosed_name()`; if a
   strip_prefix join on the zip side reintroduces a raw join, route it through
   the helper too.

**Exit criteria:**

- A tar entry with `../`, an absolute path, or an escaping symlink/hard-link
  target causes extraction to fail with a typed error and writes nothing outside
  the destination.
- In-repo relative symlinks (target stays inside the repo) still extract
  successfully — add a positive test for this so containment isn't over-tightened.
- Existing extraction tests still pass.

**Test command:** `cargo test -p slug_bzlmod repository_executor`

## Phase 8: Atomic and Concurrency-Safe Materialization — COMPLETE WITH PLAN 64 FOLLOW-UP

**Severity:** CRITICAL (crash leaves live partial tree) / HIGH (concurrent race).
**Completed:** 2026-06-08 (commit 39726f2a, parking_lot fix ea34f863, self-deadlock fix d2de2e93)
**Review follow-up:** 2026-06-11 review found residual gaps owned by Plan 64:
the per-canonical-name `parking_lot::Mutex` is still held across async
repository-rule execution, and `finalize_staging_dir` still removes the old
non-empty canonical directory before retrying rename. Treat this phase as
historically implemented but not finally closed.

**Problem:** Repository materialization removes the canonical output directory in
place and then writes the new tree directly into the final path. A crash or
cancellation mid-fetch leaves a partial tree at the canonical path that direct
file-ops readers will happily read (they read the path, not the completion
marker). There is also no filesystem lock on `bazel-external/{name}`: two DICE
computations with differing `repo_env`/`repo_mappings` are distinct keys, so they
will both `remove_dir_all` and rewrite the same directory and race on disk.

**Evidence (verified via audit; re-locate by symbol):**

- `app/slug_bzlmod/src/repository_executor.rs`: `prepare_working_dir`
  (~line 567) `remove_dir_all`s and writes into the final dir.
- `app/slug_bzlmod/src/repository_execution.rs`: materialization compute
  (~lines 2751-2795) removes then writes the canonical path; completion marker is
  written last (good for *reuse* decisions, useless for *concurrent readers*).
- `ExtensionRepoExecutionKey` eq/hash (~line 397-420) includes `repo_env` and
  `repo_mappings`, so two non-equal keys can target the same canonical dir.

**Work:**

1. Add a failing regression: a test that materializes into a temp output base,
   then simulates a partial/aborted materialization (e.g. write a partial tree
   plus no marker), and asserts the next materialization does not expose the
   partial tree to a reader — i.e. it rebuilds atomically. A concurrency test
   (two `tokio` tasks materializing the same canonical name) asserting the final
   tree is complete and the digest matches is the stronger version; add it if the
   harness supports driving two computes.
2. Materialize into a unique temp dir (sibling of the canonical path, same
   filesystem so rename is atomic), then `rename` into place only after the tree
   is fully written and verified. On any error, delete the temp dir and leave the
   existing canonical path untouched. This is the Bazel `_tmp` → final-rename
   model.
3. Add a per-canonical-name lock so concurrent materializations of the same
   output path serialize. Prefer an in-process async lock keyed by canonical name
   (e.g. a `DashMap<CanonicalName, Arc<tokio::sync::Mutex<()>>>` owned by the
   materialization manager) over an OS file lock, since within one daemon all
   materializations share a process. Document that cross-daemon concurrency on
   the same output base is out of scope here (note it for a future phase).
4. Ensure the completion marker is written only after the rename, so the marker
   can never describe a partially-renamed tree.

**Exit criteria:**

- No code path writes a non-final repository tree directly at the canonical path.
- A crash/error during materialization leaves either the prior good tree or no
  tree, never a partial tree, at the canonical path.
- Two concurrent materializations of the same canonical name serialize and both
  observe a complete tree.

**Test command:** `cargo test -p slug_bzlmod repository_execution repository_executor`

## Phase 9: Download Integrity Enforcement and Hermetic Fetch — COMPLETE WITH PLAN 64 FOLLOW-UP

**Severity:** HIGH (hermeticity / supply-chain).
**Completed:** 2026-06-08 (commit 39726f2a, auth param 9daea904)
**Review follow-up:** 2026-06-11 review found `repository_ctx` and `module_ctx`
still warn-and-ignore non-empty `auth`/`headers`, and shared downloads still
shell out through `curl`/`wget`. Plan 64 owns completing or explicitly rejecting
those semantics.

**Problem:** Three issues in the download path:
(a) `http_archive` integrity is optional and only verified when a hash is
present, so an unpinned archive downloads and extracts with zero verification and
no warning; (b) downloads shell out to ambient `curl`/`wget`, inheriting host
proxy/netrc/env and failing opaquely when those binaries are missing; (c)
`auth`/`headers` parameters on `download`/`download_and_extract` are accepted but
silently ignored, so auth-gated mirrors return 401/403 with no diagnostic.

**Evidence (verified 2026-06-08):**

- `app/slug_bzlmod/src/repository_executor.rs`: `sha256`/`integrity` read via
  `get_optional_string` (~606-607) and only verified `if let Some(...)`
  (~1319-1364); curl at ~1401, wget at ~1436.
- `app/slug_interpreter_for_build/src/repository_ctx.rs`: `auth`/`headers` are
  `#[allow(unused_variables)]` in `download`/`download_and_extract`
  (~2187-2193, 2269-2278).
- `app/slug_interpreter_for_build/src/module_ctx/methods.rs`: same ignore
  (~168-173, 244-247).

**Work:**

1. Decide the unpinned-download policy to match Bazel 9: Bazel *allows* unpinned
   downloads but records them and (with `--incompatible_*` flags) can require
   integrity. The minimum correct fix here is to **emit a visible warning event**
   when an archive is downloaded with neither `sha256` nor `integrity`, recording
   the computed hash in the warning so the user can pin it. Do NOT silently
   accept. Confirm exact Bazel behavior in the bzlmod/repository sources before
   choosing warn-vs-error; cite it in the PR.
2. Replace `curl`/`wget` shell-outs with the in-process HTTP client already used
   elsewhere (`RegistryClient` / the native executor download path that already
   has connect+total timeouts). If a full replacement is too large for one PR,
   at minimum: thread `--connect-timeout` into the curl path to match the native
   path's timeouts and detect missing-binary with a clear typed error. Prefer the
   in-process client; note any deferral explicitly.
3. Wire `auth` and `headers` through to the actual request. `auth` is a dict of
   `{url_prefix: {"type","login","password"} | {"pattern"}}`; apply matching
   credentials as headers per Bazel's `downloader` semantics. If full `auth`
   parsing is deferred, at least pass `headers` through and emit an explicit
   "auth not yet supported" error (not a silent ignore) when `auth` is non-empty,
   so users get a diagnostic instead of a 401.
4. Add tests: (a) unpinned download produces a warning event carrying the
   computed hash; (b) a download with a wrong `sha256` fails with a typed
   integrity error (likely already covered — keep it); (c) `headers` supplied by
   the rule reach the request (assert against a local test server or a mock).

**Exit criteria:**

- An unpinned archive download is never silently accepted; it warns (or errors,
  per confirmed Bazel 9 behavior) and surfaces the computed hash.
- `headers` (and ideally `auth`) reach the HTTP request, or `auth` produces an
  explicit unsupported-feature error rather than a silent ignore.
- Download has bounded connect and total timeouts on every path.

**Test command:**
`cargo test -p slug_bzlmod repository_executor && cargo test -p slug_interpreter_for_build repository_ctx`

## Phase 10: MVS Semantic Correctness — compatibility_level and Fixpoint Discovery — COMPLETE

**Severity:** CRITICAL (silently wrong resolution vs Bazel).
**Completed:** 2026-06-08 (commit 39726f2a; note: 846e191e removed compatibility_level per Bazel 9 parity)

**Problem A — compatibility_level is force-zeroed.** The parser stores
`compatibility_level: 0` unconditionally and discards the declared value with a
comment claiming "Bazel 9 accepts compatibility_level but stores 0." That belief
is wrong: Bazel 9 still uses `compatibility_level` as a hard MVS conflict gate —
two selected versions of one module with different compatibility levels is a
resolution error that forces the user to upgrade. Because every module's level is
0 here, `check_compatibility_conflicts` can never fire, and a genuine
cross-compatibility-level conflict that Bazel rejects is silently collapsed to a
single version.

**Problem B — single-pass discovery.** Module discovery is a single-pass BFS over
the literally-requested `(name, version)` pairs. Bazel discovers the transitive
deps of the *selected* version and iterates to a fixpoint. If MVS bumps a module
to a higher version whose `MODULE.bazel` introduces new deps, those new deps can
go undiscovered here, under-resolving the graph.

**Evidence (verified 2026-06-08):**

- `app/slug_bzlmod/src/globals.rs` (~690-700): `compatibility_level: 0` then
  `let _ = compatibility_level;`.
- `app/slug_bzlmod/src/parser.rs` (~280): copies the (already-zeroed) decl level.
- `app/slug_bzlmod/src/resolution.rs`: `check_compatibility_conflicts` (~1036)
  is dead because all levels are 0; discovery BFS (~639-720) enqueues deps of
  requested versions only.
- Bazel source of truth: `Discovery.java`, `ModuleFileFunction`, and
  `BazelModuleResolutionFunction` (compatibility-level conflict check) under the
  bzlmod dir; `Version`/`ModuleKey` for compatibility-level handling.

**Work:**

1. **Part A test first:** a workspace where the root depends (transitively) on
   two modules requiring the same dependency at versions with *different*
   `compatibility_level`s. Assert resolution errors with a compatibility-level
   conflict message matching Bazel's shape.
2. Stop zeroing `compatibility_level`: parse and store the declared value in
   `globals.rs`; ensure it flows through `parser.rs` → `BzlModule` →
   resolution. Remove the misleading comment and the `let _ =` discard.
3. Confirm `check_compatibility_conflicts` now fires correctly and that
   `multiple_version_override` pairs allowed versions by compatibility level the
   way Bazel does (this connects to Part A Phase 3's multiple-version work — note
   the dependency between the phases in the PR).
4. **Part B test first:** a workspace where MVS bumps module `A` from the
   requested `1.0` to `2.0` (because another module requires `A@2.0`), and where
   `A@2.0`'s `MODULE.bazel` adds a new dep `B` that `A@1.0` did not have. Assert
   `B` appears in the resolved graph.
5. Make discovery iterate to a fixpoint: after version selection, re-expand any
   module whose selected version differs from the version whose deps were already
   discovered, until no new modules or version bumps appear. Match Bazel's
   `Discovery` + selection loop. Keep it deterministic (sorted work queue) so the
   resolved graph digest is stable.

**Exit criteria:**

- A cross-compatibility-level conflict errors the way Bazel 9 does.
- A version bumped upward by MVS contributes its newly-introduced transitive deps
  to the resolved graph.
- Existing resolution tests pass; resolved-graph digests remain deterministic.

**Test command:** `cargo test -p slug_bzlmod resolution`

## Phase 11: Registry Multi-Registry Fallback and Mirrors — COMPLETE

**Severity:** HIGH (parity — multi-registry and mirrored setups break).
**Completed:** 2026-06-08 (commit 39726f2a — RegistryChain)

**Problem:** Only a single registry base URL is supported per client. Bazel
accepts `--registry` multiple times with ordered fallback (try each registry in
order until the module is found) and rewrites archive source URLs through
`bazel_registry.json` `mirrors` / `module_base_path`. Neither ordered fallback
nor mirror rewriting is implemented; `fetch_bazel_registry_json_file` exists but
its `mirrors`/`module_base_path` are never parsed or applied.

**Evidence (verified via audit):**

- `app/slug_bzlmod/src/registry.rs`: single base URL (~222-247);
  `fetch_bazel_registry_json_file` (~380) result unused for mirroring;
  `find_best_version` has a `// TODO: semver range` (~427-455) but appears
  unused by MVS.
- Bazel source of truth: `IndexRegistry`, `RegistryFactoryImpl`, and the
  `--registry` flag handling; `bazel_registry.json` mirror schema.

**Work:**

1. Test first: configure two registries where the module exists only in the
   second; assert resolution finds it via fallback. Second test: a
   `bazel_registry.json` with a `mirrors` entry; assert the archive URL is
   rewritten through the mirror.
2. Accept an ordered list of registry base URLs (from repeated `--registry` /
   buckconfig) and try each in order for metadata/source/module-file fetches,
   stopping at the first hit. Record which registry served each file for the
   lockfile `registryFileHashes` keying (Bazel keys by full URL).
3. Parse `bazel_registry.json` `mirrors` and `module_base_path` and apply them
   when constructing source/archive URLs, matching Bazel's rewrite order.
4. Leave `find_best_version` semver-range as a separate documented gap unless a
   live caller appears; do not expand scope here.

**Exit criteria:**

- A module present only in a later registry resolves via ordered fallback.
- Archive URLs are rewritten through configured registry mirrors.
- Lockfile registry-file-hash keys remain per-URL and stable.

**Test command:** `cargo test -p slug_bzlmod registry resolution`

## Phase 12: module_ctx Fidelity Fakes and Env-Tracking Symmetry — COMPLETE

**Severity:** HIGH (extensions consulting these get wrong answers; env changes
miss invalidation).
**Completed:** 2026-06-10 (env-tracking + root_module_direct_deps validation; is_dev_dependency was already fixed in 39726f2a)

**Problem:** Several `module_ctx` accessors are hardcoded placeholders rather
than real values, and there is an env-tracking asymmetry between `repository_ctx`
and `module_ctx`:
(a) `module_ctx.is_dev_dependency()` always returns `false`, ignoring its
argument; (b) `root_module_direct_deps` and `root_module_direct_dev_deps` always
return `None` instead of the documented label lists; (c) `module_ctx.os` reads
(e.g. `mctx.os.environ[...]`) record no env input, so an env change does not
invalidate the extension — while `repository_ctx.os` over-records *every* env var.

**Evidence (verified 2026-06-10):**

- `is_dev_dependency` — fixed in commit 39726f2a (returns real per-usage dev-dep status).
- `root_module_direct_deps` / `root_module_direct_dev_deps` — removed as spurious
  struct fields (Bazel 9 does NOT expose them as module_ctx attributes; they are
  only parameters to `extension_metadata()`). Added `RootModuleDirectDeps` enum
  with `Unset`/`All`/`Explicit(IndexSet<String>)` variants. `extension_metadata()`
  now validates both parameters matching Bazel 9 parity: both must be set or both
  unset, at most one "all", explicit lists must be disjoint. `ModuleExtensionMetadata`
  extended to carry the validated values.
- `module_ctx.os.environ` env-tracking — now records all env vars as DICE inputs
  via `record_all_repo_env_inputs()`, matching `repository_ctx.os` behavior.
  Files: `context.rs` (added method + wired into `get_attr("os")`).
- Tests: 6 new tests for `validate_root_module_deps` (None/All/Explicit/invalid
  string/duplicates), 1 new test for os.environ recording. All 136
  slug_interpreter_for_build tests pass, all 441 slug_bzlmod tests pass.

**Work:**

1. Tests first: (a) an extension that calls `module_ctx.is_dev_dependency(tag)`
   and branches — assert it sees the real per-usage dev-dependency status; (b) an
   extension reading `root_module_direct_deps` — assert it gets the real label
   list; (c) a same-daemon regression where a module extension reads an env var
   via `mctx.os.environ` and the env var changes between invocations — assert the
   extension re-executes.
2. Implement `is_dev_dependency` from the aggregated usage data (the aggregation
   layer already tracks dev-dep status per usage; thread it into the ctx).
3. Populate `root_module_direct_deps` / `root_module_direct_dev_deps` from the
   resolved root module's direct deps.
4. Make `module_ctx.os.environ` access record env inputs the same way
   `repository_ctx` does, and tighten `repository_ctx.os` to record only the env
   vars actually read rather than all of them (match Bazel, which records per-key
   reads). Keep `getenv`/`which` recording, which is already correct.

**Exit criteria:**

- `is_dev_dependency`, `root_module_direct_deps`, and
  `root_module_direct_dev_deps` return real values matching Bazel.
- A module extension that reads an env var re-executes when that var changes;
  reading one env var does not invalidate on unrelated env changes.

**Test command:** `cargo test -p slug_interpreter_for_build module_ctx`

## Phase 13: Cache-Dir Portability (Env-Derived Value Digests) — COMPLETE

**Severity:** HIGH (value digests not portable across machines/daemons).
**Completed:** 2026-06-08 (commit 39726f2a — content-based digest, no path in digest)

**Problem:** The module cache base directory is derived from `$XDG_CACHE_HOME` /
`dirs::home_dir()` and that absolute path is folded into
`RegistryFileInputsValue.digest` and override cache dirs. Process env / home dir
is not a tracked DICE input, so the same logical workspace produces different
value digests across machines and environments — replay/cache-sharing across
hosts is broken even though the inputs are identical.

**Evidence (verified via audit):**

- `app/slug_bzlmod/src/cache.rs` (~79-88): `default_cache_dir()` reads env/home.
- `ModuleCache::new()` is called inside DICE computes
  (`legacy_configs/cells.rs` ~2013/2111/2116; `dice_graph.rs` ~730/807/1089).
- The cache base path is hashed into `RegistryFileInputsValue.digest`
  (`cells.rs` ~2133).

**Work:**

1. Test first: compute a registry-file-inputs value digest twice with two
   different cache base directories but identical registry content; assert the
   digests are equal (they currently differ).
2. Stop folding the absolute cache path into any value digest. The digest must be
   a function of *content* (registry file bytes / hashes), not of where the cache
   happens to live. Use relative/content keys inside the digest.
3. If the cache directory genuinely must influence behavior, make it an explicit
   injected DICE key so it is tracked, rather than an ambient `std::env` read
   mid-compute. Prefer option (2) — remove it from identity entirely.
4. Audit other compute-time `std::env`/`home_dir` reads flagged in the audit
   (`TempPatchDir` naming via `SystemTime::now()`/`pid` is scratch-only and OK;
   confirm nothing else leaks ambient state into a cached value).

**Exit criteria:**

- Value digests for identical registry content are identical regardless of cache
  directory location.
- No absolute machine-specific path is part of any cached value's identity.

**Test command:** `cargo test -p slug_bzlmod cache && cargo test -p slug_common bzlmod`

## Phase 14: Lockfile Writer, Mode Enforcement, and Interop Digests — COMPLETE WITH PLAN 64 FOLLOW-UP

**Severity:** HIGH (no production writer; `--lockfile_mode` mostly cosmetic) /
MED (extension digests not Bazel-interoperable).
**Completed:** 2026-06-08 (commit b3679927, build fix a36685ec)
**Review follow-up:** 2026-06-11 review found no production caller of
`Lockfile::write_for_purpose(...)`, and digest helpers still need a fallible
stable-serialization audit. Plan 64 owns the production lockfile lifecycle and
digest-honesty closure.

**Problem:** Three connected gaps:
(a) `write_for_purpose` has no production caller and there is no `slug mod update`
command, so the lockfile is read-only in practice — `update` mode never updates
and the file is never created/refreshed by a build; (b) `--lockfile_mode` is
largely cosmetic: `error` mode only catches extension-fact drift and
registry-checksum gaps, and the `StaleLockfile`/`LockfileModeError` variants are
dead; (c) `bzlTransitiveDigest` / `usagesDigest` use Slug-private algorithms that
share Bazel's *encoding* but not its *content*, so a Bazel-written lockfile's
extension entries always miss and Slug's are not Bazel-replayable — the headline
cross-tool reproducibility promise does not hold for the extension section.

**Evidence (verified 2026-06-08):**

- `app/slug_bzlmod/src/lockfile.rs`: `write_for_purpose` (~1111-1163) gated to
  `ExplicitModUpdate`; only test caller (`write` → `Test`, ~1122). Dead variants
  `StaleLockfile`/`LockfileModeError` (~90-100).
- `app/slug_bzlmod/src/extension_execution_dice.rs` (~2420-2431) and
  `extensions.rs` (~330-356): Slug-private digest algorithms.
- Bazel source of truth: `BazelLockFileModule` (the writer hook on build
  completion), `BazelLockFileFunction`, and the extension digest computation in
  `ModuleExtensionEvalStarlarkThreadContext` / `SingleExtensionEvalFunction`.

**Work — split into two PRs (14a writer/mode, 14b digests):**

14a:
1. Test first: run a build in `update` mode against a workspace with no lockfile;
   assert a `MODULE.bazel.lock` is written. Run in `error` mode with a lockfile
   whose resolved graph no longer matches; assert the build fails with a
   stale-lockfile error.
2. Wire a production writer: on successful resolution, in `update`/default mode,
   write the lockfile via the existing `write_for_purpose` with a real
   production `LockfileWritePurpose`. Decide and document the trigger point (end
   of resolution / build completion) by matching Bazel's `BazelLockFileModule`.
3. Implement `error`-mode enforcement for the cases beyond extension facts:
   resolved-graph drift, registry-file-hash drift, yanked-version drift. Use the
   dead `StaleLockfile` / `LockfileModeError` variants. Make `off` truly skip,
   `refresh` re-fetch (connect to Part A Phase 2).
4. If a `slug mod` command surface is needed to host explicit updates, scope it
   minimally and note it; do not build a full `mod` CLI in this phase.

14b:
5. Decision (2026-06-25): keep Slug extension lockfile digests explicitly
   Slug-private until byte-for-byte Bazel parity is implemented with a golden
   test. Bazel 9 computes `usagesDigest` via
   `SingleExtensionUsagesValue.hashForEvaluation`, which hashes Gson JSON for
   `trimForEvaluation()`
   (`/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionUsagesValue.java:78-81`),
   writes it in
   `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java:284-288`,
   and compares it in
   `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java:339-342`.
   Bazel's `.bzl` transitive digest comes from
   `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/RegularRunnableExtension.java:209-210`.
6. Slug now documents `compute_extension_input_hash` as private cache identity
   and hashes extension inputs with field-wise binary tags. A future interop
   slice must introduce a Bazel golden before claiming shared lockfile extension
   digest compatibility.

**Exit criteria:**

- A build writes/refreshes `MODULE.bazel.lock` in update/default mode.
- `error` mode fails on resolved-graph, registry-hash, and yanked drift, not just
  extension facts; dead lockfile-mode variants are either used or removed.
- Extension digests are clearly marked Slug-private; byte-for-byte Bazel 9.0.1
  parity remains a future golden-test-backed interop slice.

**Test command:** `cargo test -p slug_bzlmod lockfile`

## Phase 15: Swallowed-Error Hardening — COMPLETE

**Severity:** MED (silently-incorrect repos) / LOW.
**Completed:** 2026-06-08 (commit 39726f2a)

**Problem:** A handful of failure paths in the fetch/extract layer are swallowed
with `.ok()` and execution continues, which can yield a silently-incorrect
repository. The slop census confirmed core logic does *not* swallow errors, so
this is a small, targeted cleanup of specific sites, not a systemic problem.

**Evidence (verified via audit):**

- `app/slug_bzlmod/src/repository_executor.rs`: patch application in native
  http_archive logs failures as "non-fatal" and continues (~810-841) — a failed
  patch yields a silently-unpatched repo; tar `extract` ignores `std::io::copy` /
  `set_permissions` / `create_dir_all` errors via `.ok()` (~1613-1635);
  `materialize_llvm_multicall_aliases` ignores all link/copy errors (~699-720).
- `app/slug_interpreter_for_build/src/repository_ctx.rs`:
  `ensure_label_path_materialized` swallows materialization errors and continues
  with a possibly-dangling path (~1130-1138).

**Work:**

1. Patch-application failure in `http_archive` must be fatal (Bazel fails the
   repo if a patch does not apply). Add a test with a patch that does not apply
   and assert the repo errors. Remove the "non-fatal" downgrade.
2. In tar extraction, propagate `std::io::copy` and `create_dir_all` errors
   (`set_permissions` may stay best-effort, matching Bazel's leniency on mode on
   some filesystems — confirm and comment). This pairs naturally with Phase 7.
3. `ensure_label_path_materialized`: propagate the materialization error instead
   of continuing with a dangling path; add a test asserting a clear error rather
   than a later mysterious missing-file failure.
4. Review the remaining `.ok()` sites in these two files and convert any that can
   mask an incorrect repo into propagated errors; leave genuinely best-effort
   cleanup (`remove_dir` of scratch) as-is with a one-line comment.

**Exit criteria:**

- A failed patch fails the repository, not silently skips.
- Extraction I/O failures and label-path materialization failures propagate as
  typed errors.
- Remaining `.ok()` discards in the fetch/extract layer are deliberate cleanup,
  each justified by a comment.

**Test command:** `cargo test -p slug_bzlmod repository_executor && cargo test -p slug_interpreter_for_build repository_ctx`

## Part B Validation Matrix

After each phase, run that phase's command plus:

```sh
cargo build -p slug
cargo test -p slug_bzlmod
cargo test -p slug_external_cells
cargo test -p slug_interpreter_for_build
cargo test -p slug_common bzlmod
```

After the security phases (7-9) and before merge, also run the end-to-end
guardrails:

```sh
TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug \
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short
```

## Part B Completion Criteria

- No archive can write outside its repository root; symlink/hard-link targets are
  contained (Phase 7).
- Materialization is atomic and concurrency-safe; no partial trees at canonical
  paths (Phase 8).
- Unpinned downloads are never silently accepted; fetch is in-process or
  bounded-timeout; `headers`/`auth` are honored or explicitly errored (Phase 9).
- `compatibility_level` is enforced and discovery reaches a fixpoint (Phase 10).
- Ordered multi-registry fallback and registry mirrors work (Phase 11).
- `module_ctx` fidelity accessors return real values; env reads are tracked
  symmetrically (Phase 12).
- Value digests are machine-portable (Phase 13).
- The lockfile is written by production builds, `--lockfile_mode` is enforced,
  and extension digests are Bazel-interoperable or honestly marked Slug-private
  (Phase 14).
- Targeted swallowed-error sites propagate instead of masking bad repos
  (Phase 15).
