# Plan 61 Sub-plan 01: Registry-File Content-Addressing

> Parent: [Plan 61: True DICE-Owned Bzlmod](./61-true-dice-bzlmod.md)
>
> Created: 2026-05-29
>
> Addresses parent **Remaining Work item 2** (registry-cache out-of-root reads) and
> reduces a live per-transaction filesystem-poll bridge surface.

## Overview

`RegistryFileInputsKey` re-reads every locked registry cache file
(`bazel_registry.json`, `MODULE.bazel`, `source.json`) from `~/.cache/slug/registry/...`
and re-hashes it on **every DICE transaction**, because it depends on the
`validity()=false` child `AbsoluteTextFileInputKey`. The registry files are
checksum-pinned by the lockfile's `registry_file_hashes` (an already-tracked DICE
input), so the on-disk cache is a content-addressed blob whose identity is the
recorded hash. This plan makes the key a pure function of
`(project_root, registry_file_hashes)` for checksum-pinned (`http(s)`) registries —
fetching from disk/network only on cache miss — so warm builds stop re-reading the
registry cache and the registry class becomes genuinely DICE-owned.

`file:`/unsupported registries keep the current direct-read behavior (Bazel
treats `file:` registries as `IGNORE` — always read from disk).

## Current State Analysis

### The live bridge surface

`RegistryFileInputsKey::compute` (`app/slug_common/src/legacy_configs/cells.rs:2107-2212`):

- For each `(url, expected_hash)` in `self.registry_file_hashes`, it resolves the
  cache path (`cached_registry_file_path`, cells.rs:~1959) and calls
  `read_text_file_for_project_input(ctx, &project_fs, &path)` (cells.rs:2153-2154).
- The cache path is `~/.cache/slug/registry/...` (`ModuleCache::default_cache_dir`,
  `app/slug_bzlmod/src/cache.rs:79-128`), which never relativizes to the project
  root, so `read_text_file_for_project_input` falls through to
  `read_absolute_text_file_input_via_dice` → `ctx.compute(&AbsoluteTextFileInputKey{..})`
  (cells.rs:870-871, 874-882) and returns `BzlmodFileInputTracking::Polled`.
- `AbsoluteTextFileInputKey::validity` hard-returns `false` (cells.rs:763-767).
  **A `validity=false` child forces its parent to re-execute every transaction**,
  so `RegistryFileInputsKey::compute` re-reads and re-sha256s every registry file
  on every command — for a registry-heavy workspace, hundreds of `~/.cache` reads
  + hashes per warm no-op build.

### What the flags actually do (corrected during research)

- `RegistryFileInputsKey::validity` returns `value.cache_safe` (cells.rs:2221-2226),
  **not** `!has_untracked_inputs`. `cache_safe` is only set `false` for *unsupported*
  URL shapes (`cached_registry_file_path` returns `None`, cells.rs:2145-2150).
- `RegistryFileInputsValue.has_untracked_inputs` (set at cells.rs:2156) is **not**
  consumed by the parent graph validity. `BzlmodResolvedModuleGraphKey::validity`
  (`app/slug_bzlmod/src/dice_graph.rs:587-592`) checks only
  `lockfile_inputs.has_untracked_inputs()`. So the registry leaf does **not** force
  the whole graph to recompute — `BzlmodResolvedGraphSourceInputsValue`
  change-prunes via `registry_file_inputs.digest` equality (dice_graph.rs:478).
- Therefore the observable cost of this bridge is **the per-transaction disk
  re-read/re-hash at the registry leaf**, driven by the `validity=false`
  `AbsoluteTextFileInputKey` dependency — not a full-graph recompute. Removing the
  unnecessary disk dependency is the fix.

### Lockfile input is already tracked

`registry_file_hashes` is read from the **visible** lockfile
(cells.rs:2655-2665), which lives at `<project_root>/MODULE.bazel.lock` and is
tracked by DICE (`TrackedLockfileContentKey`, `tracked_by_dice=true` for in-project
paths). So keying the registry value on `registry_file_hashes` preserves
same-daemon invalidation: editing the lockfile changes the hashes → the key field
changes → recompute.

### Bazel 9 parity (verified against /var/mnt/dev/bazel)

- `IndexRegistry.doGrabFile` downloads `bazel_registry.json` / `MODULE.bazel` /
  `source.json` with `useChecksum=true`, using the lockfile-recorded SHA-256 from
  `knownFileHashes`; the recorded hash is the source of truth and the cached blob
  is content-addressed (not re-stat'd when the hash matches). `metadata.json` is
  explicitly `useChecksum=false` ("not immutable").
- `RegistryFactoryImpl` maps lockfile modes: `error → ENFORCE` (missing checksum
  throws), `refresh → USE_IMMUTABLE_AND_UPDATE`, `off/update → USE_AND_UPDATE`;
  `file:` scheme → `IGNORE` (always read from disk, never hash-pinned).
- `ExternalFilesHelper` (`ExternalFileAction.DEPEND_ON_EXTERNAL_PKG_FOR_EXTERNAL_REPO_PATHS`,
  default): absent `--watchfs`, external files (repo/registry cache, absolute
  paths) are re-stat'd each build via `handleDiffsWithMissingDiffInformation` +
  `ExternalDirtinessChecker`. Content-addressing the registry blob is exactly how
  Bazel avoids depending on that re-stat for immutable registry files.
- `FileStateValue` is `size`+`digest` (or size+mtime-proxy) and uses `equals()` as
  the change-pruning cutoff.

### Key Discoveries
- `RegistryFileInputsKey::compute` reads via `validity=false` `AbsoluteTextFileInputKey`: `app/slug_common/src/legacy_configs/cells.rs:2153-2154`, `740-768`.
- Graph validity ignores registry `has_untracked_inputs`: `app/slug_bzlmod/src/dice_graph.rs:587-592`.
- `registry_file_hashes` sourced from tracked visible lockfile: `cells.rs:2655-2674`.
- Cache dir is `~/.cache/slug/registry`: `app/slug_bzlmod/src/cache.rs:79-128`.
- Existing owning-abstraction tests for this key: `cells.rs:4021-4178` (currently assert `has_untracked_inputs` is true — these encode the bridge and must be updated).
- Bazel: `IndexRegistry.java`, `RegistryFactoryImpl.java` (`KnownFileHashesMode`), `ExternalFilesHelper.java`, `FileStateValue.java`.

## Desired End State

For a workspace whose registries are all `http(s)` with recorded hashes:
- `RegistryFileInputsKey` has **no `validity=false` child dependency**; its value is
  a pure function of `(project_root, registry_file_hashes)`.
- A warm no-op build does **not** re-read any `~/.cache/slug/registry` file.
- Editing the visible lockfile's `registry_file_hashes` still invalidates the key
  same-daemon.
- A cache miss still fetches (and a fetched-content checksum mismatch still errors,
  honoring `--lockfile_mode=error` parity).
- `file:`/unsupported registries keep direct-read behavior (`cache_safe=false`),
  matching Bazel's `IGNORE`.

Verify: new unit tests below pass; full parent guardrail suite stays green; a
registry-dependent warm build shows zero registry-cache reads via a counter.

## What We're NOT Doing

- **Not** building a watched absolute-path filesystem key or extending
  `slug_file_watcher` to new roots. (The watcher is project-root-scoped across all
  4 backends; that is a separate, larger effort.)
- **Not** addressing class (a) out-of-project `local_path_override` module/include
  files or class (c) hidden/output-base lockfile in this plan. Those still hit
  `AbsoluteTextFileInputKey`/`AbsolutePathMetadataInputKey` (validity=false). They
  are re-evaluated after this slice — they genuinely require either a watcher
  extension or an explicit Bazel-parity "external re-stat is acceptable" decision.
- **Not** changing `metadata.json` handling (Bazel keeps it mutable; Slug does not
  currently key it here).
- **Not** removing `AbsoluteTextFileInputKey` itself (still used by (a)/(c)).

## Implementation Approach

Make `RegistryFileInputsKey::compute` content-address checksum-pinned entries:
trust the recorded hash for DICE identity, touch disk only to populate the cache on
miss. Keep `file:`/unsupported entries on the existing direct-read/`cache_safe=false`
path. The key already hashes `(url, expected_hash)` into its digest
(cells.rs:2140-2143); the change is to stop creating the disk dependency and stop
re-reading present files for validity.

## Phase 1: Content-address checksum-pinned registry reads

### Overview
Replace the per-entry `read_text_file_for_project_input` (which creates the
`validity=false` dependency) with: if the entry is checksum-pinned and the cache
blob exists, contribute `expected_hash` to the digest with no DICE file dependency;
if absent, fetch (network/repo-cache) and verify the fetched bytes. `file:` and
unsupported URLs keep current behavior.

### Changes Required

#### 1. `RegistryFileInputsKey::compute`
**File**: `app/slug_common/src/legacy_configs/cells.rs` (~2107-2212)
**Changes**:
- For each `(url, expected_hash)`:
  - Resolve `cached_registry_file_path`; `None` → unsupported → `cache_safe = false`
    (unchanged).
  - For supported `http(s)` URLs: do **not** call `read_text_file_for_project_input`.
    Check blob existence cheaply *without* a tracked-or-`validity=false` DICE file
    dependency (a direct `Path::exists()`/`symlink_metadata` is acceptable here
    because the value's identity is `expected_hash`, not the file bytes — mirrors
    Bazel trusting the content-addressed repo cache). If present, fold
    `expected_hash` into the digest and continue. If absent, `fetch_missing_registry_file`,
    verify the fetched bytes' SHA-256 == `expected_hash` (error on mismatch — keeps
    `lockfile_mode=error` parity), and fold the hash in.
  - For `file:` URLs (detect scheme): keep the current
    `read_text_file_for_project_input` + verify path so local-file registries
    remain strict tracked/`Polled` reads (`cache_safe=false`), matching Bazel
    `IGNORE`.
- Do not set `has_untracked_inputs = true` for content-addressed entries; keep
  setting it for `file:`/unsupported direct reads.
- Keep the empty-`registry_file_hashes` early return (cells.rs:2115-2122).

#### 2. `RegistryFileInputsKey::validity`
**File**: `app/slug_common/src/legacy_configs/cells.rs:2221-2226`
**Changes**: Keep returning `cache_safe`. With the disk dependency removed for
pinned entries, a fully-pinned value (`cache_safe=true`) is now cached across
transactions (no `validity=false` child re-executing the parent). No signature
change required.

#### 3. Update existing tests that encode the bridge
**File**: `app/slug_common/src/legacy_configs/cells.rs:4021-4178`
**Changes**: The current tests assert `has_untracked_inputs == true` and that the
key re-polls. Update them to reflect content-addressing: a pinned registry value
has `has_untracked_inputs == false` and `validity == true`, and re-computation does
not occur on a second transaction with unchanged `registry_file_hashes`.

### Success Criteria

#### Automated Verification:
- [ ] New red→green unit test (see Testing Strategy) passes: `cargo test -p slug_common registry_file_inputs`
- [ ] `cargo test -p slug_common bzlmod`
- [ ] `cargo test -p slug_bzlmod`
- [ ] `cargo test -p slug_external_cells`
- [ ] `cargo build -p slug`
- [ ] `cargo fmt --check` and `git diff --check`
- [ ] `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short`

#### Manual Verification:
- [ ] On a registry-dependent workspace (e.g. `/var/mnt/dev/zeromatter-kuro`), a
  warm second build performs **zero** `~/.cache/slug/registry` reads (confirm via a
  read counter / `strace -e trace=openat -f` spot check or a DICE recompute log on
  `RegistryFileInputsKey`).
- [ ] `//sdk:sdk_contents` still reaches `BUILD SUCCEEDED` (frontier confirmation,
  not a close condition).

**Implementation Note**: After automated verification passes, pause for manual
confirmation of the warm-no-op read-count before updating the parent plan
checkpoint.

## Testing Strategy

### Unit Tests (owning abstraction — `slug_common` cells.rs tests)
1. **Red test first** — `registry_file_inputs_pinned_entry_is_content_addressed`:
   build a `RegistryFileInputsKey` over a temp cache (`cache_base_dir`) with one
   pinned `http(s)` URL whose cache blob is present and matches; assert
   `has_untracked_inputs == false`, `validity == true`, and that a second
   transaction does **not** re-read the blob (instrument the read path with a
   counter, or assert the key is not re-executed). Expected initial failure:
   `has_untracked_inputs == true` (current bridge behavior).
2. `registry_file_hashes` edit (different hash) → key recomputes (same-daemon
   invalidation preserved).
3. Cache miss + fetch returns mismatching bytes → checksum-mismatch error
   (`lockfile_mode=error` parity).
4. `file:` registry URL → still direct-read, `cache_safe=false`,
   `has_untracked_inputs == true` (Bazel `IGNORE` parity).
5. Unsupported URL shape → `cache_safe=false` (unchanged).

### Integration / same-daemon
- Reuse the parent Plan 61 Python guardrails for registry create/edit/delete; add a
  warm-no-op assertion that registry reads do not recur if a counter hook is
  available.

### Manual Testing Steps
1. Clean `slugd`; build a registry-dependent target; record registry-cache read
   count.
2. Re-run the same build warm; confirm registry-cache read count is 0.
3. Edit the visible lockfile's registry hash; confirm recompute/failure per mode.

## Performance Considerations

Eliminates O(number-of-locked-registry-files) disk reads + SHA-256 hashes per warm
command for registry-heavy workspaces. No new caching state beyond the existing
`~/.cache/slug` blob store.

## Migration Notes

None. Behavior change is internal to DICE input tracking; no on-disk format change.
Cold builds and cache-miss fetches behave as before.

## References

- Parent plan: `thoughts/shared/plans/slug-bazel-subplans/61-true-dice-bzlmod.md` (Remaining Work item 2; Target Shape `RegistryFileInputsKey` / lockfile policy)
- Bridge code: `app/slug_common/src/legacy_configs/cells.rs:2103-2227`, `740-768`, `2655-2674`
- Graph validity: `app/slug_bzlmod/src/dice_graph.rs:587-592`, `478`
- Cache dir: `app/slug_bzlmod/src/cache.rs:79-128`
- Bazel 9: `IndexRegistry.java`, `RegistryFactoryImpl.java`, `ExternalFilesHelper.java`, `FileStateValue.java` (under `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/`)
