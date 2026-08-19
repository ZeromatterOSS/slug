# Current Slug V2 Packet

Packet: `WP-2A-m1-observed-terminal-epoch-association-correction-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `95148642`
Accepted Rust base: `a4dd40d6`
Accepted external-build design: `1a217e2a`
Accepted loading-proof correction: `ce110d9a`
Accepted revision-event design: `5dabd4bf`
Accepted source-certified event policy: `0b4b5210`
Result: freeze the exact terminal-carrier Arc association required after an
observed root is reused across an intervening accepted root.

## Docs-only authority

Write only canonical, this manifest, Stage 2 and the orchestration routing log,
under respectively 40/220/180/30 net lines and 470 aggregate. The retained
four-file Rust candidate is evidence and is non-writable during this design.
No Rust, Cargo, BUILD, fixture, oracle, caller, server or public-behavior change
is authorized.

## REPLAN evidence and owner

Retry 5 fixes the equal package-event replay: its focused external lifecycle
and strict/source event reducer pass. The unchanged server lifecycle then
fails only after external success/edit/missing/directory/recreate, an accepted
root PackageAll command, and an external wrong-kind filegroup. Exact tracing
showed `ObservedTerminal(ResultArc)` for the Host Lstat of the external route
directory.

The intervening root command correctly removes that external observation from
the accepted command epoch. On the later external command, the observed
package child is DICE-reused and its terminal carrier still owns the prior
exact result Arc, while preflight/repository preparation supplies an equal new
Arc to the command epoch. Whole-terminal pointer validation correctly rejects
the mismatch. This is the previously frozen further-erasure REPLAN condition;
do not weaken validation, key equality, child ownership or DICE reuse.

Native terminal acceptance in `runtime/dice.rs` is the uniquely smallest
owner. It has both the Complete root carrier and the command epoch before
selection. No lower key, event owner, Host producer or side store is needed.

## Frozen association contract

For every Complete `NativeCommandRoot` that exposes `observations()`, reconcile
the terminal epoch into the command epoch before sealing and selecting the
terminal. Build one stable shared epoch with terminal entries first and current
command entries second:

- terminal-only demands install the terminal's exact shared result Arc;
- equal duplicates retain the terminal Arc;
- unrelated command demands and their exact Arcs remain unchanged;
- differing values, operation conflicts or invalid epochs fail closed before
  selection, revision finalization, materializer acceptance or publication.

The reconciled epoch is command-local and becomes the sole input to ordinary
selected-demand filtering. `selected_snapshot` still selects only the exact
activation-closure demands, constructs the same repository validations and
retains exact Arcs. Reconciliation changes Arc authority, never demand
membership: a terminal demand not selected by the closure must still fail the
existing length/demand validation. Full terminal validation by length, demand,
value and `Arc::ptr_eq` remains unconditional.

Terminal-less roots are unchanged. Need and typed outer have no terminal epoch
and do not reconcile. Any reconciliation, selection, revision, repository,
materializer, cancellation or abort failure leaves the prior accepted path,
repository and event snapshots untouched. Add no Host read, retained map/Vec,
cache, interner, task, lock, side store, child carrier or event owner.

Preserve every accepted external owner/order/prefix/certificate/repository and
source-certified event-policy contract. The terminal Result, full selected
epoch and certificate epoch remain the only retained semantic state; temporary
merge input is compute-local and reuses `PathObservationEpoch::from_shared`.
This use follows the Buck2-derived compact Arc-backed representation and adds
no parallel collection.

## Retry authority, caps and proof

After independent design ACCEPT, schedule exactly
`WP-2A-m1-external-singleton-observed-build-implementation-retry-6` in the
same four Rust files. Correct only DICE to <=380 semantic/11,400 physical;
retain build proof <=440/3,450, the line-neutral loading assertion at 3,439,
and events <=100 production plus <=160 tests/2,050. Aggregate <=1,080 semantic
and <=20,339 physical against `a4dd40d6`. No server-test write.

Discriminating proof must cover terminal-only installation; equal fresh command
Arc replacement by exact terminal Arc; unrelated Arc preservation; differing
value/conflict failure before selection/publication with prior path/repository/
event snapshots intact; strict selected-demand membership/length; external
success -> root PackageAll -> external wrong-kind and success with exact Arcs;
observed query/build root switches; warm reuse, cancellation and child-event
parity. Retain the complete external, certificate, revision, event-policy,
loading assertion, lifecycle, rollback and exact-cap proof.

Exact: public values/errors, selected paths/repositories and child events.
Slug-native: observed terminal-carrier/command-epoch Arc association.
Unsupported/deferred: multi-build, one-shot adapters, broader actions/external
globs and exact identity bytes.

STOP on Rust during design; another file/owner/state/API, direct Host read,
membership weakening, pointer-validation weakening, stale-value preference,
child/event drift, retained scratch, cap excess, broader activation or M1
closure. REPLAN if stable terminal-first shared reconciliation cannot preserve
the exact selected carrier. After accepted retry 6 return only to one docs-only
M1 owner audit.
