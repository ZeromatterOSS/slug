# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-source-input-observation-carrier-visibility-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Audit and Rust base: pending docs commit / `ff0728ce`

## Goal and decision authority

Design only the uniquely smaller same-crate visibility prerequisite between
the accepted private root apparent-repository source-input observation and its
sole future consumer in the sibling source-path module. Freeze one minimal
`pub(super)` key/carrier/field-private opaque-outer surface plus sibling compile
proof without changing computation or activating a caller.

Write only the canonical plan, this manifest, Stage 6 and routing log at net
caps <=40/<=180/<=220/<=30 and <=470 aggregate additions. Rust, tests,
fixtures, oracles, Cargo/BUILD, exports and callers are read-only in this
packet.

## Audited frontier and decision

Accepted `ff0728ce` adds private
`HostRootApparentRepositorySourceInputObservationKey`,
`ObservedHostRootApparentRepositorySourceInput` and
`HostRootApparentRepositorySourceInputObservationError` at
`root_apparent_repository_source_input.rs:176-439`. The observed key/carrier/
outer have zero production consumers. Their key/new, carrier/accessors and
typed outer are private to the source-input module, so the sibling source-path
owner cannot name the Key associated Value.

The outer's sole `Route` variant directly names the lower opaque root-route
observation error. Promoting that enum would reveal child terminal structure
the source-path owner does not need. Effective Key visibility therefore
requires a field-private nominal wrapper around a renamed private inner at the
observed Key error projection.

The legacy `HostRootApparentRepositorySourceInputKey` has exactly one
production consumer: `HostRootApparentRepositorySourcePathInputKey` imports it
at source-path line 27 and computes it at line 234. Source path performs its
path normalization before this one predecessor, then owns only source-input
terminal/view/path projection. The accepted observed source input is its exact
future child and no second semantic or visibility prerequisite exists. Reusing
the legacy child would discard its epoch.

The source-path key has exactly one production consumer, the existing private
host source observation at its line 234; that observation key has zero
production callers. Public command analysis instead uses Bzlmod
`RootRepositoryRouteKey` and `RootRepositoryRouteObservationKey` at
`dice.rs:4476-4494`, while root bootstrap remains imperative and dormant.
None consumes the observed source-input surface, supplies sibling visibility
or replaces its epoch.

No crate-public API, `runtime/mod.rs` or crate-root reexport, module move,
adapter, lower-carrier promotion, source-path owner, source-observation rewrite,
public-command bridge or bootstrap activation is needed. Thus source-input
carrier visibility is uniquely smaller than source-path observation ownership.

## Design deliverable

Freeze exactly one minimal same-crate surface:

- the existing source-input observation key and only its existing two-argument
  `Option<Self>` constructor at `pub(super)`, preserving root rejection and
  exact `observed-{legacy Display}`;
- the existing carrier with private fields and concrete `pub(super)` borrowed
  `Arc<Result<HostRootApparentRepositorySourceInput,
  HostRootApparentRepositorySourceInputError>>` and `PathObservationEpoch`
  accessors; and
- private inner `enum RootApparentRepositorySourceInputObservationError`,
  retaining exactly `Route(HostRootApparentRepositoryRouteObservationError)`
  and existing derives/Dupe; plus field-private opaque
  `pub(super) struct HostRootApparentRepositorySourceInputObservationError(
  RootApparentRepositorySourceInputObservationError)` with matching derives/
  Dupe, wrapping only the observed Key error projection.

Keep the private inner, carrier fields and variants private. Add no public
field, alias, variant, inspector, outer constructor/conversion, crate-root
export, adapter or semantic caller. Existing legacy source-input types and
aliases stay unchanged and are not new observation surface.

Freeze exactly one test-only sibling proof in
`root_apparent_repository_source_path_input.rs`. It may construct only the
observed source-input key for `/workspace` and `@first`, assert exact Display

```text
observed-HostRootApparentRepositorySourceInputKey { workspace: NormalizedAbsolutePath { path: "/workspace" }, apparent_repo: ApparentRepoName("first") }
```

and use one nonexecuted function-pointer proof of the associated
`SourcePreparationOutcome<Result<carrier, opaque outer>>` plus concrete
borrowed Result-Arc/epoch accessors. It must not construct or inspect the
carrier/outer, compute the key, name the private inner/variant, invoke source
path or activate semantics. Production source-path imports remain unchanged.

Adjust only private-inner/public-wrapper spelling in existing source-input
source-shape proof. Preserve all identity/root/Display, Need, dependency,
finisher, terminal, Arc, epoch, equality, validity, real-family/event and
lifecycle/cancellation/nonactivation assertions. Production evidence must name
the private inner `Route` mapping exactly once and the observed Key projection
wrapper exactly once.

## Prospective authority, caps and validation

Prospective implementation authority is exactly:

- `app/slug_core_v2/src/runtime/root_apparent_repository_source_input.rs`,
  baseline 1,716 physical/tests 440, SHA-256
  `619374fed097c1423037ab80268ef714800364b2f967eb6b7b446f8b99cf4b10`;
- test-only
  `app/slug_core_v2/src/runtime/root_apparent_repository_source_path_input.rs`,
  baseline 845 physical/tests 300, SHA-256
  `254303b96882a68329c334662d4e07bb728b0c6b0c3eb7f78adbaf44896ff200`.

Prospective caps are <=80 source-input production, <=50 source-input colocated
proof and <=80 source-path sibling proof; <=210 aggregate semantic additions
and physical <=1,816/925. Add no production helper or new source-input-module
test and exactly one source-path smoke. The wrapper-only source-shape helper
adjustment may not enlarge any accepted observation test; the smoke stays below
100. Both files remain cohesive below the 2,000-line trigger and no hot-path or
retained-representation change applies. Formatting is mandatory with no waiver
or new `rustfmt::skip`.

Prospective validation is serial: the three accepted observed source-input
tests, exact sibling smoke, protected legacy source-input/source-path and
observed route tests, full `cargo test -p slug_core_v2`, direct dependent
`cargo check -p slug_commands_v2`, `cargo fmt --all -- --check`, exact two-file
allowlist/SHA/accounting/physical/test-size/effective-visibility/wrapper/source-
shape checks and `git diff --check`. Reuse accepted source-input owner and
same-crate opaque-wrapper evidence. Add no Bazel oracle for a visibility-only
change.

## Compatibility and stops

Source-input Main/Builtin/spec projection, Need and terminal order, retained
route identity, policies, errors, equality/invalidation and lower events remain
**exact** Bazel 9 compatibility. The crate-internal opaque Result-Arc+epoch
handoff is **Slug-native**. Source-path ownership, source observation,
public-command/bootstrap activation and exact Bazel configuration/output/
ActionKey bytes remain **unsupported/deferred**.

STOP implementation/activation in this packet; third file/type/key/carrier/
adapter; crate-public visibility or root export; public field/alias/private-
inner/variant exposure; source-path compute; source-input semantic/event/epoch/
retention drift; proof beyond wrapper spelling plus one exact smoke; formatter/
cap/test waiver; Cargo/BUILD, fixture/oracle; upper source/public/bootstrap
work, milestone closure, M8/M7B or exact identity work. REPLAN before widening
or hash drift.

## Terminal

Design ACCEPT may schedule only
`WP-6-7A-host-root-apparent-repository-source-input-observation-carrier-visibility-implementation`,
then returns to root apparent-repository source-path observation-owner design.
M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted owner `ff0728ce` is +941/-81 in exactly the source-input file, 1,716
physical lines, and remains callerless. Its production prefix lines 1..=439 is
unchanged from the accepted design at SHA-256
`6bf7709327d6b0070ca17449f655d747f816c050e8ea3c023921ebb49c5bb9fc`.
