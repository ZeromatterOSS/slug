# Current Slug V2 Packet

Packet: `WP-2A-m1-multi-build-analysis-error-acceptance-correction-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling and retained-candidate base: `7d027088`
Accepted semantic design: `a2d440cb`
Accepted Rust base: `3f1d4dd4`
Result: formally REPLAN the observed multi-build implementation at its native
semantic-analysis-error acceptance boundary. Retain the dirty two-file Rust
candidate, but make it non-writable during this docs-only design.

## Exact docs authority and caps

Write exactly:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`: <=40 net.
2. this manifest: <=220 net.
3. `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`: <=160 net.
4. `.codex/skills/slug-agent-orchestration/references/routing-log.md`: <=30 net.

Aggregate docs net <=450. STOP every Rust, Cargo, BUILD, fixture, oracle,
generated artifact and public/caller edit. The retained `runtime/dice.rs` and
`runtime/tests/build_command_tests.rs` candidate is evidence only.

## Formal REPLAN evidence

The retained candidate proves the natural owner and success association are
sound: a public three-branch build with two exported sources and a recursively
analyzed rule reaches only the observed family, produces the ordered semantic
result/events, retains an exact aggregate certificate, and accepts a local
terminal epoch as a pointer-identical subset of the larger selected dependency
epoch.

One frozen terminal is impossible under `a2d440cb`. Configured analysis
semantic errors are transient DICE values. The multi root therefore also
becomes unavailable for activation-closure selection. Keeping selection strict
fails with `UnavailableRoot`; applying the existing exact legacy behavior and
dropping that unavailable error root yields an empty selected demand set, after
which `SelectedDependencySuperset` rejects the nonempty local
anchor/package/source epoch as `Demand`. Making analysis errors DICE-valid,
discarding the local epoch/certificate, weakening exact Arc validation or
duplicating the analysis observation graph would all be unsound.

The retained proof also exposed two bounded implementation corrections, not
new owners: per-branch source certificates must be taken out of semantic
targets/errors once their Arcs enter the aggregate certificate, and the public
mixed-build proof needs an invocation-exclusive precreated stable parent so a
parallel sibling cannot mutate an observed ancestor.

## Frozen correction owner and algebra

Native terminal acceptance remains the uniquely smallest owner. Add one private
terminal-demand association, defaulting to existing closure-only selection.
Only `BuildCommandRootObservationKey` with more than one target and an exact
semantic `BuildCommandErrorKind::Analysis` terminal may select
`TransientTerminalLocal`.

For that one case:

- preserve the existing unavailable-root seal/drop behavior and exact legacy
  analysis-error event semantics;
- before `selected_snapshot`, extend only the selected unscoped path demands
  with every demand from the terminal's already-associated local epoch,
  deterministically sorted and deduplicated;
- source every selected value/Arc from the command epoch after terminal-first
  association, so equal values retain the terminal Arc and conflicts have
  already failed closed;
- keep repository requests/validations strictly empty and reject any attempt to
  use this policy for a non-analysis terminal, singleton, external, query,
  cquery, neutral, legacy/direct or one-shot root;
- validate exact demand/value/Arc identity for the entire local epoch and exact
  certificate-subset identity; and
- retain no policy, selected set, map or second epoch after acceptance.

The accepted `SelectedDependencySuperset` success policy stays unchanged:
terminal-only demands still fail and configured-analysis/action-closure
remainder remains activation-closure owned. The new local selection is sound
only for the semantic analysis terminal whose exact lower prefix is already
owned by the terminal while the transient upper root cannot supply a closure.
Need/typed outer/cancel, selection, revision, materializer or publication
failure remains atomic and leaves prior path/repository/event state unchanged.

Add the smallest crate-private helper in `runtime/demands.rs` to return a new
`SelectedWorkspaceDemands` with additional unscoped terminal paths while
preserving repository fields. Use compute-local sorting/deduplication; retain no
map/cache/interner/store/lock/task and perform no Host read.

During the retry, take each Complete branch certificate out of its
`BuildRequestedTarget` or source-bearing error when building the aggregate.
The observed multi semantic Result must therefore match legacy multi semantics
and retain no per-branch certificate; the one aggregate certificate beside the
Result Arc remains the sole certificate carrier and shares every exact Arc
with the local epoch.

## Retry authority, compatibility and proof

After independent design acceptance, schedule exactly
`WP-2A-m1-multi-build-observed-publication-implementation-retry` with:

1. `app/slug_core_v2/src/runtime/dice.rs`: <=410 production plus <=40
   colocated tests; <=11,700 physical.
2. `app/slug_core_v2/src/runtime/demands.rs`: <=20 production; <=1,230
   physical.
3. `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`: <=500 tests;
   <=3,900 physical.

Aggregate semantic <=970 and combined physical <=16,830. Preserve every
accepted `a2d440cb` identity, full-batch, epoch, revision, event, memory and
compatibility contract except the explicit analysis-error selection correction
above.

Required new discrimination:

- a real source-plus-rule analysis error accepts the exact local prefix and
  aggregate certificate, selects no repository sidecar, publishes exact legacy
  semantic/error event behavior and recovers after the rule fix;
- success with recursive analysis still has a strict selected remainder, while
  a terminal-only demand remains rejected;
- default closure-only roots reject the same synthetic local injection;
- multi public targets/errors retain no branch certificate, the internal
  aggregate retains both source epochs with exact `Arc::ptr_eq`, and direct
  legacy projection remains semantically exact;
- an invocation-exclusive parent is created before either retained runtime,
  followed by warm/edit/restore and default-parallel validation; and
- cap, `Allocative`/`Dupe`, Buck2 retention, AI cleanup and rollback scans
  remain clean.

Exact: public/legacy semantic errors, target values/order, child events and all
previously accepted roots. Slug-native: the private transient-terminal local
demand association and aggregate-only certificate carrier. Unsupported/
deferred: mixed/external multi, recursive patterns, one-shot migration, broader
actions/globs and exact Bazel identity bytes.

STOP/REPLAN on any fourth Rust file, wider unavailable-root behavior, retained
selected state, repository sidecar, terminal-only success admission, child
event/equality change, cap excess, partial validation or M1 closure. This design
may schedule only the named implementation retry.
