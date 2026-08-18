# Current Slug V2 Packet

Packet: `WP-2A-m1-direct-local-evaluation-observation-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Design/scheduling base: `bfd4f1f6`
Rust base: `cc34e31d`
Result: implement only the accepted private observed direct-local evaluation
sibling and eventless support projection seam.

## Authority and caps

Write only:

- `app/slug_bzlmod_v2/src/source_preparation.rs`
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`

Against `cc34e31d`: at most 240 source net lines and 13,800 physical lines;
380 proof net lines and 2,500 physical lines; 620 aggregate semantic lines and
16,300 combined physical. Keep touched helpers below 200 lines.

## Required implementation

Keep `DirectLocalModuleEvaluationKey` and its Value exact. Add one private
structural `DirectLocalModuleEvaluationObservationKey` and carrier containing
only the local evaluation Result Arc plus the accepted preparation epoch. One
Legacy/Observed evaluation driver selects only matching preparation families
and projects the exact local Arc to legacy. Evaluation adds no path edge:
PreparationCompute is empty; preparation semantic, Unsupported, RootAbsent,
evaluation error and success keep the full unchanged preparation epoch;
preparation Need/typed outer has no carrier. Need is invalid/self-unequal;
Complete outer compares by outer value; Complete carrier compares semantic
Result+epoch.

The shared driver remains the sole local evaluation-event authority. Store the
exact matching-family local batch only for semantic Complete carriers,
including legacy-equivalent empty batches. Need, typed outer and cancellation
publish none. Child events remain child-owned and precede the evaluation batch.

Refactor `direct_local_module_support` through one crate-private, eventless
mode-aware support projection. Legacy behavior and exact returned support Arc
remain unchanged. The callerless observed branch forwards evaluation Need/
outer and associates projected Supported/Unsupported/error semantics with the
evaluation epoch for the later package-source owner. Add no key, retained
evaluation Arc, event or caller; do not edit or activate `host_package.rs`.

Retain only the existing evaluated semantic graph inside one Result Arc plus
one compact epoch. Included-file vectors, evaluator/module scratch, event
staging and support projection stay compute-local. Add no state/store/cache,
collection/interner, lock/task, Host read, revision/certificate, export or
second event owner.

## Compatibility and proof

Exact: evaluation values/errors, Starlark semantics, include order, legacy
result/Arc behavior and child/evaluation events. Slug-native: sibling/carrier,
typed outer, retry epoch and observed support association. Deferred: observed
package source, recursive external `.bzl`, package load/query/build
publication, broader identities and exact identity bytes.

Prove identity/Display; exact legacy Arc/result/event parity; observed semantic
parity; exact preparation epoch membership/order and every Result Arc with no
new demands; every empty/full prefix and Need/outer validity/no-publication;
print/error and child-before-parent event order/warm suppression; both family
directions and zero support/source/ExternalBzl/load/query activation; real
poll-drop cancellation/recovery; edit/delete/recreate and A/B/A; eventless
support Supported/Unsupported/error projection; compact Allocative retention,
Buck2/AI cleanup, focused/full bzlmod/loading/query and established core
baselines, fmt/check/diff/accounting, Clippy/archive disposition, and one
independent latest-diff review.

## STOP / REPLAN

STOP on every other file, Cargo/BUILD/fixture/oracle write, a caller/export,
`host_package.rs`, mixed families, rebuilt/partial epoch, moved/duplicate
events, parent batch on Need/outer/cancel, retained scratch/state, upper
activation, public behavior, cap excess, multiple successors or M1 closure.
`REPLAN` if evaluation adds a path edge, exact preparation Arcs cannot survive,
the support seam needs retained state/API/event ownership, another file/owner
is required, or legacy result/event semantics must change. After ACCEPT,
return only to a docs-only upper-source owner audit.
