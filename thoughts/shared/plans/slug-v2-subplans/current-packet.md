# Current Slug V2 Packet

Packet: `WP-2A-m1-external-singleton-source-certified-event-policy-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `c1b7cca1`
Accepted Rust base: `a4dd40d6`
Accepted external-build design: `1a217e2a`
Accepted loading-proof correction: `ce110d9a`
Accepted revision-event design: `5dabd4bf`
Superseded revision-only closure design: `56ed9923`
Result: freeze the terminal-scoped event policy required when source preflight
accepts directly without a revision retry.

## Exact docs authority and stop evidence

Write exactly canonical/current/Stage/routing under 40/220/180/30 net and 470
aggregate against `c1b7cca1`. Retain but do not write the four dirty Rust
files. Cargo, BUILD, fixtures, oracles, server tests, generated evidence,
callers, exports and public behavior are forbidden.

Exact retry-4 logs show cold, warm and source edit each call reconciliation with
no carried provisional epoch; the edit accepts directly after preflight. Its
ordinary accepted fold drops from four event entries to two, while the unused
current-closure provisional contains all four. Thus revision-only
reinterpretation cannot affect the failing path.

Global `Known(None)` reinterpretation is unsound because strict query/build
roots and the existing event reducer use it for real removal. The native root/
terminal boundary is the smallest owner that knows whether the admitted
source-certificate producer invariant applies.

## Frozen terminal policy and current-closure algebra

Add a private terminal-dependent
`EventReconciliationPolicy::{Strict, SourceCertifiedCurrentClosure}` hook to
`NativeCommandRoot`. Default every root/terminal to Strict. Only an observed
external singleton terminal that retains a `SourceCertificate` opts into
SourceCertifiedCurrentClosure. PackageAll and every legacy/query/cquery/other
build terminal remain Strict. Pass the typed policy explicitly through
`prepare_accept` to event reconciliation; retain no policy in accepted state.

Strict preserves existing ordinary and retry behavior byte-for-semantics,
including prior Some -> current Known(None) removal.

For SourceCertifiedCurrentClosure, apply the same fold to normal accepted output
and provisional retry state. Exact ordered root equality gates association;
root mismatch uses Strict. With matching roots, iterate exact current closure
order:

1. prior event node + Known(Some), including empty, uses current;
2. prior event node + Known(None) or NoTransition carries prior Some;
3. prior event node absent from current closure drops;
4. new current node contributes only Known(Some), in current order.

For actual retries, preserve final current-order folding with the same policy,
revision->Need carry, tombstones for removed prior-domain entries, multiple
retry latest transition, true-prior delta and post-materializer atomicity.
Outer/cancel/abort/selection/revision/materializer failure changes no accepted
roots/events.

Freeze the opt-in producer invariant: the terminal has a certificate and every
reachable semantic-Complete event-owning child stores Some(batch), including
Some(empty); Need/outer stores none and cannot be accepted. Therefore present
prior Known(None) is transient lineage for this policy, while exact closure
absence is removal. REPLAN on any invariant violation.

Accepted/provisional roots and entries remain compact Dupe/Allocative Arc
slices. Maps/Vecs/closure state remain compute-local. Add no retained policy,
closure/dependency graph/map, cache, store, interner, lock, task, Host read,
child carrier, event owner or snapshot.

## Retry authority, proof and compatibility

After independent ACCEPT schedule exactly
`WP-2A-m1-external-singleton-observed-build-implementation-retry-5` in the
same four Rust files and unchanged caps: DICE +340/11,350, build proof
+440/3,450, loading zero/3,439, events +100 production/+160 tests/2,050;
aggregate <=1,040 semantic and <=20,289 physical against `a4dd40d6`.
Remove temporary logs.

Preserve every external owner/order/prefix/certificate/repository/family/event,
legacy infrastructure and compatibility contract from `1a217e2a`,
`ce110d9a`, and `5dabd4bf`.

Proof: Strict prior Some -> KnownNone removes and all query/build reducer
behavior stays unchanged; source policy carries the same case only with
matching roots; mixed absent/mismatch/reorder/changed/empty/new/current-order
table; a real source edit proves no RevisionRetry yet retains exact package
event membership and emits nothing; delete/directory/recreate, simultaneous
BUILD or `.bzl` change/removal and exact changed replay; a forced actual
revision retry plus Need preserves carry; final transitions/retries/failures
remain atomic; root switch and unchanged server lifecycle pass. Prove the
certificate and semantic-Complete Some(including empty) invariant.

Exact: public values/errors and child event text/order. Slug-native: observed
certificate/repository, accepted roots and terminal-scoped event association.
Unsupported/deferred: multi-build, one-shot, broader actions/external globs and
exact identity bytes.

STOP now on Rust/out-of-scope docs, global KnownNone changes, opt-in without a
certificate, child filtering, path/key weakening, carrying absent prior nodes,
prior-order replay, producer-invariant failure, retained policy/closure/map,
behavior/family drift, cap excess, broader activation or M1 closure. Schedule
only retry 5 after independent design acceptance.
