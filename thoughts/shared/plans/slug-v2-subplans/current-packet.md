# Current Slug V2 Packet

Packet: `WP-2A-m1-loading-query-proof-and-parallel-authority-correction-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `113a74b2`
Rust base: `a9270586`
Accepted query design: `44c1b444`
Result: freeze only the bounded proof and parallel-workspace authority correction, then retry the same retained implementation.

## Docs authority and retained candidate

Write exactly canonical/current/Stage 2/routing docs within 40/180/140/30 and
390 aggregate net-line caps. The retained seven-file Rust candidate is
non-writable during this design. Stop Cargo, Rust, fixtures, oracles, public
activation and M1 closure.

The candidate implements the accepted query owner. Focused query, structural
identity and selected-Arc proof pass; full query is 121/121 and bzlmod is
complete. Production ownership, terminal algebra, event ownership, compact
retention and compatibility classes are not reopened.

## Frozen correction

The accepted loading proof at `host_package_load_tests.rs:1439-1444` still
asserts that query graph/environment/core do not name
`RepositoryPackageLoadObservationKey`. The query design now necessarily uses
that accepted sibling in graph/environment. The retry may replace only that
static assertion with this exact split proof (subject only to rustfmt wrapping):

```rust
let query = concat!(
    include_str!("../../slug_query_v2/src/graph.rs"),
    include_str!("../../slug_query_v2/src/loading_environment.rs"),
);
let core = include_str!("../../slug_core_v2/src/runtime/dice.rs");
assert!(query.contains("RepositoryPackageLoadObservationKey"));
assert!(!core.contains("RepositoryPackageLoadObservationKey"));
```

No loading production or other loading test may change.

Three public query proofs use `tempfile::tempdir()` under mutable shared `/tmp`.
New Host observation correctly sees parallel sibling churn, so isolated tests
pass while default-parallel core replays events or rejects selected epochs.
Authorize exactly one one-line-to-four-line replacement in each of:

1. `real_query_command_drives_typed_results_and_cold_events_without_warm_replay`;
2. `direct_external_query_uses_host_route_native_materialization_and_apparent_output`;
3. `observed_query_publication_preserves_terminal_and_selected_epoch_arcs`.

Each replacement creates its own fixed test-exclusive directory below
`CARGO_MANIFEST_DIR/../../target` before any runtime, then uses
`tempfile::tempdir_in`. All other relocated bytes remain exact. The three
replacements add exactly +9 semantic/physical test lines; allow +12 rounded
margin only in `runtime/tests/query_command_tests.rs`.

The retry keeps every prior production/test cap except that core query proof
becomes relocation plus <=372 new proof and <=1,132 physical. Aggregate test,
semantic and physical caps become +1,328/+2,482/19,531. Add exactly
`app/slug_loading_v2/src/host_package_load_tests.rs` with <=4 semantic test
lines and <=3,442 physical solely for the exact assertion replacement above.

## Compatibility, proof and terminal

Exact public query behavior, loading proof truth and all legacy/direct APIs
remain exact. Stable test parents and the existing private observation/outer/
selected association are Slug-native. One-shot evaluation, external exported-
source publication, multi-build, unsupported breadth and exact identity bytes
remain deferred.

The retry must run all three corrected query tests in isolation, the full
default-parallel core suite, full query/loading/bzlmod, fmt, diff-check, exact
relocation/accounting, cleanup and independent review. Loading must no longer
report the obsolete nonactivation failure; core must no longer replay warm
events, reject repository selection or lose exact Arc identity under parallel
execution.

After independent design ACCEPT schedule exactly
`WP-2A-m1-loading-query-observed-publication-implementation-retry` with the
eight-file authority above. STOP on any production/design change, other test
body/file, weakened assertion, shared parent, cap excess or M1 closure. REPLAN
if the exact bounded correction cannot make default-parallel validation pass.
