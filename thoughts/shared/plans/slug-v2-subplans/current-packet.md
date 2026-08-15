# Current Slug V2 Packet

Packet: `WP-2A-m1-host-bzl-module-frontier-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: add one callerless loading-private observed Host `.bzl` module key
whose complete success and semantic-error values retain the exact recursive
source frontier, without activating glob/package loading, core publication, or
public overlap.

## Accepted predecessor and learned facts

Commit `2225cf99` supplies the doc-hidden Bzlmod
`RootPackageSourceObservationKey` and exact source carrier. The live
`HostBzlModuleEvalKey` then performs source conversion, parse/load-label
resolution, sequential recursive child evaluation, Starlark evaluation,
freeze, and one local completed event batch.

The evented `HostBzlLoadCycleGuard` is typed only around the legacy key.
`notify_cycle` wakes every strongly connected `cycle.keys` member
simultaneously, so a cycle member cannot wait for a child carrier to obtain the
other cycle sources. `cycle.path` is non-cycle ancestry and must not be added
to a child key's retained value; its actual parent already owns and composes
that source. The invalid `BzlLoadCyclePoisonKey` forces cycle recomputation and
prevents a context-specific cycle result from becoming a reusable warm value.

Relevant unchanged Bazel 9.2 behavior remains source-anchored to
`BzlLoadFunction`, `BzlLoadFunctionTest`, `BzlLoadCycleReporter`, and
`AbstractLabelCycleReporter`. Existing accepted Slug tests remain the
regression evidence. No new oracle is needed because the sibling is callerless
and the legacy behavior is not changed.

## Frozen implementation contract

1. In `bzl_module.rs`, add loading-private
   `ObservedHostBzlModule { result:
   Arc<Result<FrozenBzlModule, HostBzlModuleError>>, observations:
   PathObservationEpoch }`, deriving `Debug, Clone, PartialEq, Eq, Allocative,
   Dupe`, with only borrowed crate-private accessors.
2. Add loading-private `HostBzlModuleObservationKey { workspace, label }`
   with the legacy structural identity, normal key derives, a distinct
   `observed-host-bzl-module` Display, and Value
   `SourcePreparationOutcome<Result<ObservedHostBzlModule,
   ObservedPathFrontierError>>`. Equality is `complete_eq`; validity is
   `is_complete`.
3. Replace the legacy Host module orchestration with one private mode-aware
   driver serving legacy and observed wrappers. Mode-selecting source and child
   helpers compute exactly one legacy or observed family. Neither module key
   computes the other, and the external/older module families remain untouched.
4. Compute source first. Observed mode consumes only
   `RootPackageSourceObservationKey::for_bzl`, moves its exact epoch into
   scratch, and preserves its semantic Result Arc/bytes. Source Need forwards
   unchanged. Source outer error completes outer with no carrier/event. Source
   semantic error completes inside `HostBzlModuleError::Source` with the exact
   source epoch.
5. Preserve source conversion, parse and load-label order. Input, parse, and
   load-label semantic errors retain the current source epoch. Extract and
   visit direct loads in existing source order.
6. For each ordinary observed child, compute only
   `HostBzlModuleObservationKey` under the generalized same-family guard.
   Forward Need/outer error before parent completion. Union the child's exact
   epoch before interpreting its semantic result; then accept the module or
   wrap the unchanged `Child` error. Later child cache state is excluded from
   an earlier decisive terminal.
7. Generalize the one Host cycle guard in `cycle_detector.rs` to retain
   distinct Legacy and Observed family tags over one shared private
   `HostBzlCycleIdentity { workspace, label }`. Start, edge, finish, and
   detected-cycle conversion must reject mixed families. Never canonicalize the
   two DICE families to one detector node, and do not add a second detector,
   graph, lock, task, map, or event channel.
8. Preserve the current single sequential child wait and async receiver mutex.
   It may enclose only the existing one outstanding `guard_this` child
   compute/race per guard and must never be reacquired by that guard. It must
   not enclose source computation, cycle-member source reacquisition, Starlark
   evaluation, event publication, or any other DICE compute.
9. On an observed detected cycle, compute the invalid
   `BzlLoadCyclePoisonKey` exactly as legacy does. Rotate only
   `cycle.keys` so the current identity is first; do not include
   `cycle.path`. For every remaining cycle member compute its accepted
   `RootPackageSourceObservationKey::for_bzl` directly, recording real DICE
   dependencies and unioning exact source epochs. This is source-key reuse, not
   reconstructed Host observation.
10. In a stable detected cycle, every member source already completed before
    its outgoing edge was registered. Nevertheless forward source Need and
    outer error with no carrier/event; a now-semantic source error becomes the
    exact current source terminal. Union conflict/mismatch remains completed
    outer. Only after all member epochs are accepted return the unchanged
    semantic `Cycle` error and one empty completed event batch.
11. Preserve evaluation and freeze order after all children. Success,
    evaluation error, and freeze error retain the current source plus every
    accepted child epoch. Reuse the exact existing semantic module/result Arcs
    and exact observation-result Arcs.
12. Each selected key activation owns exactly one equivalent local event batch
    only for semantic completion. Legacy and observed wrappers each publish
    only their own batch. Need, cancellation, and outer frontier errors publish
    no parent batch or carrier; completed child DICE state remains
    dependency-owned.
13. The final observed value retains only one semantic Result Arc and the
    existing Arc-backed `PathObservationEpoch`. Source text, AST, loads,
    loaded modules, evaluator, loader, globals, union vectors, cycle guard,
    event batch, transaction, and child carriers remain compute-local scratch.
    The detector retains only its existing request-local tagged identity graph,
    never an epoch or semantic carrier.
14. Do not export the observed key/carrier from `lib.rs`. The next
    package-loading producer is in the same crate. Do not change any legacy key,
    public Value/error/API, consumer, output, or event behavior.

For `A -> B -> C -> B`, the required cycle frontier is:

- B: `S(B) + S(C)`;
- C: `S(C) + S(B)`; and
- A's later child error: `S(A) + B`, therefore `S(A) + S(B) + S(C)`.

Including `S(A)` directly in B/C is forbidden because it makes their DICE
value depend on the caller that first reached the cycle.

## Compatibility boundary

Preserve admitted serial Host `.bzl` source, load-label, child, cycle,
evaluation, freeze, diagnostic, event and recovery behavior exactly. The
callerless frontier association, exact-Arc aggregation and family-tagged cycle
identity are Slug-native. External modules, Host glob/directory unions, final
package-load aggregation, loading consumption, core request revision, public
overlap, repository/materializer work and exact Bazel identity bytes remain
unsupported/deferred.

## Proof and validation

Add focused colocated proof for:

- direct and nested observed success with exact source/child observation Arcs;
- source semantic, input, parse, load-label, child, evaluation and freeze
  terminals retaining only the decisive prefix;
- source/child Need, forced outer conflict/mismatch, cancellation source proof,
  complete-only equality/validity, and no parent event/carrier;
- direct self-cycle and `A -> B -> C -> B`, proving each notified member
  reacquires only rotated `cycle.keys`, the top parent retains all three
  sources, the poison dependency remains invalid, and `cycle.path` is not
  retained by B/C;
- simultaneous legacy/observed evaluation of one label without detector-node
  conflation or cross-family edges;
- exactly one local event batch per completed observed activation, zero legacy
  source/module activation, and unchanged legacy event/cycle tests; and
- warm reuse, source mutation and A/B/A restoration for non-cycle terminals.

Run focused observed-module/cycle tests, unchanged legacy Host module/cycle
tests, full `slug_loading_v2`, direct `slug_core_v2` check,
`cargo fmt --all -- --check`, strict Clippy with inherited-baseline
disposition, `scripts/v2_archive_status.sh`, artifact scan and
`git diff --check`. No Bazel oracle is required. Do not run Cargo commands
concurrently in one target directory.

Because `bzl_module.rs` has 3,852 production and 5,092 physical lines, require
independent pre/post cohesion and nine-category cleanup review. The shared
mode-aware driver must replace, not duplicate, the legacy orchestration.

## Authority and caps

Write exactly:

- `app/slug_loading_v2/src/bzl_module.rs`;
- `app/slug_loading_v2/src/cycle_detector.rs`; and
- at completion only, canonical/current/Stage 2.

Caps from the live 5,092/552-line baselines are:

- `bzl_module.rs`: 330 production, 400 in-module tests, 730 total net and
  5,822 physical lines;
- `cycle_detector.rs`: 115 production, zero tests, 115 total net and 667
  physical lines; and
- aggregate: 445 production, 400 tests and 845 total net Rust lines.

Completion ledgers are capped at 180 net lines. No correction is authorized.
Read-only authority is the design packet's prior sources plus the exact Bazel
9.2 classes named above and directly referenced focused tests.

## STOP / REPLAN

STOP on every other Rust file; Cargo/oracle/generated writes; a public export,
consumer or behavior change; a second module/cycle key, carrier, detector,
graph, channel, lock, task, container or event owner; key-to-key module compute;
mixed-family detector nodes; `cycle.path` source retention in a cycle member;
detector-held epochs; direct/reconstructed/historical Host reads; partial
cycle/member prefixes; outer-error laundering; duplicate evaluator/driver;
glob/package-load/core/repository/materializer/watcher/JVM work; or cap/ceiling
excess.

REPLAN if family distinction cannot coexist in the one guard; a cycle member
cannot record direct accepted source-key dependencies for every `cycle.keys`
member; poison invalidity cannot prevent context-stale reuse; semantic/outer/
Need/event ownership cannot remain distinct; the legacy driver or cycle
diagnostic changes; a third Rust file is required; cleanup finds a concrete
split boundary; or any correction is needed.

## Immediate successor

On acceptance schedule only docs-only
`WP-2A-m1-host-glob-frontier-design`. Do not combine final package-load
aggregation, loading consumption, core publication, public overlap, or another
consumer.
