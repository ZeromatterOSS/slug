# Current Slug V2 Packet

Packet: `WP-2A-m1-loading-query-observed-publication-implementation-retry`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Rust base: `a9270586`
Accepted query design: `44c1b444`
Accepted proof correction: `e22404a8`
Accepted selection correction: `1f2fb3f6`
Result: finish and validate only the retained observed loading-query candidate.

## Exact authority and caps

Write exactly evaluator/loading_environment/graph/lib and new
`observed_loading_query.rs` in `slug_query_v2`; core `runtime/dice.rs` and new
`runtime/tests/query_command_tests.rs`; and loading
`host_package_load_tests.rs`. No other file.

Preserve per-file caps 170+20/417, 360+60/2,346, 520+100/3,771, 4/81,
760/780, 100+12/11,000, relocation+372/1,132, and loading proof +4/3,442.
Aggregate caps are +1,154 production/+1,328 tests/+2,482 semantic and 19,531
physical against `a9270586`. Relocate base core lines 7,318-8,036 exactly;
only the three accepted stable-parent replacements may differ.

## Frozen implementation and corrections

Preserve the structural observed query root, private observed graph/subtree
siblings, matching Legacy/Observed drivers, compute-local environment,
anchor/evaluator order, left-first union-before-semantic exact Arcs, immediate
sequential terminals and full subtree-batch outer > compatible Need union >
semantic > success. REPLAN rather than inventing a QueryError for Need union.

Carriers remain one natural Result Arc plus compact epoch with `Allocative` and
`Dupe`; root retains no child carrier. Environment/arena/graph/traversal/
listing/event/union scratch remains compute-local. Child keys alone own event
batches; Need/outer/cancel publishes none. Add no retained collection, cache,
interner, store, lock, task, Host read, revision, certificate or event owner.

Retain the exact loading query-positive/core-negative assertion and the three
distinct crate-target `tempdir_in` parents from `e22404a8`; no other loading or
relocated body changes.

Add the private typed two-case `NativeCommandRoot` selection-association policy
from `1f2fb3f6`. Default every root to strict path-only selection, retaining
RepositoryRequests/RepositoryValidations rejection. Only
`RootQueryCommandObservationKey` opts into closure-selected repository
sidecars. `selected_snapshot` remains sole resolver/conflict/validation owner;
materializer acceptance consumes its exact sidecars. Full observed/selected
path epoch length/demand/value/`Arc::ptr_eq` comparison remains unconditional.
Do not add repository state to the query carrier.

## Compatibility, proof and terminal

Exact public query values/errors/order/events/materialization, loading proof and
legacy/direct APIs remain exact. Private observation/selection association and
stable test parents are Slug-native. One-shot query, external exported-source,
multi-build, unsupported breadth and identity bytes remain deferred.

Prove external query accepts nonempty selected requests and validations with
exact path/result Arcs; root query selection is repository-empty; strict roots
still reject both mismatch kinds; cancel/abort accepts none; warm/lifecycle/
events/families remain exact. Run the three corrected tests isolated, then
default-parallel core, full query/loading/bzlmod, fmt, diff-check, exact
relocation/accounting, retention/cleanup and independent review.

STOP on any other file/root opt-in, unrestricted boolean, weakened validation,
retained state, production semantic/order/event drift, caller/public change,
body drift beyond the three exceptions, cap excess or M1 closure. REPLAN on
another material miss. After ACCEPT commit and return to one docs-only audit.
