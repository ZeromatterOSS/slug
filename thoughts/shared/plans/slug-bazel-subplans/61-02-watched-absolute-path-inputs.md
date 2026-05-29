# Plan 61 Sub-plan 02: Tracked Out-of-Project Filesystem Inputs

> Parent: [Plan 61: True DICE-Owned Bzlmod](./61-true-dice-bzlmod.md)
>
> Created: 2026-05-29
>
> Addresses parent **Remaining Work items 2 and 3** (out-of-project module/override
> files and out-of-project hidden lockfile) under the strict non-negotiable
> "no untracked filesystem reads".

## Overview

Out-of-project bzlmod inputs — `local_path_override` module/include files,
`git_override`/`archive_override` cached `MODULE.bazel` files, and the out-of-project
hidden lockfile — are read through `AbsoluteTextFileInputKey` /
`AbsolutePathMetadataInputKey`, whose `validity()` returns `false`
(`cells.rs:763-801`). DICE treats them as transient, so they cannot provide
change-pruning: every transaction re-reads them AND re-runs the whole bzlmod graph
(`has_untracked_inputs`/`tracked_by_dice=false` bubble to
`BzlmodResolvedModuleGraphKey::validity`).

The fix is to make them real cacheable DICE inputs invalidated by an explicit
per-sync re-stat-diff pass — which is exactly Bazel's model and also satisfies the
strict non-negotiable. The inotify-watcher version is a later pure performance
improvement, not a correctness requirement.

### Key correction vs an earlier draft

"Re-stat each build" is NOT an untracked read. Bazel's `ExternalDirtinessChecker`
re-stats `EXTERNAL_OTHER` files each build and injects a Skyframe invalidation only
when the `FileStateValue` (size+digest) changed. The DICE-faithful equivalent is a
**cacheable** key plus a **per-sync re-stat-diff** that calls `ctx.changed()` only on
change — identical in spirit to how `ProjectReadFileKey` is cacheable and dirtied by
the file watcher, but with poll-diff as the trigger instead of inotify. This:
- matches Bazel exactly (Bazel polls/re-stats external files; it does not inotify
  them, even with `--watchfs`, which only covers the workspace), and
- satisfies "no untracked filesystem reads" (the read is owned by a cacheable DICE
  key with explicit invalidation).

So Phase A (poll-diff) closes items 2/3 with no change to the non-negotiable. Phase B
(inotify) is a later optimization to skip the re-stat on warm no-ops.

## Current State Analysis

### The bridge
- `AbsoluteTextFileInputKey` / `AbsolutePathMetadataInputKey` (`cells.rs:738-802`):
  `validity()==false`, `compute()` does direct `std::fs`. Transient → no cutoff → full
  graph recompute each transaction.
- Out-of-project consumers (in-project goes through the watched `ProjectReadFileKey`):
  `read_bzlmod_file_for_module_inputs` (cells.rs:804-829), `local_override_module_dir_exists`
  (cells.rs:831-847), the hidden-lockfile read in `read_text_file_for_project_input` /
  `TrackedLockfileContentKey` (cells.rs:857-872, 943-1012), and direct
  `read_absolute_text_file_input` override/non-root reads (cells.rs:687, 1330, 1579, 1638).
- Validity bubbles via `has_untracked_inputs` / `tracked_by_dice=false` to
  `BzlmodResolvedModuleGraphKey::validity` and lockfile-input validity
  (`dice_graph.rs:587-592`, `1801-1809`).

### Invalidation machinery
- `FileChangeTracker` (`file_ops/dice.rs:282-411`): per-key dirty sets;
  `write_to_dice` → `ctx.changed(...)`. `ProjectReadFileKey` is cacheable and dirtied
  purely by watcher events; the whole project tree is pre-watched so any read is
  covered. Out-of-project paths lack that pre-watch — which is why poll-diff (re-stat
  each sync) is the correct trigger for them.
- `file_watcher.sync(ctx)` runs per command before computation (`ctx.rs:~674`); it
  receives the `DiceTransactionUpdater` and is the natural place to inject the
  out-of-project re-stat-diff.

### Key Discoveries
- Bridge: `app/slug_common/src/legacy_configs/cells.rs:738-802`.
- Tracker: `app/slug_common/src/file_ops/dice.rs:282-429`.
- Per-command sync injection point: `app/slug_server/src/ctx.rs:~674`.
- Bazel anchor: `ExternalFilesHelper.java` (EXTERNAL_OTHER), `DirtinessCheckerUtils.java`
  (`ExternalDirtinessChecker.check` re-stats + diffs `FileStateValue`), `FileStateValue.java`
  (size+digest equality = cutoff).
- Prior off-spec attempt (reverted) added a process-global registry whose "invalidation"
  only fired on in-project changes → stale. **Lesson: invalidation must come from an
  actual per-sync re-stat-diff of the out-of-project paths (or real OS watches), not a
  blanket redirty.**

## Desired End State

- Out-of-project override/module/include files and the out-of-project hidden lockfile are
  cacheable watched DICE inputs (`validity=true`).
- Warm no-op does not re-run bzlmod resolution; an out-of-project edit/create/delete is
  observed same-daemon and invalidates exactly the dependent nodes (via the per-sync
  re-stat-diff in Phase A; via inotify in Phase B).
- `has_untracked_inputs`/`tracked_by_dice=false` no longer set for these classes.
- No process-global correctness state: the registry + last-seen digests live in
  daemon/file-watcher state.
- `AbsoluteTextFileInputKey`/`AbsolutePathMetadataInputKey` removed from non-test
  production.

## What We're NOT Doing
- Not touching EdenFS (out of scope).
- Not re-doing the registry cache (done via content-addressing in sub-plan 01).
- Not changing in-project bzlmod input handling (already watched).
- Phase B (inotify) is optional/perf; not required to close items 2/3.

## Status

- **A.1 + A.2 LANDED** (commit `a60606ed`): cacheable `WatchedAbs*` keys + tracker
  channel + accessors; daemon-owned `WatchedAbsInputRegistry` + per-command
  re-stat-diff (`inject_watched_abs_changes`) + `DiceData`/daemon plumbing +
  `ctx.rs` injection. Behavior-preserving (registry empty until A.3 wires the
  reads). slug_common suite green (130 passed; pre-existing `persisted_cell_graph`
  executor-ordering failure also fails on HEAD).
- **A.3 cutover: validated, not yet landed.** A prototype of A.3 (route the 3
  cells.rs out-of-project helpers through the watched keys, return tracked, delete
  the `validity=false` bridge) was implemented and **passed the full Plan 61 pytest
  suite (178)** — the mechanism works end-to-end (hidden-lockfile/override edits
  observed same-daemon). It was reverted to land A.1+A.2 clean. Re-landing A.3
  requires updating the unit tests that encode the old untracked-poll semantics:
  `local_override_module_inputs_key_repolls_*` (3), `non_registry_override_..._repolls_same`,
  `non_root_module_files_key_repolls_same`, and `bzlmod_clean_lockfile_inputs_key_tracks_hidden_lockfile_fail_open`
  — each needs: set the registry in `DiceData` (`set_watched_abs_input_registry`),
  flip `assert!(x.has_untracked_inputs)` → `assert!(!x.has_untracked_inputs)`, and
  call `inject_watched_abs_changes` between the edit and the re-read (see the
  rewritten `out_of_project_module_include_reads_are_watched_and_invalidate` for the
  pattern). The facts guardrail `test_hidden_lockfile_facts_create_edit_delete_are_observed`
  also needs its over-strict eval-count proxy on the restore step relaxed to a
  build-outcome assertion (restoring to a previously-evaluated facts state is a
  correct DICE cache hit). `persisted_cell_graph` is a pre-existing unrelated flake.

## Phase A — Bazel-parity poll-diff (closes items 2/3, strict-compliant)

### A.1 Cacheable absolute-path keys + tracker channel
**Files**: `app/slug_common/src/file_ops/dice.rs`
- Add `WatchedAbsFileKey(Arc<AbsNormPathBuf>)` (digest + lazy content) and
  `WatchedAbsPathMetadataKey(Arc<AbsNormPathBuf>)`; `validity=true`, `equality` on
  digest/metadata.
- Add `abs_files_to_dirty` / `abs_paths_to_dirty` to `FileChangeTracker` +
  `abs_file_contents_changed` / `abs_path_added_or_removed` mutators; extend
  `write_to_dice`.
- `DiceFileComputations::read_watched_abs_file_if_exists` /
  `read_watched_abs_path_metadata_if_exists`.

Success (automated): `cargo test -p slug_common file_ops`; `cargo build -p slug`.

### A.2 Daemon-owned out-of-project input registry + per-sync re-stat-diff
**Files**: `app/slug_server/src/daemon/state.rs`, `app/slug_server/src/ctx.rs`,
`app/slug_common` (registry type)
- Add `WatchedAbsInputRegistry` (set of absolute paths + last-seen digest/metadata) in
  daemon state (NOT a `LazyLock` static). Register API called by the bzlmod read paths.
- In the per-command sync (alongside `file_watcher.sync`, `ctx.rs:~674`): re-stat each
  registered path, diff against last-seen, and for changed ones call the new
  `FileChangeTracker` abs mutators → `ctx.changed(...)`. Update last-seen. This is the
  `ExternalDirtinessChecker` analog. No first-read gap (every sync re-stats all
  registered paths; a newly-registered path's first value is freshly read, then diffed
  on subsequent syncs).

Success (automated): unit test that a registered path's edit injects a `ctx.changed`
for its key; `cargo build -p slug`.

### A.3 Wire the bzlmod out-of-project classes
**Files**: `app/slug_common/src/legacy_configs/cells.rs`
- Route the out-of-project branches (`read_bzlmod_file_for_module_inputs`,
  `local_override_module_dir_exists`, hidden-lockfile read, direct
  `read_absolute_text_file_input` override/non-root reads) through the A.1 keys,
  registering each path with the registry at read time.
- Stop setting `has_untracked_inputs`/`tracked_by_dice=false` for these (they are now
  tracked); `BzlmodResolvedModuleGraphKey` becomes cacheable for these workspaces.
- Make `AbsoluteTextFileInputKey`/`AbsolutePathMetadataInputKey` `#[cfg(test)]`-only or
  delete — removing the `validity=false` poll bridge from production (`cells.rs:763-801`).

Success (automated): `cargo test -p slug_common bzlmod`/`slug_bzlmod`/`slug_external_cells`;
`cargo build -p slug`; `cargo fmt --check`; `git diff --check`; `rg` confirms the old keys
are non-production.

### A.4 Same-daemon guardrails per replay-input class
**Files**: `tests/core/bzlmod/test_plan61_guardrails.py`
- Add/strengthen same-daemon edit/create/delete coverage for: out-of-project
  `local_path_override` MODULE.bazel + included segment; cached `git_override`/
  `archive_override` MODULE.bazel; out-of-project hidden lockfile edit/create/delete/facts.
  Each asserts: (a) warm no-op does not recompute resolution, (b) an out-of-project edit
  invalidates same-daemon, (c) no stale value served (the off-spec failure mode), incl.
  an edit-immediately-after-first-read case.

Success (automated):
`TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short`.

## Phase B — inotify optimization (pure perf, later)

Skip the per-sync re-stat for registered paths whose parent dir has a live inotify
watch; fall back to poll-diff for the rest. Extend `notify.rs` to install
`NonRecursive` watches on registered out-of-project parent dirs and carry absolute
events into the abs tracker channel. watchman/fs_hash_crawler parity is a further
follow-on. Phase B changes performance only, not correctness — Phase A already closes
items 2/3.

## Risks / Notes
- **No process-global correctness state**: registry + last-seen in daemon state (fixes
  the prior off-spec bug).
- **Per-sync cost**: re-stat of a small, bounded set (registered bzlmod inputs only) —
  same order as Bazel's external dirtiness check; Phase B removes it on warm no-ops.
- **Digest vs mtime**: use content digest for files (like `FileStateValue` size+digest)
  so an mtime-only touch does not invalidate.

## References
- Parent: `61-true-dice-bzlmod.md` (items 2, 3; Target Shape `ModuleSourceKey`, `LockfileContentKey`)
- Sub-plan 01 (landed): `61-01-registry-content-addressing.md`
- Bridge: `app/slug_common/src/legacy_configs/cells.rs:738-802`
- Tracker/sync: `app/slug_common/src/file_ops/dice.rs:282-429`, `app/slug_server/src/ctx.rs:~674`
- Bazel: `ExternalFilesHelper.java`, `DirtinessCheckerUtils.java`, `FileStateValue.java`
