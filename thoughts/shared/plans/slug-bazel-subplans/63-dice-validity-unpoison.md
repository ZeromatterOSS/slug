# Plan 63: Un-poison DICE validity overrides (warm-build fix)

> Parent: [Plan 62: Bzlmod replay parity followups](./62-bzlmod-replay-parity-followups.md)
>
> Created: 2026-06-11
>
> Follow-up owner: [Plan 64](./64-plan62-implementation-review-remediation.md)
> Phase 64.7 adds the same-daemon semantic replay guardrail that proves the
> generation marker edge invalidates external-tree package/file reads end to end.
>
> Root-cause investigation: `/var/mnt/dev/tmp/dice_commit_fix_plan.md`

## TL;DR

Commit 39726f2a added `fn validity(_x) -> bool { false }` to six file-IO DICE keys
as a staleness stopgap. In DICE, `validity=false` makes every computed value
TRANSIENT: never stored in the VersionedGraph, and transiency propagates to
every transitive dependent. Since ~all build keys transitively read files
through these keys, the entire build graph is recomputed every command. The fix
is to remove the overrides and close the three invalidation gaps the stopgap
was papering over.

## Phase 1 — un-poison the WatchedAbs keys (3 deletions)

Delete `validity` overrides from:
- `WatchedAbsFileKey` (dice.rs ~824)
- `WatchedAbsPathMetadataKey` (dice.rs ~870)
- `WatchedAbsDirEntriesKey` (dice.rs ~994)

Replace with `fn validity(x) -> bool { x.is_ok() }` (upstream convention: IO
errors stay transient). Delete the misleading doc comments about mutability
that the stopgap added (lines 816-823, 862-869, 991-993).

Correctness guaranteed by registry re-stat-diff (already built, plan 61-02).

**Expected result:** most of the build graph (AnalysisKey, BuildKey,
EvalImportKey, InterpreterResultsKey, etc.) becomes warm-cacheable. Project*
cone remains transient until Phase 3.

## Phase 2 — close within-command re-materialization window

Add `ExternalTreeGenerationKey { tree_root: Arc<PathBuf> }` in slug_common
file_ops/dice.rs. Its compute reads the repo's `.slug_repo_complete` marker
file via raw fs (one tiny read per repo, ~160 total). This key stays transient
(`validity=false`) — it's the only key that needs to be.

In `compute_watched_abs_*`: if path is under a registered mutable-tree root,
`ctx.compute(&ExternalTreeGenerationKey)` first. Tree roots registered into
DiceData by slug_external_cells at delegate construction.

This creates a dep edge so re-materialization at version v invalidates cached
WatchedAbs leaves through normal dep checking (Bazel's
RepositoryDirectoryValue pattern).

## Phase 3 — un-poison the Project* keys + close invalidation gaps

1. Delete `validity=false` from ProjectReadFileBytesKey (~630),
   ProjectPathMetadataKey (~665), ProjectReadDirEntriesKey (~1075);
   replace with `x.is_ok()`.

2. **Gap A:** Drop `is_bzlmod_config_project_file` filter in
   `project_file_contents_changed` (dice.rs ~437-448) — always dirty on change.

3. **Gap B:** Add `project_dir_entries_to_dirty: HashSet<ProjectReadDirEntriesKey>`
   to FileChangeTracker; populate with parent dir on file create/delete events;
   emit in `write_to_dice`.

4. **Gap C:** For Project* reads under registered tree roots (bazel-external/),
   add `ExternalTreeGenerationKey` dep. Split repository_materialization_state
   reads into "protocol reads" (marker probe — stays transient) and "content
   reads" (gain generation dep, become cacheable).

## Implementation status

### Phase 1 — DONE

Replaced `fn validity(_x) -> bool { false }` with `fn validity(x) -> bool { x.is_ok() }`
in all three WatchedAbs keys + deleted the misleading stopgap doc comments:
- `WatchedAbsFileKey` (dice.rs ~829)
- `WatchedAbsPathMetadataKey` (dice.rs ~878)
- `WatchedAbsDirEntriesKey` (dice.rs ~1010)

### Phase 2 — DONE

- Added `ExternalTreeGenerationKey { tree_root: Arc<PathBuf> }` in dice.rs (~line 1019).
  Reads `.slug_repo_complete` via raw fs.
- Extended `WatchedAbsInputRegistry` with `register_tree_root()` and
  `is_under_tree_root()` methods (watched_abs.rs).
- Added generation-dep preamble to all three `WatchedAbs*::compute` functions:
  if the path is under a registered tree root, compute `ExternalTreeGenerationKey` first.
- Registered tree roots at all three delegate-construction call sites in
  `get_file_ops_delegate` (extension_repo.rs ~lines 749, 847, 949).

### Phase 2.5 — Generation key cacheability fix (this session)

The generation key was `validity=false` (transient), which transitively
re-poisoned every dependent WatchedAbs/Project* key — the dep edge was wired
but neither freshness mechanism was active. The `equality()` content-compare
was dead code, and nothing ever invalidated the key.

Two-part fix:

1. `ExternalTreeGenerationKey::validity` changed from `false` to `x.is_ok()`
   (makes the generation node cacheable; activates the existing `equality()`
   which short-circuits dependents when the marker content is unchanged).

2. Added marker-content tracking to the per-sync re-stat-diff:
   - `WatchedAbsInputState.tree_generation_markers: HashMap<PathBuf, Option<String>>`
     tracks last-seen `.slug_repo_complete` digest per registered tree root.
   - `WatchedAbsChanges.tree_roots_changed: Vec<PathBuf>` carries changed roots.
   - `diff_and_update()` now re-stats each tree root's marker and compares.
   - `FileChangeTracker.ext_tree_generation_to_dirty: HashSet<ExternalTreeGenerationKey>`
     and `ext_tree_generation_changed()` method emit `ctx.changed()` for
     generation nodes whose marker changed.
   - `inject_watched_abs_changes()` wires the new bucket.

Correctness: marker unchanged → generation node hit → dependents hit (warm
reuse restored). Marker changed by re-materialization → generation node
invalidated → content differs → dependents recompute.

### Phase 3 — DONE (gaps A, B, C)

- Un-poisoned `ProjectReadFileBytesKey`, `ProjectPathMetadataKey`,
  `ProjectReadDirEntriesKey`: `validity=false` → `x.is_ok()`.
- Gap A: `project_file_contents_changed` now dirties ALL changed project files
  unconditionally; only the `project_files_requiring_pre_config_commit` flag
  remains gated on `is_bzlmod_config_project_file`.
- Gap B: Added `project_dir_entries_to_dirty: HashSet<ProjectReadDirEntriesKey>`
  to `FileChangeTracker`; populated with parent dir on file change; emitted in
  `write_to_dice`.
- Gap C: Project* keys under mutable tree roots now depend on the same
  `ExternalTreeGenerationKey` that WatchedAbs keys use. The project-relative
  path is converted to absolute via `io.project_root().root().join(self.0)`
  and matched against the existing abs registry with `is_under_tree_root()`.
  No separate `ProjectTreeGenerationKey` or project-relative tree-root registry
  needed — one shared generation node per tree, one source of truth for
  invalidation.

### Test fix

- Updated `recorded_input_markers_match_lockfile_format_through_dice_reads` in
  repository_materialization_state.rs to explicitly inject invalidation via
  `FileChangeTracker` between transactions (previously relied on `validity=false`
  forcing re-reads).

### Verification

- `cargo build -p slug` — 0 errors
- `cargo test -p slug_common -- file_ops watched_abs` — all passed (6 tests
  including new `diff_detects_generation_marker_change`)
- `cargo test -p slug_external_cells` — 13 passed
