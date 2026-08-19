# Current Slug V2 Packet

Packet: `WP-2A-m1-external-singleton-observed-build-implementation-retry-2`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `5dabd4bf`
Accepted Rust base: `a4dd40d6`
Accepted external-build design: `1a217e2a`
Accepted loading-proof correction: `ce110d9a`
Accepted revision-event design: `5dabd4bf`
Result: publish one nonroot exported-source build through the observed root while
preserving exact child-event state through certificate revision retries.

## Exact Rust authority and caps

Write exactly:

1. `app/slug_core_v2/src/runtime/dice.rs`: <=300 production net and <=11,300
   physical;
2. `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`: <=400 test net
   and <=3,400 physical;
3. `app/slug_loading_v2/src/host_package_load_tests.rs`: only the accepted
   line-neutral assertion change, zero net and <=3,439 physical; and
4. `app/slug_core_v2/src/runtime/events.rs`: <=80 production plus <=100 test
   net and <=1,950 physical.

Aggregate semantic <=880 and combined physical <=20,089 against `a4dd40d6`.
No other loading byte, relocation or file. Cargo, BUILD, fixtures, oracles,
generated evidence, exports, callers and planning docs are forbidden. Large
owners remain cohesive exceptions; touched helpers stay below 200 lines.

## External owner, order and terminal algebra

Keep the structural `BuildCommandRootObservationKey`. Admit the existing root
PackageAll plus every syntactic nonroot Single; classify external wrong-kind
targets after observed package load. Root Single, multi-target and all other
identities retain neutral/legacy routing and the public observed -> neutral ->
legacy constructor order.

One private mode-aware external driver selects only matching legacy or observed
route/package/source children. Preserve exact legacy infrastructure projection
and post-`compute_join` precedence. Observed child compute errors remain
semantic with their decisive prefix. Neither mode activates its sibling.

Observed order is anchor -> route -> repository package -> exact ExportedFile
classification -> RequestRevisionKey -> selected source. Revision occurs only
after classification and before source. Missing/wrong-kind targets activate
neither revision nor source. Union every completed observed epoch left-first
before semantics; equal duplicates keep the first exact Arc, conflict/
operation mismatch is typed outer, and Need/outer is immediate and carrierless.

Prefixes are empty/anchor/anchor+route/anchor+route+package/full-source for the
corresponding compute and semantic terminals. Present, Absent, source semantic,
accepted directory WrongKind and success retain the full source child epoch as
an exact `SourceCertificate`; earlier terminals retain none. Success is one
loaded-only target with empty action closure.

Only external observed Single initializes revision and selects
`ClosureRepositories`; PackageAll stays strict-empty. `selected_snapshot`
remains the sole repository sidecar owner, full path value/Arc validation is
unconditional, and finalization reobserves the entire certificate through the
active materializer. Equal demands preserve Arcs; changed demands alone publish
one revision and retry. Every failure/abort preserves prior accepted state.

The root owns no batch. Matching anchor/module/package/source keys remain sole
owners. Preserve exact cold/error text/order, warm suppression, sibling-source
churn suppression and external -> root PackageAll no replay.

## Revision-retry event carry

Add one private command-local `ProvisionalEventEpoch`, a compact
Dupe/Allocative Arc-backed ordered slice of
`(DiceNodeId, Option<EventBatch>)`. Some is the effective batch, None is a
known-removal tombstone, and absence alone permits true-prior fallback. It is
never accepted state.

Create/fold the carry only after source-certificate revision retry. Preserve it
unchanged through any later Need attempt for the same fixed command root. No
carry crosses commands. Typed outer, cancellation, abort and every acceptance/
materializer/revision failure drop it.

Reconcile carry and final `SelectedEventState` against the true prior in this
order:

1. retry entries/tombstones retain first selected-closure order;
2. final Known(Some) replaces and final Known(None) retains a tombstone;
3. final NoTransition uses carried Some/None, then true-prior Some;
4. final-only nodes append in final closure order; and
5. only nonempty final effective Some batches differing from true prior emit.

Filter Some entries into the existing accepted `AcceptedEventEpoch`; never put
tombstones there. Multiple retries preserve first node order and latest known
transition. Publish accepted event state only after materializer acceptance.
Use only compute-local merge scratch; hold no lock across DICE.

## Loading proof, retention and compatibility

In `host_package_load_tests.rs`, only replace the stale core-negative
`RepositoryPackageLoadObservationKey` assertion with the corresponding positive
assertion. Preserve the adjacent query-positive assertion and every other byte.

Retain exactly one build Result Arc, compact full path epoch and shared compact
certificate epoch. Accepted events remain one Arc-backed Some-only slice; the
provisional tombstone slice is command-local. Child carriers/outcomes, selected
paths, event/union scratch and repository sidecars stay compute-local or
dependency-owned. Add no other map/Vec/collection, cache, store, interner, lock,
task, Host read, revision duplicate, event owner or historical snapshot.

Exact: public values/errors/classification, BUILD/module event text/order,
PackageAll/root Single/multi-target and all legacy/direct APIs. Slug-native:
observed carrier/certificate/repository and provisional retry-event association.
Unsupported/deferred: multi-build, one-shot, broader actions, external globs
and exact Bazel identity bytes.

## Proof, validation and STOP

Preserve all existing routing/family/prefix/Arc/certificate/repository/
lifecycle/cancellation/rollback proof. Add exact event reducer sequences:
prior A -> hidden retry A -> final NoTransition suppresses/retains A; retry B
omitted by final emits once; final C overrides provisional B and emits only C;
prior A -> retry None -> final NoTransition remains absent/no output -> later A
emits; Some(empty) remains distinct; revision -> Need -> completion preserves
carry; multiple retries keep first order/latest transition; every failure leaves
the true prior unchanged.

Extend the public external lifecycle to prove no package replay on source edit/
delete/directory/recreate, changed BUILD/`.bzl` replay, exact cold/error order
and external -> root suppression. Run the unchanged server lifecycle to
completion. Require 33/33 build, loading 138/138, full bzlmod, documented core/
query/server baselines, fmt/diff, exact caps, Buck2 retention, AI cleanup and
independent final review.

STOP on any other file/loading byte, child filtering, key/path equality
weakening, accepted-state expansion, new owner/cache/lock/task/Host read,
retained scratch, behavior/family/order drift, cap excess, baseline widening,
broader activation or M1 closure. REPLAN on any new blocker. After ACCEPT
return only to one docs-only remaining M1 owner audit.
