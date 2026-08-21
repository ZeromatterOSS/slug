# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-route-observation-carrier-visibility-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Audit and Rust base: pending docs commit / `cf30f8f2`

## Goal and decision authority

Design only the uniquely smaller same-crate visibility prerequisite between
the accepted private root apparent-route observation and its sole future
consumer in the sibling root source-input module. Freeze one minimal
`pub(super)` key/carrier/field-private opaque-outer surface plus sibling compile
proof without changing computation or activating a caller.

Write only the canonical plan, this manifest, Stage 6 and routing log at net
caps <=40/<=180/<=220/<=30 and <=470 aggregate. Rust, tests, fixtures,
oracles, Cargo/BUILD, exports and callers are read-only in this packet.

## Audited frontier and decision

Accepted `cf30f8f2` adds private
`HostRootApparentRepositoryRouteObservationKey`,
`ObservedHostRootApparentRepositoryRoute` and
`HostRootApparentRepositoryRouteObservationError` at
`root_apparent_repository_route.rs:294-539`. The observed key/carrier/outer
have zero production consumers. Their key/new, carrier/accessors and typed
outer are private to the route module, so the sibling source-input owner cannot
name the Key associated Value.

The outer's sole `Definition` variant directly names the lower opaque root-
definition observation error. Promoting that enum would reveal child terminal
structure the source-input owner does not need. Effective Key visibility
therefore requires a field-private nominal wrapper around a renamed private
inner at the Key error projection.

The legacy `HostRootApparentRepositoryRouteKey` has exactly one production
consumer: `HostRootApparentRepositorySourceInputKey` imports it at source-input
line 24 and computes it at line 186. Source input owns only route terminal/
source-capability projection from that one predecessor, so the accepted
observed route is its exact future child and no second semantic prerequisite
exists. Reusing the legacy route would discard its epoch.

The source-input key has exactly one production consumer, source-path input at
its line 234. Source-path input has exactly one production consumer, the
existing host source observation at its line 234; that observation has zero
production callers. Public command analysis instead uses Bzlmod
`RootRepositoryRouteKey` and `RootRepositoryRouteObservationKey` at
`dice.rs:4476-4494`, and `root_bootstrap.rs` remains imperative and dormant.
None directly consumes the observed route, supplies sibling visibility or
replaces its epoch.

No crate-public API, `runtime/mod.rs` or crate-root reexport, module move,
adapter, lower-carrier promotion, source-input owner, source-path/source-
observation rewrite, public-command bridge or bootstrap activation is needed.
Thus route carrier visibility is uniquely smaller than source-input
observation ownership.

## Design deliverable

Freeze exactly one minimal same-crate surface:

- the existing route observation key and only its existing two-argument
  `Option<Self>` constructor at `pub(super)`, preserving root rejection and
  exact `observed-{legacy Display}`;
- the existing carrier with private fields and concrete `pub(super)` borrowed
  `Arc<Result<HostRootApparentRepositoryRoute,
  HostRootApparentRepositoryRouteError>>` and `PathObservationEpoch`
  accessors; and
- private inner `enum RootApparentRepositoryRouteObservationError`, retaining
  exactly `Definition(HostRootApparentRepositoryDefinitionObservationError)`
  and existing derives/Dupe; plus field-private opaque
  `pub(super) struct HostRootApparentRepositoryRouteObservationError(
  RootApparentRepositoryRouteObservationError)` with matching derives/Dupe,
  wrapping only the observed Key error projection.

Keep the inner enum, carrier fields and variants private. Add no public field,
alias, variant, inspector, outer constructor/conversion, crate-root export,
adapter or semantic caller. The already `pub(super)` legacy route Result alias
is unchanged and is not a new observation surface.

Freeze exactly one test-only sibling proof in
`root_apparent_repository_source_input.rs`. It may construct only the observed
route key for `/workspace` and `@first`, assert exact Display
`observed-HostRootApparentRepositoryRouteKey { workspace:
NormalizedAbsolutePath { path: "/workspace" }, apparent_repo:
ApparentRepoName("first") }`, and use one nonexecuted function-pointer proof of
the associated `SourcePreparationOutcome<Result<carrier, opaque outer>>` plus
concrete borrowed Result-Arc/epoch accessors. It must not construct or inspect
the carrier/outer, compute the key, name the private inner/variant, invoke
source input or activate semantics. Production source-input imports remain
unchanged.

Audit only private-inner/public-wrapper spelling in existing
`observed_root_apparent_repository_route_identity_finisher_and_terminal_algebra`.
Preserve all identity/root/Display, Need, dependency, finisher, terminal, Arc,
epoch, equality and validity assertions. Driver/source evidence must name the
private inner Definition mapping exactly once and the observed Key projection
wrapper exactly once. Real-family/event and lifecycle/cancellation/
nonactivation proof remains byte-unchanged.

## Prospective authority, caps and validation

Prospective implementation authority is exactly:

- `app/slug_core_v2/src/runtime/root_apparent_repository_route.rs`, baseline
  1,890 physical/tests 541, SHA-256
  `aa25fa3d36c6b9ba7ff5a9bb4ca6565f2cb2e8d579e6d4ab6721efaf8139d8d8`;
- test-only
  `app/slug_core_v2/src/runtime/root_apparent_repository_source_input.rs`,
  baseline 814 physical/tests 271, SHA-256
  `76893b9cfd6c7358260cafe60caa8c5a6922f6b7c6e85e791c3f5603360f1dd3`.

Prospective caps are <=80 route production, <=50 route colocated proof and
<=80 source-input sibling proof; <=210 aggregate semantic additions and
physical <=1,990/894. Add no production helper or new route-module test and
exactly one sibling smoke. The adjusted route identity stays below 200 and the
smoke below 100. The route file remains cohesive below the 2,000-line trigger;
the source-input file changes only its colocated compile proof. No hot-path or
retained-representation change applies.

Prospective validation is serial: focused observed route, exact sibling smoke,
protected legacy route/source-input and observed root-definition tests, full
`cargo test -p slug_core_v2`, direct dependent
`cargo check -p slug_commands_v2`, `cargo fmt --all -- --check`, exact two-file
allowlist/SHA/accounting/physical/test-size/effective-visibility/wrapper/source-
shape checks and `git diff --check`. Reuse accepted route owner and same-crate
opaque-wrapper evidence. Add no Bazel oracle for a visibility-only change.

## Compatibility and stops

Route values/five-family projection/predecessor/views/source capability/
errors/order/equality/invalidation/lower events remain **exact** Bazel 9
compatibility. The crate-internal opaque Result-Arc+epoch handoff is
**Slug-native**. Source-input ownership, source-path/source observation,
public-command/bootstrap activation and exact Bazel configuration/output/
ActionKey bytes remain **unsupported/deferred**.

STOP implementation/activation in this packet; third file/type/key/carrier/
adapter; crate-public visibility or root export; public field/alias/private-
inner/variant exposure; source-input compute; route semantic/event/epoch/
retention drift; proof beyond wrapper spelling plus one exact smoke; formatter/
cap/test waiver; Cargo/BUILD, fixture/oracle; upper source/public/bootstrap
work, milestone closure, M8/M7B or exact identity work. REPLAN before widening.

## Terminal

Design ACCEPT may schedule only
`WP-6-7A-host-root-apparent-repository-route-observation-carrier-visibility-implementation`,
then returns to source-input observation-owner design. M7 remains partial and
M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted owner `cf30f8f2` is +816/-56 in exactly the route file, 1,890 physical
lines, and remains callerless.
