# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-route-observation-carrier-visibility-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design and Rust base: `7444a51e` / `cf30f8f2`

## Goal and authority

Implement only the designed same-crate visibility handoff between the accepted
private root apparent-route observation and its future sibling source-input
owner. Promote the existing route observation key/carrier through exact
`pub(super)` nominal APIs, hide the Definition terminal behind one field-private
opaque outer, and prove sibling type usability without changing computation or
activating a caller.

Rust authority is exactly:

- `app/slug_core_v2/src/runtime/root_apparent_repository_route.rs`; and
- test-only `app/slug_core_v2/src/runtime/root_apparent_repository_source_input.rs`.

Every third Rust/API/export/caller/fixture/oracle/Cargo/BUILD file and all
orchestration docs are read-only during implementation.

## Frozen nominal surface

In route, promote exactly the existing key to:

```rust
pub(super) struct HostRootApparentRepositoryRouteObservationKey(
    HostRootApparentRepositoryRouteKey,
);
```

The tuple field stays private. Promote only its existing constructor, with the
exact signature:

```rust
pub(super) fn new(
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
) -> Option<Self>
```

Preserve root-name rejection and `observed-{legacy Display}`. For `/workspace`
and `@first`, Display remains exactly:

```text
observed-HostRootApparentRepositoryRouteKey { workspace: NormalizedAbsolutePath { path: "/workspace" }, apparent_repo: ApparentRepoName("first") }
```

Promote exactly the existing carrier to
`pub(super) struct ObservedHostRootApparentRepositoryRoute`, retaining private
`result` and `observations` fields and existing derives/Dupe. Expose only these
concrete borrowed accessors:

```rust
pub(super) fn result(
    &self,
) -> &Arc<Result<HostRootApparentRepositoryRoute, HostRootApparentRepositoryRouteError>>

pub(super) fn observations(&self) -> &PathObservationEpoch
```

Do not expose or add an observation Result alias, fields, constructors,
conversions or inspectors.

Rename the existing private enum exactly to
`RootApparentRepositoryRouteObservationError`. It retains only
`Definition(HostRootApparentRepositoryDefinitionObservationError)`, the
existing Debug/Clone/PartialEq/Eq/Allocative derives and manual Dupe. The
driver outcome and every driver terminal continue to use this private inner.

Add exactly this same-derived/manual-Dupe field-private opaque wrapper:

```rust
pub(super) struct HostRootApparentRepositoryRouteObservationError(
    RootApparentRepositoryRouteObservationError,
);
```

The observed Key associated Value remains
`SourcePreparationOutcome<Result<ObservedHostRootApparentRepositoryRoute,
HostRootApparentRepositoryRouteObservationError>>`. Only its
`Complete(Err(inner))` projection wraps
`HostRootApparentRepositoryRouteObservationError(inner)`. Need, success,
legacy, equality, validity, driver order, child request, Result Arc, epoch,
events, retention and cancellation remain byte-equivalent. Add no outer
constructor/conversion/inspector, alias, public field or variant.

## Exact proof changes

In the route module, change only wrapper/source spelling in existing
`observed_root_apparent_repository_route_identity_finisher_and_terminal_algebra`.
Preserve every identity/root/Display, Need, dependency, finisher, terminal,
Arc, epoch, equality and validity assertion. Its producer scan must prove
exactly one private
`RootApparentRepositoryRouteObservationError::Definition(error)` mapping and
exactly one Key projection
`HostRootApparentRepositoryRouteObservationError(error)`. The observed real-
families/events and lifecycle/cancellation/nonactivation tests remain
byte-unchanged. Add no route-module test.

In source input, add exactly one test named
`root_apparent_repository_route_observation_surface_is_sibling_usable` and
explicit test-only imports of the three promoted names. It constructs only the
observed route key for `/workspace`, `@first` and asserts the exact Display
above. A nonexecuted `inspect` function plus exact function-pointer cast names:

- `<HostRootApparentRepositoryRouteObservationKey as Key>::Value`;
- `SourcePreparationOutcome<Result<ObservedHostRootApparentRepositoryRoute,
  HostRootApparentRepositoryRouteObservationError>>`; and
- borrowed `&Arc<Result<HostRootApparentRepositoryRoute,
  HostRootApparentRepositoryRouteError>>` and `&PathObservationEpoch` results
  from `result` and `observations`.

The smoke cannot construct or inspect the carrier/outer, name the private
inner/variant, call compute, invoke source-input computation or activate any
graph edge. Production source-input imports and compute stay byte-unchanged.

## Baselines, caps and validation

Entry baselines are exact:

- route: 1,890 physical, cfg(test) line 541, SHA-256
  `aa25fa3d36c6b9ba7ff5a9bb4ca6565f2cb2e8d579e6d4ab6721efaf8139d8d8`;
- source input: 814 physical, cfg(test) line 271, SHA-256
  `76893b9cfd6c7358260cafe60caa8c5a6922f6b7c6e85e791c3f5603360f1dd3`.

Caps are <=80 route production, <=50 route colocated proof, <=80 source-input
sibling proof, <=210 aggregate semantic additions and physical <=1,990/894.
Add no production helper or new route test and exactly one source-input smoke.
The adjusted identity test stays below 200 and the smoke below 100. Add no new
`rustfmt::skip`; preserve existing skips and require rustfmt-stable bytes with
no formatting waiver. Both files remain cohesive below the 2,000-line trigger.
This is not a hot-path or retained-representation change.

Run serially:

1. focused `observed_root_apparent_repository_route_identity_finisher_and_terminal_algebra`
   and `root_apparent_repository_route_observation_surface_is_sibling_usable`;
2. protected observed route real-family/event and lifecycle tests, legacy
   route/source-input tests and observed root-definition suite/smoke;
3. full `cargo test -p slug_core_v2`;
4. direct dependent `cargo check -p slug_commands_v2`;
5. `cargo fmt --all -- --check`; and
6. exact two-file allowlist/entry-SHA/accounting/physical/test-size/visibility/
   wrapper/source-shape checks plus `git diff --check`.

Reuse accepted route-owner and same-crate opaque-wrapper evidence. A visibility-
only change adds no Bazel oracle.

## Compatibility and stops

Route values, five-family projection, predecessor/views/source capability,
errors, order, equality, invalidation and lower events remain **exact** Bazel 9
compatibility. The crate-internal opaque Result-Arc+transaction-local epoch
handoff is **Slug-native**. Source-input ownership, source path/source
observation, public-command/bootstrap activation and exact Bazel configuration/
output/ActionKey bytes remain **unsupported/deferred**.

STOP third file/type/key/carrier/adapter, crate-public visibility/root reexport,
public field/alias/private-inner/variant/inspector, source-input production or
caller activation, lower redesign, semantic/order/event/equality/epoch/
retention/cancellation drift, proof beyond exact wrapper spelling plus one
sibling smoke, formatter/cap/test waiver, Cargo/BUILD, fixture/oracle, upper
source/public/bootstrap work, milestone closure, M8/M7B or exact identity work.
REPLAN before widening, entry-hash drift or any need to expose child terminal
structure.

## Terminal

ACCEPT returns only to docs-only root apparent-repository source-input
observation-owner design. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Committed audit `7444a51e` is docs-only +211/-181. It selected this two-file
same-crate handoff as uniquely smaller than source-input ownership; accepted
route owner `cf30f8f2` remains the Rust base.
