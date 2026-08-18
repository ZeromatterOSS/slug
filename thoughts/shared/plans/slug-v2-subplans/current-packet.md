# Current Slug V2 Packet

Packet: `WP-2A-m1-direct-local-evaluation-upper-source-owner-audit`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling/Rust base: `cc34e31d`
Result: audit only the first complete post-preparation observation owner before
any evaluation or upper source/load activation.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`

Against `cc34e31d`: at most 40 canonical net lines, 180 Stage 2 net lines,
200 manifest net lines, and 420 aggregate net lines.

## Required audit

Start from the live checkout and trace the complete direct-local chain after
the accepted `DirectLocalModulePreparationObservationKey` through:

- `DirectLocalModuleEvaluationKey`, its preparation/support inputs, local
  semantic result and Complete evaluation event batch;
- any direct-local support or recursive fragment/evaluation helper that owns a
  reusable semantic fact below evaluation;
- `RepositoryPackageSourceKey`, including support, external package lookup,
  BUILD source selection, and its source/preparation association;
- `ExternalBzlModuleEvalKey` and recursive external `.bzl` loading/event
  ownership; and
- `RepositoryPackageLoadKey`, its local Complete BUILD batch, package value,
  and every query/build consumer that remains legacy.

For every edge, identify the exact legacy and accepted observed key families,
Result Arc and `PathObservationEpoch` availability, Need/typed-outer/semantic
prefix, DICE equality/validity, event owner, cancellation behavior, retained
versus compute-local data, warm reuse, and edit/delete/recreate/A-B-A path.
Determine whether `DirectLocalModuleEvaluationKey` is the uniquely smallest
complete next owner, whether one smaller observed support/fragment/evaluation
producer is required, or whether the requested family isolation is impossible.
Do not assume an epoch may be discarded merely because a child DICE key owns
it, and do not combine two existing legacy/observed parent families.

Audit source analogues in pinned Bazel 9.2 only where ownership is ambiguous;
reuse accepted evidence and do not run or write a new oracle. Measure the live
future Rust files and freeze file-specific semantic and physical caps only
after the natural owner is proved.

## Terminal and proof contract

End in exactly one of:

1. one docs-only bounded design for the uniquely smallest complete owner;
2. one docs-only uniquely smaller prerequisite design that returns directly to
   this audit after its implementation; or
3. formal `REPLAN` with the conflicting ownership/family/event constraints.

Any selected design must freeze structural key identity, one matching-family
driver, exact Arc/epoch order and decisive prefixes, full joined-batch
outer/Need/semantic algebra, complete-only equality, child versus parent event
publication, cancellation/no-publication, compact retained lifetimes, legacy
and reverse-family isolation, zero unselected upper activation, lifecycle
proof, exact/Slug-native/deferred compatibility, future file/cap authority,
cleanup/retention review, and one independent design review. Implementation may
follow only an independently accepted design.

Exact compatibility remains MODULE/BUILD/`.bzl` bytes, parsing, semantic
values/errors, dependency order, legacy behavior and existing child events.
New sibling/carrier/typed-outer/retry mechanics are Slug-native. Public query/
build activation, broader repositories, evaluation/source/load publication,
and exact identity bytes stay unsupported/deferred until separately accepted.

## STOP / REPLAN

STOP on every Rust, Cargo/BUILD, fixture, oracle, generated artifact or public
behavior write; any implementation or design conclusion before the audit is
complete; a second active successor; widened query/build identity; or M1
closure. `REPLAN` if no bounded single-family owner exists, another retained
store/collection/event owner is required, exact Arcs cannot survive, legacy
events/semantics must change, or the future bounded file/cap envelope cannot
hold.
