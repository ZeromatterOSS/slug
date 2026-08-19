# Current Slug V2 Packet

Packet: `WP-2A-m1-external-singleton-observed-build-implementation-retry-6`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling/design base: `503af0a9`
Accepted Rust base: `a4dd40d6`
Accepted external-build design: `1a217e2a`
Accepted loading-proof correction: `ce110d9a`
Accepted revision-event design: `5dabd4bf`
Accepted source-certified event policy: `0b4b5210`
Accepted terminal-epoch correction: `503af0a9`
Result: accept the observed external singleton build with exact terminal Arc
authority and unchanged public/event semantics.

## Exact Rust authority and caps

Write exactly `runtime/dice.rs` <=380 semantic/11,400 physical,
`runtime/tests/build_command_tests.rs` <=440 tests/3,450,
`slug_loading_v2/src/host_package_load_tests.rs` only the accepted line-neutral
assertion at 3,439, and `runtime/events.rs` <=100 production plus <=160 tests/
2,050. Aggregate <=1,080 semantic and <=20,339 physical against `a4dd40d6`.
No other file/loading byte, docs, Cargo, BUILD, fixture, oracle, caller or server
test. Remove every temporary trace; touched helpers stay below 200 lines.

## Complete implementation contract

Preserve structural nonroot-Single observed admission, exact matching-family
legacy/observed driver, anchor -> route -> package -> ExportedFile
classification -> revision -> source order, union-before-semantic prefixes,
exact source-child certificate, external-only repository selection, selected
value/Arc validation, child-only event ownership, exact legacy infrastructure
projection, compact retention and every accepted failure polarity.

Preserve private terminal-dependent
`EventReconciliationPolicy::{Strict, SourceCertifiedCurrentClosure}`. Default
every root/terminal to Strict. Only an observed external singleton terminal
retaining a SourceCertificate opts in. Apply matching-root current-closure
reconciliation to normal acceptance and actual retries: current Some including
empty wins; present-prior KnownNone/NoTransition carries; absent prior drops;
new Some contributes in current order. Preserve provisional tombstones,
revision->Need, multiple retries, true-prior delta and post-materializer atomic
replacement. Retain no policy/closure/map and do not change global KnownNone.

For every Complete root exposing `observations()`, before sealing/selection,
rebuild the command epoch through stable shared construction with terminal
entries first and current command entries second. Terminal-only demands install
the terminal exact Arc; equal duplicates preserve that Arc; unrelated command
demands/Arcs survive; differing values, operation conflicts or invalid epochs
fail closed before selection, finalization, materializer acceptance or
publication. Need/outer and terminal-less roots do not merge.

The reconciled command-local epoch is the sole input to ordinary selected
demand filtering. Selected closure demands still exclusively control
membership. Repository selection/validation and unconditional terminal
length/demand/value/`Arc::ptr_eq` validation remain unchanged. A terminal demand
outside the selected closure still fails the existing membership/length proof.
Any merge/selection/revision/repository/materializer/cancel/abort failure leaves
the prior accepted path/repository/event snapshots untouched.

Retain only the build Result, full selected path epoch, certificate epoch, and
compact Dupe/Allocative accepted/provisional event root/entry slices. Merge
input, children, maps/Vecs and repository/event scratch stay compute-local or
dependency-owned. Add no Host read, side store, collection, cache, interner,
lock, task, child carrier, event owner or accepted state.

## Proof, compatibility and STOP

Retain the complete external family/prefix/Arc/certificate/repository/event/
lifecycle/cancellation/rollback proof and line-neutral loading assertion. Add
terminal-only install; equal fresh command Arc replacement by terminal Arc;
unrelated Arc preservation; conflicting value failure before publication with
prior snapshots intact; strict selected membership; external success -> root
PackageAll -> external wrong-kind and success with exact terminal Arcs;
observed query/build root switches; warm/cancel/event parity. Preserve Strict
event removal, source-policy carry, direct edit without RevisionRetry, actual
retry and child removal/change/current-order proof.

Exact: public values/errors, selected paths/repositories and child events.
Slug-native: observed terminal/command epoch association, certificate,
repository and event retry association. Unsupported/deferred: multi-build,
one-shot, broader actions/external globs and exact identity bytes.

Run focused event/build/server lifecycle, 33/33 build, loading 138/138, full
bzlmod, documented core/query/server baselines, fmt/diff, exact accounting,
Buck2 retention, AI cleanup and independent final review.

STOP every other file/loading byte, global event change, terminal merge on
Need/outer, selected-membership or pointer-validation weakening, stale-value
preference, child filtering, another owner/state/API/Host read, retained
scratch, behavior/family drift, cap excess, broader activation or M1 closure.
REPLAN on any new blocker. After ACCEPT return only to one docs-only M1 owner
audit.
