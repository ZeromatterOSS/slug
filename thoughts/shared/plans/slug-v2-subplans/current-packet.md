# Current Slug V2 Packet

Packet: `WP-2A-m1-external-singleton-revision-event-carry-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `de38d1b8`
Accepted Rust base: `a4dd40d6`
Accepted external-build design: `1a217e2a`
Accepted loading-proof correction: `ce110d9a`
Result: preserve exact child-event suppression across a source-certificate
revision retry, then resume the retained external singleton implementation.

## Exact docs authority and retained candidate

Write exactly canonical/current/Stage 2/routing under <=40/220/180/30 and
<=470 aggregate docs net against `de38d1b8`. Retain but do not write the dirty
candidate in:

- `app/slug_core_v2/src/runtime/dice.rs`;
- `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`; and
- `app/slug_loading_v2/src/host_package_load_tests.rs`.

STOP all Rust, Cargo, BUILD, fixture, oracle, export, caller and public-behavior
changes during this design.

## Formal REPLAN evidence and owner

The retained candidate passes 33/33 build-command, full loading 138/138, full
bzlmod and the documented 240/241 core baseline within +611 semantic lines and
17,915 physical. Independent implementation review accepted its build owner,
prefix/certificate/family/memory proof.

Focused server validation exposes one exact event regression. At accepted Rust
base `a4dd40d6`, the external server lifecycle passes source edit/delete/
directory/recreate and fails only on the documented later root-switch
`ROOT_EVENT` replay. The candidate fixes that switch but newly replays the
unchanged child-owned `DEP_BUILD_EVENT` when the exported source is deleted.
The other four current server query failures reproduce at the Rust base and are
unchanged baselines. Therefore the moved external-build failure is current,
not inherited.

`RepositoryPackageLoadObservationKey` solely owns the replayed batch. The build
root may neither filter it nor weaken its equality/path epoch. The gap is at
the native event-selection/revision-retry boundary: pre-revision preparation
computes the effective ordered event epoch, but retry discards it; an equality-
reused final root can hide a previously accepted child from the final closure,
causing the flat accepted epoch to drop that node and replay its equal batch on
the next reevaluation.

Freeze the correction in `runtime/events.rs` plus the narrow `runtime/dice.rs`
acceptance call site. Do not change the child key, external driver, DICE key
equality, exact path validation, event producer or public surface.

## Frozen revision-event carry

Add one private command-local `ProvisionalEventEpoch`: a compact Dupe/Allocative
Arc-backed ordered slice of `(DiceNodeId, Option<EventBatch>)`. `Some(batch)` is
the effective batch; `None` is an explicit known-removal tombstone; absence
alone permits fallback to the true prior. It is not accepted event state.

Carry that provisional epoch only in the revision-retry loop. Pre-retry
`prepare_accept` derives it from the selected exact closure, the true prior and
any earlier carry. It remains unpublished. A Need before any revision carry
behaves unchanged. Once a carry exists, preserve it through later Need attempts
for the same fixed command root; eventual final Known transitions override it.
Typed outer, cancellation, abort and every acceptance/materializer/revision
failure drop it.

On the next terminal preparation, reconcile the final `SelectedEventState`,
the carried retry epoch and the true prior accepted epoch in deterministic
order:

1. start with retry entries and tombstones in their selected closure order;
2. for a final `Known(Some(batch))`, replace the carried/prior entry and compare
   that final batch with the true prior for emission;
3. for final `Known(None)`, retain a `None` tombstone and emit nothing;
4. for final `NoTransition`, use the carried Some/None first, then true-prior
   Some, with absence only when neither exists;
5. append final-only nodes in final closure order; and
6. emit only nonempty final effective batches whose node/batch differs from the
   true prior.

Fold multiple revision retries into the carried effective epoch. Stable first
occurrence fixes ordering; latest known transition fixes value/removal. This
carry is valid only after a source-certificate revision retry and only inside
the same fixed native command root. A later ordinary Need preserves that carry
without publishing or modifying it; no carry crosses commands.

Filter only provisional `Some` entries into the effective accepted epoch;
tombstones never enter it. Accepted event state remains exactly one Arc-backed
ordered node/batch slice and changes only after materializer acceptance. The
provisional slice is owned by the stack/command attempt. Use only compute-local
merge scratch. Add no retained map, other collection, cache, interner, store,
lock, task, Host read or event owner. Hold no lock across DICE.

## Future retry authority, caps and proof

After independent design ACCEPT, schedule exactly
`WP-2A-m1-external-singleton-observed-build-implementation-retry-2` with:

1. `runtime/dice.rs`: <=300 production net, <=11,300 physical;
2. `runtime/tests/build_command_tests.rs`: <=400 test net, <=3,400 physical;
3. `slug_loading_v2/src/host_package_load_tests.rs`: the already-frozen
   line-neutral assertion only, <=3,439 physical; and
4. `runtime/events.rs`: <=80 production plus <=100 tests, <=1,950 physical.

Aggregate semantic <=880 and combined physical <=20,089 against `a4dd40d6`.
No server test edit is authorized.

Prove exact reducer sequences: prior A -> retry-hidden A -> final no-transition
suppresses and retains A; retry B -> final omitted emits B once; final C
overrides provisional B and emits only C; prior Some(A) -> retry Known(None) ->
final NoTransition remains absent with no output; a later Known(Some(A)) emits;
Some(empty) remains distinct; multiple retries preserve first node order and
latest transition.
Prove a revision retry followed by Need and equality-reused completion retains
the hidden child; cancel/abort/failure leaves the true prior epoch unchanged.

Extend the public external lifecycle to assert no package replay across source
edit/delete/directory/recreate, exact cold/error order, changed BUILD/`.bzl`
replay and external -> root suppression. Run the existing server test unchanged
and require it to pass through the entire lifecycle. Preserve full loading,
build, bzlmod and documented core/query baselines, exact caps, fmt/diff,
retention/cleanup and independent review.

Exact public values/errors/event text/order and all existing paths remain
exact. The private observed carrier, certificate and retry-event association
are Slug-native. Multi-build, one-shot, broader actions, external globs and
identity bytes remain unsupported/deferred.

STOP on any other file, child-event filtering, path/equality weakening, new
accepted state, retained scratch, behavior/family/order drift, cap excess,
baseline widening or M1 closure. REPLAN if command-local carry cannot
distinguish hidden retry descendants without suppressing genuinely removed
nodes. After retry ACCEPT return only to one docs-only remaining M1 owner audit.
