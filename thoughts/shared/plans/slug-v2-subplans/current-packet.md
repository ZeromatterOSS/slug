# Current Slug V2 Packet

Packet: `WP-2A-m1-loading-query-observed-publication-implementation-retry`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Rust base: `a9270586`
Accepted query design: `44c1b444`
Accepted correction design: `e22404a8`
Result: finish and validate only the retained observed native loading-query publication candidate.

## Exact authority and caps

Write exactly:

1. `app/slug_query_v2/src/evaluator.rs`: +170 production/+20 proof, <=417;
2. `app/slug_query_v2/src/loading_environment.rs`: +360/+60, <=2,346;
3. `app/slug_query_v2/src/graph.rs`: +520/+100, <=3,771;
4. `app/slug_query_v2/src/lib.rs`: +4, <=81;
5. new `app/slug_query_v2/tests/observed_loading_query.rs`: +760, <=780;
6. `app/slug_core_v2/src/runtime/dice.rs`: +100/+12 glue, <=11,000 after
   exact relocation of base lines 7,318-8,036;
7. new `app/slug_core_v2/src/runtime/tests/query_command_tests.rs`: exact
   719-line relocation plus <=372 proof, <=1,132; and
8. `app/slug_loading_v2/src/host_package_load_tests.rs`: <=4 test lines and
   <=3,442 solely for the frozen assertion replacement.

Caps against `a9270586` are +1,154 production, +1,328 tests, +2,482 aggregate
semantic and 19,531 combined physical. Existing large owner/proof files remain
cohesive exceptions; touched helpers stay below 200 lines.

## Frozen implementation

Preserve the doc-hidden structural observed query root, private observed root/
external graph and subtree siblings, shared Legacy/Observed drivers and the
mode-aware compute-local environment. Direct and one-shot APIs remain legacy;
only the existing native public query constructor selects observed.

Retain exactly the root query Result Arc plus compact epoch; private graph and
subtree DICE values each retain one natural Result Arc plus epoch. All carriers
are `Allocative` and `Dupe`. Environment, arena, resolved graph, traversal,
listing, event and union scratch remain compute-local. Add no cache, interner,
store, lock, task, Host read, revision, certificate or event owner.

Preserve anchor-first/evaluator order, matching observed child families and
left-first union-before-semantic exact Arcs. Sequential Need/outer/semantic
stops immediately; issued subtree batches scan fully and reduce first outer/
epoch error > compatible Need union > first semantic > success. REPLAN rather
than invent a QueryError if existing Needs cannot union.

Root/graph/subtree/environment remain eventless. Child keys alone own batches;
Need/outer/cancel publishes none. Existing native terminal validation compares
the full selected epoch by value and Arc identity, and consuming projection
preserves the exact public Result Arc and event buffer.

## Exact correction

In `host_package_load_tests.rs`, replace only the obsolete concatenated upper
nonactivation block with the exact query-positive/core-negative assertion
frozen in `e22404a8`; no loading production or other loading test changes.

In the three named tests from `e22404a8`, replace only
`tempfile::tempdir()` with a distinct fixed crate-target parent created before
runtime plus `tempfile::tempdir_in`. These are the sole exceptions to exact
relocated bytes. All other relocated bodies remain byte-identical to base.

## Compatibility, proof and terminal

Exact public query values/errors/order/events, loading proof truth and all
legacy/direct APIs remain exact. Private observation/outer/selected association
and stable test parents are Slug-native. One-shot workspace evaluation,
external exported-source publication, multi-build aggregation, unsupported
query breadth and exact identity bytes remain deferred.

Run the three corrected query tests in isolation, then default-parallel full
core, full query/loading/bzlmod, fmt, diff-check, exact relocation/accounting,
retention/cleanup and independent review serially. Preserve all earlier proof:
identity, prefixes, batch positions, exact Arcs, expression breadth, events,
family isolation, cancellation, lifecycle and zero upper-build activation.

STOP on another file/caller, production semantic/order/family/event change,
weakened proof, shared stable parent, any other relocated-body drift, retained
scratch, cap excess or M1 closure. REPLAN if the bounded correction does not
make default-parallel validation pass. After ACCEPT commit and return to one
docs-only next-owner audit; do not close M1.
