# Current Slug V2 Packet

Packet: `WP-2A-m1-multi-build-observed-publication-implementation-retry`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted Rust base: `3f1d4dd4`
Accepted semantic design: `a2d440cb`
Accepted analysis-error correction: `5e1df076`
Result: resume and complete the retained observed multi-build candidate under
the corrected transient-analysis acceptance contract.

## Exact Rust authority and caps

Write exactly:

1. `app/slug_core_v2/src/runtime/dice.rs`: <=410 production plus <=40
   colocated tests; <=11,700 physical.
2. `app/slug_core_v2/src/runtime/demands.rs`: <=20 production; <=1,230
   physical.
3. `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`: <=500 tests;
   <=3,900 physical.

Aggregate semantic <=970 and combined physical <=16,830. Every other Rust
file, Cargo, BUILD, fixture, oracle, generated artifact and caller/public
surface is forbidden.

## Frozen implementation contract

Preserve `a2d440cb`: extend only private
`BuildCommandRootObservationKey` to already-validated root-only multi
requests; keep empty, singleton neutral/PackageAll/external and every direct
legacy/one-shot route exact. Anchor first, then request-ordered
`compute_join` branches through matching observed package/analysis/path
families, then the matching configured action closure. Merge every Complete
local epoch left-first before semantic inspection. Preserve first typed outer,
incompatible-Need failure, compatible Need union, first semantic error and
ordered success. STOP rather than inventing a new public error if live Need
kinds cannot union.

Observed multi initializes `RequestRevisionKey` before exported-source
observation. Build one request-ordered aggregate `SourceCertificate` with
stable shared Arcs. Take each source certificate out of its branch target/error
after merging it, so the semantic Result matches legacy multi behavior and the
aggregate beside the Result Arc is the sole retained certificate. The terminal
retains exactly one Result Arc, one local anchor/package/source epoch and at
most one aggregate certificate epoch. Branch outcomes, analysis remainder,
Needs, maps/frontiers and union scratch remain compute-local.

Success uses multi-only `SelectedDependencySuperset`: repository sidecars are
forbidden, every terminal demand must be an exact pointer-identical selected
demand, terminal-only demands fail, and additional analysis/action-closure
entries stay selected-closure owned. Every other observed root keeps exact full
epoch validation.

Implement `5e1df076` only for a multi semantic
`BuildCommandErrorKind::Analysis` terminal. Preserve exact legacy
unavailable-root pruning, then extend the selected unscoped path set with every
demand in the already-associated local terminal epoch before snapshot
construction. Add the bounded helper in `runtime/demands.rs`; preserve
repository fields, sort/deduplicate compute-locally, and source all values/Arcs
from the terminal-first command epoch. Validate the full local epoch and
certificate subset. Do not use this policy for success or any other root/error.

The root remains eventless. Child package/analysis keys remain sole batch
owners. Certificate terminals use `SourceCertifiedCurrentClosure` only with
the semantic-Complete Some(including empty) invariant; no-certificate terminals
stay Strict. Need/outer/cancel and selection/revision/materializer/publication
failure publish nothing and leave prior path/repository/event state intact.
Retain no selected set, child carrier Arc, cache/interner/store, new lock/task
or direct Host read. Keep touched helpers below 200 lines and require
`Allocative`/`Dupe`, Buck2 retention and AI cleanup review.

## Compatibility, proof and STOP

Exact: public multi target/error/order/configured semantics and child events;
all singleton, legacy/direct and one-shot behavior. Slug-native: private
observed multi carrier/outer, aggregate-only certificate,
SelectedDependencySuperset and transient-analysis terminal-local association.
Deferred: external/mixed multi, recursive patterns, one-shot migration,
broader actions/globs and exact Bazel identity bytes.

Proof must retain the full `a2d440cb` identity/branch/order/Need/outer/epoch/
revision/event/family/cancellation/lifecycle matrix and additionally
discriminate:

- real source-plus-rule analysis error acceptance/recovery with exact local
  prefix, aggregate certificate, legacy semantic/event parity and no repository
  sidecar;
- recursive-analysis success with a strict selected remainder, terminal-only
  rejection and default-root rejection of the same local injection;
- aggregate-only branch semantics plus exact two-source certificate Arcs and
  direct legacy parity;
- an invocation-exclusive parent created before retained runtimes, followed by
  warm/source-edit/restore and default-parallel validation; and
- exact cap/physical accounting, full focused/broad validation and rollback,
  retention and cleanup scans.

STOP/REPLAN on any fourth Rust file, wider unavailable-root policy, successful
terminal-local admission, repository/event/equality drift, retained selected
state, cap excess, partial validation or M1 closure. After independent ACCEPT,
schedule only one docs-only remaining-owner audit.
