# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-source-input-observation-carrier-visibility-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design and Rust base: pending docs commit / `ff0728ce`

## Goal and authority

Implement only the designed same-crate visibility handoff between the accepted
private root apparent-repository source-input observation and its future
sibling source-path owner. Promote the existing observation key/carrier through
exact `pub(super)` nominal APIs, hide the Route terminal behind one field-
private opaque outer, and prove sibling type usability without changing
computation or activating a caller.

Rust authority is exactly:

- `app/slug_core_v2/src/runtime/root_apparent_repository_source_input.rs`; and
- test-only
  `app/slug_core_v2/src/runtime/root_apparent_repository_source_path_input.rs`.

Every third Rust/API/export/caller/fixture/oracle/Cargo/BUILD file and all
orchestration docs are read-only during implementation.

## Frozen nominal surface

In source input, promote exactly the existing key to:

```rust
pub(super) struct HostRootApparentRepositorySourceInputObservationKey(
    HostRootApparentRepositorySourceInputKey,
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
observed-HostRootApparentRepositorySourceInputKey { workspace: NormalizedAbsolutePath { path: "/workspace" }, apparent_repo: ApparentRepoName("first") }
```

Promote exactly the existing carrier to
`pub(super) struct ObservedHostRootApparentRepositorySourceInput`, retaining
private `result` and `observations` fields and existing derives/Dupe. Expose
only these concrete borrowed accessors:

```rust
pub(super) fn result(
    &self,
) -> &Arc<Result<HostRootApparentRepositorySourceInput,
                 HostRootApparentRepositorySourceInputError>>

pub(super) fn observations(&self) -> &PathObservationEpoch
```

Add no observation Result/Outcome alias. Existing legacy source-input aliases
and their visibility remain unchanged. Add no carrier field, constructor,
conversion or inspector.

Rename the existing private enum exactly to
`RootApparentRepositorySourceInputObservationError`. It retains only
`Route(HostRootApparentRepositoryRouteObservationError)`, the existing Debug/
Clone/PartialEq/Eq/Allocative derives and manual Dupe. The driver outcome and
every driver terminal continue to use this private inner.

Add exactly this same-derived/manual-Dupe field-private opaque wrapper:

```rust
pub(super) struct HostRootApparentRepositorySourceInputObservationError(
    RootApparentRepositorySourceInputObservationError,
);
```

The observed Key associated Value remains
`SourcePreparationOutcome<Result<ObservedHostRootApparentRepositorySourceInput,
HostRootApparentRepositorySourceInputObservationError>>`. Only its
`Complete(Err(inner))` projection wraps
`HostRootApparentRepositorySourceInputObservationError(inner)`. Need, success,
legacy, equality, validity, driver order, route request, Result Arc, epoch,
events, retention and cancellation remain byte-equivalent. Keep the new inner,
carrier fields/Route variant and lower opaque route terminal private. Add no
outer constructor/conversion/inspector, observation alias, public field or
variant.

## Exact proof changes

In source input, change only wrapper/source spelling in existing
`production_edge_is_only_route_then_pure_projection`. Preserve all assertions
in the three accepted observation tests. Its producer scan must prove exactly
one private `RootApparentRepositorySourceInputObservationError::Route(error)`
mapping and exactly one observed Key projection
`HostRootApparentRepositorySourceInputObservationError(error)`. Add no source-
input test; real-family/event and lifecycle/cancellation/nonactivation proof
remains byte-unchanged.

In source path, add exactly one test named
`root_apparent_repository_source_input_observation_surface_is_sibling_usable`
and explicit test-only imports of the promoted key, carrier and opaque outer,
plus the existing concrete source-input value and error types. It constructs
only the observed source-input key for `/workspace`, `@first` and asserts the
exact Display above. A nonexecuted `inspect` function plus exact function-
pointer cast names:

- `<HostRootApparentRepositorySourceInputObservationKey as Key>::Value`;
- `SourcePreparationOutcome<Result<
  ObservedHostRootApparentRepositorySourceInput,
  HostRootApparentRepositorySourceInputObservationError>>`; and
- borrowed `&Arc<Result<HostRootApparentRepositorySourceInput,
  HostRootApparentRepositorySourceInputError>>` and `&PathObservationEpoch`
  results from `result` and `observations`.

The smoke cannot construct or inspect the carrier/outer, name the private
inner/Route variant, call compute, invoke source-path computation or activate
any graph edge. Production source-path imports and compute stay byte-unchanged.

## Baselines, caps and validation

Entry baselines are exact:

- source input: 1,716 physical, cfg(test) line 440, SHA-256
  `619374fed097c1423037ab80268ef714800364b2f967eb6b7b446f8b99cf4b10`;
- source path: 845 physical, cfg(test) line 300, SHA-256
  `254303b96882a68329c334662d4e07bb728b0c6b0c3eb7f78adbaf44896ff200`.

Caps are <=80 source-input production, <=50 source-input colocated proof,
<=80 source-path sibling proof, <=210 aggregate semantic additions and physical
<=1,816/925. Add no production helper or source-input test and exactly one
source-path smoke below 100. Do not enlarge any accepted observation test; the
adjusted source-shape helper remains below 200. Add no `rustfmt::skip` and
require rustfmt-stable bytes with no formatting waiver. Both files remain
cohesive below the 2,000-line trigger. This is not a hot-path or retained-
representation change.

Run serially:

1. focused
   `observed_root_apparent_repository_source_input_identity_finisher_and_terminal_algebra`
   and
   `root_apparent_repository_source_input_observation_surface_is_sibling_usable`;
2. protected observed source-input real-family/event and lifecycle tests,
   legacy source-input/source-path tests and observed root-route suite/smoke;
3. full `cargo test -p slug_core_v2`;
4. direct dependent `cargo check -p slug_commands_v2`;
5. `cargo fmt --all -- --check`; and
6. exact two-file allowlist/entry-SHA/accounting/physical/test-size/visibility/
   wrapper/source-shape checks plus `git diff --check`.

Reuse accepted source-input-owner and same-crate opaque-wrapper evidence. A
visibility-only change adds no Bazel oracle.

## Compatibility and stops

Source-input Main/Builtin/spec projection, Need and terminal order, retained
route identity, policies, errors, equality/invalidation and lower events remain
**exact** Bazel 9 compatibility. The crate-internal opaque Result-Arc+
transaction-local epoch handoff is **Slug-native**. Source-path ownership,
source observation, public-command/bootstrap activation and exact Bazel
configuration/output/ActionKey bytes remain **unsupported/deferred**.

STOP third file/type/key/carrier/adapter, crate-public visibility/root reexport,
public field/observation alias/private-inner/variant/inspector, change to legacy
aliases or their visibility, source-path production or caller activation, lower
redesign, semantic/order/event/equality/epoch/retention/cancellation drift,
proof beyond exact wrapper spelling plus one sibling smoke, formatter/cap/test
waiver, new `rustfmt::skip`, Cargo/BUILD, fixture/oracle, upper source/public/
bootstrap work, milestone closure, M8/M7B or exact identity work. REPLAN before
widening, entry-hash drift or any need to expose child terminal structure.

## Terminal

ACCEPT returns only to docs-only root apparent-repository source-path
observation-owner design. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Committed audit `bc78883d` is docs-only +217/-194. It selected this two-file
same-crate handoff as uniquely smaller than source-path ownership; accepted
source-input owner `ff0728ce` remains the Rust base.
