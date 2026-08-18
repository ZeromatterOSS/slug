# Current Slug V2 Packet

Packet: `WP-2A-m1-post-package-load-upper-owner-audit`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted base: `a9270586`
Result: audit only the first complete upper owner after observed repository
package loading.

## Authority and caps

Write exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`: <=40 net;
- this manifest: <=180 net;
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`:
  <=140 net;
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`: <=30 net.

Aggregate docs growth is <=390 net lines against `a9270586`. Do not write
Rust, Cargo/BUILD metadata, fixtures, oracles or generated artifacts. Do not
activate a caller, change public behavior, or close M1.

## Required audit

Start from the accepted doc-hidden `RepositoryPackageLoadObservationKey` and
trace every immediate legacy package-load consumer before choosing an owner:

1. `slug_query_v2/src/loading_environment.rs` external package evaluation;
2. `slug_query_v2/src/graph.rs` external unconfigured package graph;
3. `slug_core_v2/src/runtime/dice.rs` singleton exported-source build;
4. only enough one-shot/public adapters to prove where selection and
   publication are owned.

For each path, record structural key identity and family selection, exact
semantic Result Arc and complete package epoch consumption, additional mutable
children or source-certificate requirements, Need/typed-outer/semantic order,
event ownership/replay, cancellation, retained lifetime and warm/A-B-A
behavior. Determine whether one existing upper key is the uniquely smallest
complete owner, whether a smaller shared prerequisite is required, or whether
the boundaries require formal `REPLAN`. Do not presume that query and core can
share a cutover: core's exported-source FileBytes/certificate edge and each
query graph's retained semantic sidecars must be audited explicitly.

Classify unchanged query/build values, errors, order and public events as
exact; any private sibling/carrier/typed-outer association as Slug-native; and
broader query functions, multi-package build aggregation, public publication,
exact identity bytes and M1 closure as unsupported/deferred unless separately
admitted.

## Terminal

End with exactly one independently reviewable result:

- schedule one docs-only design for the uniquely smallest complete owner;
- schedule one docs-only design for a uniquely smaller prerequisite; or
- record formal `REPLAN` with the conflicting ownership/effect boundary.

Before a design successor, freeze its natural owner, exact future Rust
allowlist and measured per-file/aggregate semantic and physical caps, complete
Arc/epoch/terminal/event/family/lifetime/lifecycle proof, compatibility classes,
retention/cleanup obligations and STOP/REPLAN conditions. At most one successor
may be scheduled. No implementation may follow without independent design
acceptance.
