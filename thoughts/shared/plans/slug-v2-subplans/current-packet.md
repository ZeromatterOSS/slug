# Current Slug V2 Packet

Packet: `WP-2A-m1-host-bzl-module-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: design the first complete recursive Host `.bzl` module frontier over
the accepted root-package source carrier, without implementing code or
activating package loading, glob evaluation, core publication, or public
overlap.

## Accepted predecessor

Commit `2225cf99` adds the callerless doc-hidden/public
`ObservedRootPackageSource` and `RootPackageSourceObservationKey`. One
mode-aware driver preserves legacy BUILD and deepest-to-declared `.bzl` source
selection while the observed sibling retains the exact decisive package-lookup
and Host-file observation prefix. It owns no events and remains unconsumed by
loading.

The implementation is accepted at 234 production plus 342 in-module test
lines, 576 total net Rust lines. `host_package.rs` reaches 4,567 physical lines
and `lib.rs` reaches its 387-line ceiling. Focused observed-source and legacy
projection suites pass 3/3 each; all 588 Bzlmod tests pass; direct loading/core
checks and formatting pass. Strict Clippy stops first in unchanged
`allocative_derive`, and archive checks reproduce only inherited baselines.
The Windows platform-path branch was source-checked but not executed because
the installed toolchain exposes only `x86_64-unknown-linux-gnu`.

Existing source selection, bytes, diagnostics, Need/error order and admitted
Host observations remain exact. The callerless frontier association/equality
is Slug-native. Recursive `.bzl`, glob, package-loading, core finalization and
public overlap remain unsupported/deferred.

## Design questions

1. Map the complete live `HostBzlModuleEvalKey` dependency, parse/load-label,
   recursive child, cycle-detection, evaluation, freeze and event order. Name
   every success, completed-error, Need and cancellation boundary before
   selecting a carrier.
2. Decide whether one loading-private observed sibling can consume
   `RootPackageSourceObservationKey` and recursively consume only its own
   observed family without computing or changing the legacy module key. If
   not, select exactly one smaller docs prerequisite.
3. Resolve the current `HostBzlLoadCycleGuard` ownership: it is typed around
   the legacy module key. Freeze a bounded shared/private cycle identity and
   diagnostic path that preserves legacy cycle/error order without a second
   graph, global seen set, public ABI, or key-to-key compute.
4. Freeze source-order decisive-prefix composition. Merge the current module
   source epoch before semantic interpretation, then merge recursive child
   epochs only as their terminals become decisive. Exclude later joined child
   cache state from an earlier terminal.
5. Keep semantic errors inside the legacy-equivalent semantic carrier and
   observation aggregation/conflict failures as completed outer
   `ObservedPathFrontierError`s. Need, cancellation and nonterminal attempts
   publish no parent carrier or parent completed event batch; completed child
   DICE state remains dependency-owned cache state.
6. Preserve exactly one equivalent event batch per selected module-key
   activation. Freeze whether a shared semantic evaluation/finalization leaf
   can serve legacy and observed wrappers without duplicating evaluation or
   making either key compute the other.
7. Bound retained memory to one semantic Result Arc plus the accepted
   Arc-backed epoch. Retain no evaluator, AST, source collection, transaction,
   cycle guard, event batch, child carrier, historical read, or second compact
   collection.
8. Freeze complete equality/validity, exact-Arc clone boundaries, A/B/A and
   warm behavior, cancellation release, outer-error polarity, focused proof,
   exact Rust file allowlist, per-file production/test/total caps, physical
   ceilings and mandatory cleanup review.
9. Identify the next missing producer after this frontier. Host glob and final
   `RootPackageLoadKey` aggregation remain separate packets and must not be
   combined here.

## Compatibility boundary

Preserve admitted serial `.bzl` source, label/load, cycle/error, evaluation,
event and recovery behavior exactly. Reuse existing pinned/source evidence;
do not claim new Bazel parity. Frontier aggregation, certificate identity and
final-validation preparation are Slug-native. Glob/directory unions, package
loading consumption, core request revision, public overlap, repository/
materializer work and exact Bazel identity bytes remain deferred.

## Authority and caps

This packet is docs-only. Write exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only the three ledgers; `docs/developers/dice.md`; the Bzlmod, loading and
workspace Cargo manifests; Bzlmod `src/{lib,host_package}.rs`; loading
`src/{lib,keys,bzl_module,cycle_detector,load_label}.rs`; workspace
`src/{lib,path_observation}.rs`; directly referenced focused tests; the
`slug-buck2-utility-reuse` skill and matching Stages-3/6 Stage 9 row; and only
`gazebo/dupe/src/lib.rs` plus `allocative/allocative/src/lib.rs` for retained
clone/memory rules.

Ledger caps are 40 canonical, 340 current, 300 Stage 2 and 680 total net lines.
No correction is authorized.

## STOP / REPLAN

STOP on Rust, Cargo, oracle or generated-file writes; a public user API/wire/
output change; loading/package-load/core activation; glob/directory,
repository/materializer, watcher or JVM work; a reverse dependency; a generic
certificate framework; a new retained container, graph or store; duplicated
evaluation/event authority; direct/reconstructed/historical Host reads;
partial recursive frontiers; outer-error laundering; combined consumers; or
cap excess.

REPLAN if recursive completion cannot remain finite under the existing cycle
semantics; the typed legacy cycle guard cannot serve both families without a
behavior/API change; completed errors cannot retain their exact decisive
prefix; Need/outer/semantic/event ownership cannot remain distinct; exact
observations require reconstruction; or no independently bounded first
producer exists.

## Immediate successor

Acceptance may schedule exactly one bounded Host `.bzl` frontier
implementation or one uniquely required docs-only prerequisite. It must not
combine Host glob, final package-load aggregation, loading consumption, core
publication, or public overlap.
