# Current Slug V2 Packet

Packet: `WP-6-7A-root-module-files-observation-completion-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `d1755008`
Accepted rules_rust evidence: `b7390392`
Result: formal REPLAN from the umbrella generated-repository frontier to its
uniquely smaller first reusable owner, `RootModuleFilesKey`.

## Why this is the first complete owner

The accepted `HostRootModuleFileObservationKey` already performs the exact root
`MODULE.bazel` and recursive include evaluation, retains the complete path
epoch and owns the sole matching MODULE event batch. Its private
`HostRootModuleFileValue`, however, retains only the evaluated module,
overrides and module-file paths; `evaluate_root_module_terminal` discards the
already-produced `RootModuleEvaluation::extension_usages`.

`RootModuleFilesKey` is the existing semantic aggregation owner. It computes
root evaluation first and `VisibleLockfileKey` second, then retains the exact
`RootModuleFiles` value used by selected graph, registry policy and extension
mapping. The legacy visible-lockfile key reads `MODULE.bazel.lock` through a
carrierless workspace raw-file value. The later selected graph therefore cannot
retain an exact root MODULE + lockfile prefix by consuming current keys, and
re-reading either path above it would duplicate ownership.

Complete this owner before selected graph, registry or extension evaluation.
It is smaller than the previously audited generated-repository chain, reuses
the accepted root-module observation, and leaves every later semantic/event
owner in place.

## Exact authority and caps

This design packet is docs-only. Write only:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
2. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`;
4. `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Docs caps are <=40 canonical, <=200 current, <=160 Stage 6 and <=30 routing
net lines, <=430 aggregate. Rust, tests, fixtures, oracles, Cargo/BUILD and all
other plans are read-only during design.

After independent design ACCEPT, future Rust authority is exactly:

1. `app/slug_bzlmod_v2/src/host_module.rs`: <=80 production, <=120 test
   semantic lines, <=4,740 physical lines from the current 4,531 baseline;
2. `app/slug_bzlmod_v2/src/module_eval.rs`: <=180 production, <=220 test
   semantic lines, <=5,850 physical lines from the current 5,451 baseline.

Aggregate semantic growth is <=600 lines and combined physical size is
<=10,590. Preserve the two cohesive large-owner exceptions and keep every new
or touched helper below 200 lines. Any third Rust file or cap excess is REPLAN.

## Frozen key, carrier and shared-driver shape

Add one private structurally distinct `RootModuleFilesObservationKey` and one
private `ObservedRootModuleFiles` carrier containing exactly:

- one local `Arc<Result<RootModuleFiles, CompactString>>`; and
- one compact `PathObservationEpoch`.

Both implement `Dupe` and `Allocative`; expose only crate-private borrowed
result/epoch accessors needed by the later selected-graph owner. The key is not
re-exported and has no caller outside bzlmod in this packet.

Extend the private `HostRootModuleFileValue` with the exact already-evaluated
`Arc<[RootExtensionUsage]>`. Do not create a second extension-usage collection:
move the existing `RootModuleEvaluation::extension_usages` into the private
child value and then into the local `RootModuleFiles` Result. Preserve the exact
legacy root-module public behavior and event batch.

Use one mode-aware RootModuleFiles driver. Legacy selects only the existing
`RootModuleEvaluationKey` then `VisibleLockfileKey`, and projects the driver's
exact local Result Arc to `RootModuleFilesKey`. Observed selects only
`HostRootModuleFileObservationKey`, then a private observed visible-lockfile
projection in `module_eval.rs`, and moves the same local Result Arc plus epoch
into `ObservedRootModuleFiles`. Neither sibling computes the other.

The observed lockfile projection preserves exact `VisibleLockfileKey` order and
semantics: read `RootModuleLockfileModeKey` first; `LockfileMode::Off` returns
`VisibleLockfileRead::Ignored` without activating a file key; every other mode
computes the existing `HostFileBytesObservationKey` for
`MODULE.bazel.lock`, uses the same `parse_visible_lockfile_bytes_for_mode`, and
retains that child's exact FileBytes Result Arc in the epoch. Do not call
`WorkspaceRawFileKey`, `HostVisibleLockfileKey`, or a direct Host API from the
observed branch.

## Order and terminal algebra

Dependency order is root module closure first, then lockfile mode, then the
lockfile file only when the mode requires it. Merge the Complete root epoch
first. Merge a Complete lockfile epoch left-first before inspecting file or
parse semantics. Equal duplicate demands preserve the earlier exact Result Arc;
conflicting values or operation mismatch return the existing typed
`ObservedPathFrontierError`.

Terminal prefixes are exact:

- root child DICE compute failure is the existing semantic root-compute error
  with an empty epoch;
- root child semantic error retains the root epoch;
- lockfile-mode DICE failure retains the completed root prefix;
- Off-mode success retains only the root prefix and activates no lockfile file;
- lockfile FileBytes semantic error, missing/present parse error and success
  retain root + lockfile prefix; and
- overall success retains the same full reached prefix.

Root or lockfile Need and typed outer return immediately with no parent carrier
and no later activation. This sequential owner performs no Need union or joined
batch reduction. Need is invalid and self-unequal; Complete typed outer is valid
and equal by outer value; Complete carrier is valid/equal by semantic Result and
epoch.

## Event, retention and compatibility contract

`RootModuleFilesKey` and its observed sibling are eventless. The matching
`HostRootModuleFileKey`/`HostRootModuleFileObservationKey` remains the sole
owner of its local MODULE evaluation batch, including empty/error-prefix
batches under existing semantics. Lockfile handling owns no event. Need, typed
outer and cancellation publish nothing; warm reuse suppresses child replay.

The observed parent retains exactly the local RootModuleFiles Result Arc and
compact epoch. Root/lockfile child carriers, source strings, AST/evaluator,
include horizon/ancestry, parser scratch, event buffer and union scratch remain
dependency-owned or compute-local. The semantic Result may retain its existing
module, overrides, extension-usage and lockfile collections; add no collection
outside that Result Arc and no cache, store, interner, lock, task, direct Host
read, revision or certificate.

Exact: root MODULE/include evaluation, extension-usage values/order, visible
lockfile modes/values/errors, legacy RootModuleFiles Result and child event
text/order. Slug-native: the structural observed sibling, compact epoch and
typed outer. Unsupported/deferred: selected-module graph/registry discovery,
extension definition/evaluation/instantiation, generated repository
route/package loading, external rules_rust analysis/actions, M8/M7B and exact
Bazel identity bytes.

## Required proof

The implementation proof must discriminate:

- distinct key identity/hash/Display, `Dupe`/`Allocative`, equality and
  validity for Need, typed outer and carrier;
- exact legacy Result/value/error parity and exact legacy projection Arc;
- root `use_extension`/`use_repo(..., "rust_toolchains")` usages and order
  surviving through the real production RootModuleFiles result;
- observed semantic parity, exact root then lockfile epoch membership/order and
  per-demand `Arc::ptr_eq`, with zero added demands;
- LockfileMode Off no-file activation, plus missing/present/bad lockfile and
  mode-input/compute error prefixes;
- root and lockfile Need/typed outer/semantic positions, carrierlessness and
  later-child suppression, stable duplicate first Arc, conflict and operation
  mismatch;
- observed-to-zero-legacy and legacy-to-zero-observed family activation,
  including concurrent roots without a mixed dependency row;
- exact child-owned MODULE batch text/order, parent eventlessness, warm
  suppression, real poll/drop cancellation and same-DICE recovery; and
- root MODULE, recursive include and visible lockfile create/edit/delete/
  recreate plus A/B/A restoration with held Result/epoch Arc lifetime.

Run focused bzlmod owner tests, full bzlmod, the accepted loading/query/core
baselines affected by RootModuleFiles, fmt, diff-check, exact cap accounting and
AI-cleanup/Buck2 retention review.

## STOP and sole successor

STOP on a third Rust file, public export/caller, selected graph, registry,
extension evaluation/instantiation, generated route/package, analysis,
toolchain/action, Stage 10, fixtures/oracles, Cargo/BUILD, direct Host reads,
event ownership change, another retained collection/state owner, cap excess,
M7A closure, M8/M7B/M9 or a second successor. REPLAN rather than weaken legacy
parity, skip lockfile Arc validation or synthesize extension usages.

After independent design ACCEPT, schedule exactly one
`WP-6-7A-root-module-files-observation-completion-implementation`. After
implementation ACCEPT, schedule only the docs-only
`WP-6-7A-selected-module-graph-observation-frontier-design`; do not activate
selected graph or external rules_rust in this packet.
