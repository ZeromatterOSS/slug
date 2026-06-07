# Plan 62: Bzlmod Replay and Bazel 9 Parity Follow-ups

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Status: Proposed
>
> Created: 2026-06-07

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

## Phase 1: External-Cell File Ops Must Be DICE-Tracked

**Severity:** High.

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

## Phase 2: Refresh-Mode Registry Metadata Replay

**Severity:** High.

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

## Phase 3: Multiple-Version Canonical Module Identity

**Severity:** High / Medium.

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

## Phase 4: Extension Unique-Name Disambiguation

**Severity:** Medium.

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

## Phase 5: Bazel 9 Lockfile Shape

**Severity:** Medium.

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

## Phase 6: Identity Hygiene and Guardrail Gaps

**Severity:** Low / Medium.

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
