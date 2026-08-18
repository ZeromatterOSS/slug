# Current Slug V2 Packet

Packet: `WP-2A-m1-direct-local-include-horizon-observation-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling/design base: `f4ed1ce2`
Rust base: `a61de5d4`
Result: implement only the accepted shared observed direct-local
include-package horizon producer.

## Authority

Write only:

- `app/slug_bzlmod_v2/src/source_preparation.rs`
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`

Caps against Rust base `a61de5d4`: 300 production and 13,200 physical lines
in `source_preparation.rs`; 420 tests and 1,580 physical lines in the proof
file; 720 aggregate semantic and 14,780 combined physical.

## Required implementation

Keep `DirectLocalIncludePackageHorizonKey` and its Value exact. Refactor
`preflight_direct_local_include_package_horizon` and the direct branch of
`preflight_nonregistry_include_horizon` through one shared mode-aware driver.
Add one crate-private structural observed horizon sibling/carrier. Legacy keeps
the accepted inspection then
legacy `ExternalRepositoryPackageLookupKey` family. Observed selects the
accepted observed inspection and observed lookup sibling for every unique
include package. Do not activate preparation, fragment source, evaluation,
upper source/load or query.

Retain one local semantic horizon Result Arc plus one compact epoch. The exact
semantic Result may retain its existing occurrence collection; retain no child
semantic Arc or additional collection. The reusable mode-aware package-batch
driver takes route, request slice and an initial inspection epoch. Standalone
keys compute the matching inspection then call it; existing preparation keeps
calling legacy mode with its precomputed requests, and later observed
preparation may pass its observed prefix without recomputing inspection.

Parse all labels in request order before lookup; first bad label returns the
inspection prefix and activates no lookup. Deduplicate valid packages by first
occurrence, then `compute_join` all unique packages in that order. For every
Complete child, union its epoch left-first before semantic inspection and keep
only compute-local prefix snapshots. Define the first semantic terminal in
first-occurrence order: DICE LookupCompute uses the prior prefix; child lookup
semantic uses its merged prefix.

Precedence is prefix-bounded through that first semantic. First typed outer or
union error in the prefix wins with no carrier; otherwise an earlier Need wins
and returns the deterministic union of all batch Needs; otherwise return the
semantic with its snapshot. Later outer/Need/conflict is dependency-owned and
cannot replace or extend that semantic carrier. With no semantic terminal,
inspect the full batch: first typed outer/union error wins over the combined
Need, otherwise Need, otherwise success retains the full epoch. Do not retain
the outcome map or prefix snapshots.

Need and typed outer carry no carrier. InspectionCompute is empty; inspection
semantic and bad-label failures retain the inspection prefix. LookupCompute
retains only completed earlier prefixes; lookup semantic failure retains the
decisive lookup prefix; success retains every reached lookup. Complete-only
validity/equality remains Need invalid/self-unequal, typed outer by outer error,
carrier by semantic result plus epoch.

The horizon remains eventless; inspection/root-module/routed-policy children
remain the only event owners. Add no parser/batch scratch retention, store,
cache/interner, lock/task, direct Host read, request revision, source
certificate, export or caller.

## Compatibility

Include parsing, package identity/order, lookup selection/errors, horizon
values and child events remain exact Bazel 9 admitted behavior. The structural
sibling/carrier, observed batch outer algebra and epoch retention are
Slug-native. Preparation, fragment loading, evaluation, upper
source/load/query/publication and identity bytes remain unsupported/deferred.

## Proof and validation

Freeze proof for identity/Display, exact legacy parity, empty/single/duplicate
and multi-package include horizons, bad label, invalid/deleted/no-BUILD/lookup
semantic errors, inspection and every lookup Need/typed outer/compute position,
exact prefix membership and Result Arcs, stable equal-duplicate first Arc,
conflict/operation mismatch, full equality/validity, both family directions,
zero preparation/fragment/evaluation/source/load/query activation, child-only
events/warm suppression, real polled cancellation/recovery, and package/path
edit/delete/recreate plus A/B/A. Discriminate earlier semantic plus later
Need/outer, earlier Need plus semantic plus later outer, no-semantic
outer-over-Need, first of two semantics, exact decisive prefix/first Arc and
full Need union.

Run focused tests, full bzlmod/loading/query, established core baselines,
fmt/check/diff/accounting, Buck2 retention and AI cleanup, then one independent
latest-diff review. After ACCEPT, commit and return only to a docs-only
preparation/fragment/evaluation and upper source/load audit.

## STOP / REPLAN

STOP on every other file, Cargo/BUILD/fixture/oracle writes, another
file/export/caller, preparation/fragment/evaluation or upper activation, mixed
families, partial/rebuilt epochs, moved events, retained scratch/state, direct
Host reads, multiple successors or M1 closure. `REPLAN` if the standalone key
and preparation preflight cannot share one natural horizon driver, exact child
Arcs cannot survive, another owner/file is required, or legacy semantics/event
ownership must change.
