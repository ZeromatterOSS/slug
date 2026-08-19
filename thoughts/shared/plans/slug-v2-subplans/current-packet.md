# Current Slug V2 Packet

Packet: `WP-6-7A-repository-source-file-observation-proof-cap-correction-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `178dec27`
Rust base: `ae8aa35e`
Accepted semantic design: `9040e168`

## Formal REPLAN evidence

The retained two-file implementation candidate is semantically sound and has
exact scope/diff hygiene. Against `ae8aa35e`,
`source_preparation.rs` is +298 net at 15,238 physical lines and
`source_preparation_observation_tests.rs` is +500 net at 2,970 physical
lines: +798 semantic and 18,208 physical aggregate. The shared driver is 179
lines, selects the matching families, merges the materialization prefix
left-first before resolution semantics, appends FileBytes before inspection,
and retains only the local Result Arc plus compact epoch. Focused proof is 3/3;
full bzlmod is 439/439; loading is 138/138; query is 53/53; core remains the
inherited 245/246 stale visibility wording baseline.

Independent review found proof-only gaps. The exact +500 proof ceiling leaves
no room to discriminate the production-used materialization/resolution
reducers for Need/typed outer, compute errors, duplicate/conflict/operation
mismatch and reached prefixes. Current family rows do not include the neutral
FileBytes dependency; event tracking starts after the event-owning child and
cannot prove its cold batch or warm suppression. Cancellation recovers through
a different request, local restoration lacks an explicit equality assertion,
and immutable edit/delete/directory/recreate A-B-A plus restored held Arcs is
absent. Forcing these cases into the old ceiling would delete required
behavioral proof or obscure it. Production semantics, ownership, retention,
events and family selection do not require redesign.

## Exact docs authority and caps

During this design write only:

1. canonical plan, <=40 net lines;
2. this manifest, <=200 net lines;
3. `06-analysis-toolchains-and-actions.md`, <=140 net lines; and
4. routing log, <=30 net lines.

Aggregate docs growth is <=410 net lines. Retain the two dirty Rust candidate
files exactly and treat them as non-writable. Every other file is read-only.

## Frozen correction

Preserve the accepted structural key/carrier, Legacy/Observed driver,
materialization -> resolution -> FileBytes order, materialization-first
left-biased epoch algebra, carrierless Need/outer, exact legacy projection,
eventless parent/path owners, matching-family isolation and compact one-Result-
Arc-plus-epoch retention. Add no production owner, caller, export, state,
event, Host read, cache, store, interner, lock or task.

Correct only the proof envelope for the immediate implementation retry:

- keep `source_preparation.rs` at <=300 production, <=30 colocated proof and
  <=15,320 physical from 14,940;
- raise `source_preparation_observation_tests.rs` from <=500 tests/3,020
  physical to <=700 tests/3,250 physical from 2,470;
- raise aggregate semantic growth from <=830 to <=1,030 and combined physical
  size from <=18,340 to <=18,570.

This adds at most 200 test-semantic and 230 proof physical lines. It may fund
only compact restructuring/addition of the missing discriminators. The retry
may not change production semantics, owner, event behavior, retained shape,
family selection or downstream activation.
Line-neutral extraction of pure, production-called terminal projectors is
allowed only where needed to make the existing live branches discriminating.

## Required retry proof

Use production-called seams or real keyed outcomes to prove:

- distinct key identity/hash/Display, accessors and carrier/typed-outer
  validity/equality;
- invalid materialized path and materialization/resolution/FileBytes DICE
  compute failures through production-called projectors with exact empty/prior
  prefixes;
- materialization and resolution Need/typed outer validity/equality,
  carrierlessness, later suppression, compute-error prefixes and semantic
  prefixes;
- materialization-prefix-first iteration order and duplicate Arc retention,
  plus conflicting-value and operation-mismatch outer polarity through the
  same merge/append seams used by production;
- FileBytes Need/compute/Complete append behavior with exact prior/full prefix
  order and carrierlessness;
- exact observed and legacy direct-dependency rows, including matching
  materialization/resolution families and neutral `PathObservationKey`;
- a phase-separated cold materialization/root child batch, parent/path/
  FileBytes silence, warm suppression and no batch on Need/outer/cancel;
- real poll-drop followed by same-DICE, identical-request recovery; and
- local and immutable edit/delete/directory/recreate A-B-A with A==restored,
  held Result/bytes/epoch equality and restored per-demand Arc checks.

Retain the existing invalid-path, source terminal, exact epoch/bytes and legacy
parity proof. Reuse accepted lower-key tests without claiming they alone prove
the new parent's branch or prefix decisions. Keep explicit nonactivation checks
for package preflight, REPO-file, repository-ignore, module preparation,
closure, discovery, selected graph, registry and public callers.

## Compatibility and STOP

Exact behavior remains relative-path validation, materialization/source order,
Host versus Materialization namespace, symlink/path/FileBytes semantics,
values/errors/nested bytes Arc and all legacy behavior. The sibling, local
Result Arc, compact epoch and typed outer remain Slug-native. Registry/
preparation/closure/discovery/selected graph, extensions/generated
repositories, rules_rust actions, M8/M7B and exact identity bytes remain
deferred.

STOP Rust, Cargo, BUILD, fixture, oracle and public writes during design.
STOP any production semantic/event/memory/family change, third retry file,
caller/export, upper/registry activation, proof deletion or cap excess.
REPLAN again if the complete proof cannot fit the corrected envelope.

After independent design ACCEPT, schedule exactly
`WP-6-7A-repository-source-file-observation-implementation-retry` with the
same two Rust files. Only after implementation ACCEPT return to
`WP-6-7A-selected-module-graph-observation-frontier-design`.
