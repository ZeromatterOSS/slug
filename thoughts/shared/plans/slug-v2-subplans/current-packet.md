# Current Slug V2 Packet

Packet: `WP-6-m2-root-action-closure-implementation`
Milestone: M2 recursive configured-target action closure
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: implement the accepted configuration-opaque command-local action
closure over existing root configured-target analysis values.
Predecessor: accepted integrated toolchain context `1533569f`, accepted
recursive target-owned action evidence, and reserved action-closure design
review.

Change root-analysis success payloads to share `Arc<AnalysisResult>` inside the
existing outer retained DICE result. Keep each `AnalysisResult` as the sole
owner of its target-local actions and direct dependency identities. Requested
build-target records retain those handles; `BuildCommandEvaluation` adds one
immutable `Arc<[Arc<AnalysisResult>]>` action closure. Its `analyses()` iterator
must expose that closure so existing declared-action counting and CLI/server
REAPI consumers include recursively owned actions, while
`analyzed_target_count()` remains the requested-root count.

Construct the closure inside the existing `BuildCommandRootKey` only after all
requested branches succeed. Seed unique analyzed roots in command-pattern
order, then traverse breadth-first frontiers. Within each frontier preserve
parent and declared dependency order, deduplicate first-seen nodes by the full
existing opaque `ConfiguredTargetKey`, and batch child reads through the
existing `RootConfiguredTargetAnalysisKey`. Duplicate roots/shared diamonds
appear once; the same label under distinct existing configurations remains
distinct. This is deterministic retention order, not Bazel-generic traversal
parity or action scheduling.

For every closure frontier, inspect all discovered unique child outcomes,
union all Needs, and return Need before the first BFS-order terminal analysis
error. DICE infrastructure failure follows the existing invariant path. Do
not cross a Need frontier or publish a partial `BuildCommandEvaluation`.
Re-reading a child key adds the required command invalidation edge but must not
evaluate its rule twice or create a second event owner/batch.

Production allowlist:

- `app/slug_analysis_v2/src/dice.rs`
- `app/slug_core_v2/src/runtime/dice.rs`

Test allowlist:

- `app/slug_analysis_v2/tests/root_analysis.rs`
- `app/slug_analysis_v2/tests/starlark_rule.rs` for mechanical successful
  helper return-type adaptation only
- inline tests in `app/slug_core_v2/src/runtime/dice.rs`
- `app/slug_cli_v2/tests/cli.rs`

Caps are 360 formatted production net lines, 650 test lines, 180 documentation
lines, and 1,190 total net lines.

Required evidence covers the accepted parent/second/first three-action
closure; roots-first multi-root layers; duplicate-root and diamond dedupe;
configuration-distinct same-label nodes without configuration display; cold
and warm activation with no duplicate evaluation or target-local event batch;
child-only action edit, delete/recreate, orphan pruning, and full A-to-B-to-A
command equality; same-frontier Need-before-error; public declared action count
three; and the existing REAPI iterator observing all three independent fixture
actions without executing them in the test.

Stop and return `REPLAN` for a third production file, result deep cloning into
the retained command closure, action aggregation into `AnalysisResult`, a new
DICE key/cache/global/lock/interner, scheduler or execution behavior,
configuration formatting/identity, configured paths, action keys/platforms,
cquery/aquery formatting, cycle semantics, external mapping/patterns,
toolchain action breadth, fixture/oracle regeneration, or any cap breach.
