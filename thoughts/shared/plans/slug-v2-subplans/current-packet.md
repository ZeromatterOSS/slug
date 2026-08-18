# Current Slug V2 Packet

Packet: `WP-2A-m1-direct-local-evaluation-observation-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `8f8df3d8`
Rust base: `cc34e31d`
Result: design only the private observed direct-local MODULE evaluation sibling
and its eventless matching-family support projection seam.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`

Against `8f8df3d8`: at most 40 canonical net lines, 120 Stage 2 net lines,
200 manifest net lines, and 300 aggregate net lines. Rust, Cargo/BUILD,
fixtures, oracles, generated artifacts and implementation are forbidden.

## Accepted owner and future scope

`DirectLocalModuleEvaluationKey` is the uniquely smallest complete owner. It
computes exactly one direct-local preparation, performs only pure in-memory
closure evaluation afterward, retains the local evaluation Result Arc, and
owns the one local Complete evaluation batch. The support helper is not a DICE
key, store or event owner; `RepositoryPackageSourceKey` adds independent
package-lookup and BUILD-source families, recursive `ExternalBzlModuleEvalKey`
adds its own source/evaluation family, and `RepositoryPackageLoadKey` crosses
both. No smaller prerequisite and no REPLAN is required.

Future Rust is exactly:

- `app/slug_bzlmod_v2/src/source_preparation.rs`
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`

Against `cc34e31d`: at most 240 source net lines and 13,800 physical lines;
380 proof net lines and 2,500 physical lines; 620 aggregate semantic lines and
16,300 combined physical. Retain the cohesive source owner and existing proof
file; keep touched helpers below 200 lines.

## Frozen design

Keep `DirectLocalModuleEvaluationKey` and its Value exact. Add one private
structural `DirectLocalModuleEvaluationObservationKey` and one carrier with
exactly the local
`Arc<Result<DirectLocalModuleEvaluation, DirectLocalModuleEvaluationError>>`
plus the complete preparation `PathObservationEpoch`. Use one Legacy/Observed
evaluation driver. Legacy selects only `DirectLocalModulePreparationKey`;
observed selects only `DirectLocalModulePreparationObservationKey`. Project
the exact local evaluation Arc to legacy; neither key computes the sibling.

The sole child order is preparation followed by root-presence and pure closure
evaluation. Accept the observed preparation epoch before semantic inspection
and forward it unchanged because evaluation adds no path observation.
PreparationCompute from a DICE compute failure has an empty prefix.
Preparation semantic error, Unsupported, RootAbsent, evaluation error and
success retain the full preparation prefix. Preparation Need and typed outer
return immediately without carrier. Need is invalid/self-unequal; a Complete
typed outer is valid/equal by the outer value; a Complete carrier is valid/
equal by semantic Result plus epoch. There is no joined-batch union at this
owner.

The shared driver is the sole local evaluation-event authority. Each sibling
stores the same matching-family local batch only for semantic Complete
carriers, including the legacy empty-batch cases. Preparation Need, typed
outer and cancellation store no parent batch. Preparation/path children keep
their own batches, and child events precede the local evaluation batch.

Refactor `direct_local_module_support` through one crate-private, eventless
mode-aware support projection seam. Legacy behavior and its exact returned
support Arc remain unchanged. The observed branch is callerless in this packet
but must preserve evaluation Need/outer polarity and forward the evaluation
epoch beside its projected Supported/Unsupported/error semantic result so a
later observed `RepositoryPackageSource` can consume it without naming or
recomputing the private key. The seam owns no DICE value or event and retains
no evaluation Result Arc beyond compute-local projection. Do not edit or
activate `host_package.rs`.

Retain only the existing evaluated semantic graph inside one Result Arc plus
one compact epoch. Included-file vectors, evaluator/module scratch, event
staging and support projection are compute-local. Add no preparation Result
Arc, collection/cache/interner/store, lock/task, direct Host read, revision,
certificate, export, caller or second event owner.

## Compatibility and proof

Exact: evaluation values/errors, Starlark semantics, include order, legacy
result/Arc behavior and evaluation/child events. Slug-native: sibling/carrier,
typed outer, epoch retry and observed support association. Deferred: observed
package source, recursive external `.bzl`, package load/query/build
publication, broader identities and exact identity bytes.

Prove distinct identity/Display; exact legacy semantic Arc projection and
result/event parity; observed semantic parity; exact equality with the
preparation epoch and per-demand `Arc::ptr_eq` with zero added demands;
PreparationCompute empty, and preparation semantic/Unsupported/RootAbsent/
evaluation error/success full prefixes; Need and typed-outer validity,
equality, no-carrier and no-event polarity; local print/error batch order,
child-before-parent events and warm suppression; both family directions and
zero support/source/ExternalBzl/load/query activation; real poll-drop
cancellation/recovery; edit/delete/recreate and A/B/A; support projection of
Supported/Unsupported/error without event/family drift; Allocative retention,
Buck2 scan, AI cleanup, serial focused/full validation and one independent
latest-diff review.

## STOP / REPLAN

STOP on every other file, a caller/export, `host_package.rs`, Cargo/BUILD,
fixture/oracle write, mixed preparation families, rebuilt/partial epochs,
moved/duplicate events, parent publication on Need/outer/cancel, retained
scratch/state, source/ExternalBzl/load/query activation, public behavior,
cap excess, multiple successors or M1 closure. `REPLAN` if evaluation adds an
untracked path edge, exact preparation Arcs cannot survive, the support seam
needs retained state/API/event ownership, another file/owner is required, or
legacy result/event semantics must change. After independent design ACCEPT,
schedule exactly one bounded implementation; after implementation ACCEPT,
return to the docs-only upper-source audit.
