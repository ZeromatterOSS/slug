# Current Slug V2 Packet

Packet: `WP-2A-m1-external-singleton-observed-build-implementation-retry-5`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `0b4b5210`
Accepted Rust base: `a4dd40d6`
Accepted external-build design: `1a217e2a`
Accepted loading-proof correction: `ce110d9a`
Accepted revision-event design: `5dabd4bf`
Accepted source-certified event policy: `0b4b5210`
Result: publish one external exported-source build with exact child-event state
across direct acceptance and certificate revision retries.

## Exact Rust authority and caps

Write exactly `runtime/dice.rs` <=340 production/11,350 physical,
`runtime/tests/build_command_tests.rs` <=440 tests/3,450,
`slug_loading_v2/src/host_package_load_tests.rs` only the accepted
line-neutral assertion at 3,439, and `runtime/events.rs` <=100 production plus
<=160 tests/2,050. Aggregate <=1,040 semantic and <=20,289 physical against
`a4dd40d6`. No other file/loading byte. Planning docs, Cargo, BUILD, fixtures,
oracles, generated evidence, exports, callers and server tests are forbidden.
Remove temporary trace logging; touched helpers stay below 200 lines.

## External and event acceptance contract

Preserve structural observed admission, exact matching-family legacy/observed
driver, anchor -> route -> package -> ExportedFile classification -> revision
-> source order, union-before-semantic prefixes, exact source-child certificate,
external-only repository selection, selected value/Arc validation, child-only
event ownership, exact legacy infrastructure projection, compact retention and
all existing failure polarity.

Add private terminal-dependent
`EventReconciliationPolicy::{Strict, SourceCertifiedCurrentClosure}` on
`NativeCommandRoot`. Default every root/terminal to Strict. Only an observed
external singleton terminal retaining a SourceCertificate opts in. PackageAll
and every legacy/query/cquery/other build terminal remain Strict. Pass policy
explicitly through prepare_accept; retain no policy state.

Strict preserves existing KnownNone removal. SourceCertifiedCurrentClosure
applies to normal accepted output and provisional retry state. Root mismatch
uses Strict. With exact ordered roots, iterate current closure order:
prior+KnownSome (including empty) uses current; prior+KnownNone/NoTransition
carries prior; absent prior drops; new KnownSome contributes. Preserve this
policy through actual final retries, later Needs, tombstones for removed domain,
multiple-retry latest transition, true-prior delta and post-materializer atomic
replacement. Every failure changes no accepted state.

The opt-in terminal must retain a certificate and every reachable
semantic-Complete event-owning child must store Some(batch), including
Some(empty); Need/outer stores none and cannot accept. Present-prior KnownNone
is therefore transient under this policy, while closure absence is removal.

Retain only build Result/path/certificate epochs and compact Dupe/Allocative
accepted/provisional root/entry slices. Policy, closure/dependency graph,
children, selected paths, maps/Vecs and repository scratch stay compute-local
or dependency-owned. Add no cache/store/interner/lock/task/Host read, child
carrier, event owner or snapshot.

## Proof, compatibility and STOP

Preserve all external routing/family/prefix/Arc/certificate/repository/
lifecycle/cancellation/rollback proof and the exact line-neutral loading
assertion. Add: Strict prior Some->KnownNone removal; source policy same case
carries only with matching roots; mixed absent/mismatch/reorder/changed/
Some(empty)/new/current-order table; real source edit proves direct acceptance,
retains exact package epoch membership and emits nothing; delete/directory/
recreate; simultaneous BUILD or `.bzl` change/removal and exact replay; forced
actual revision retry plus Need; final transitions/retries/failures; certificate
and producer invariant; root switch and unchanged server lifecycle.

Exact: public values/errors and child event text/order. Slug-native: observed
certificate/repository, accepted roots and terminal-scoped association.
Unsupported/deferred: multi-build, one-shot, broader actions/external globs and
exact identity bytes.

Run focused event/build, 33/33 build, loading 138/138, full bzlmod, documented
core/query/server baselines, fmt/diff, exact caps, Buck2 retention, AI cleanup
and independent final review.

STOP every other file/loading byte, global KnownNone change, opt-in without a
certificate, child filtering, path/key weakening, absent prior carry,
prior-order replay, producer-invariant failure, retained policy/closure/map,
behavior/family drift, cap excess, broader activation or M1 closure. REPLAN on
any new blocker. After ACCEPT return only to one docs-only M1 owner audit.
