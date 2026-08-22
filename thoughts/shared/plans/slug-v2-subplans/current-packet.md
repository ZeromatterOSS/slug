# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-source-path-input-observation-carrier-visibility-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: pending docs commit / `1b046a22`

## Goal and authority

Implement only the exact same-crate opaque visibility handoff from the
accepted source-path observation to a test-only sibling source-observation
compile proof. Promote no semantic caller and change no compute, branch,
terminal, event, epoch, equality, retention or lifecycle behavior.

Rust authority is exactly:

- `app/slug_core_v2/src/runtime/root_apparent_repository_source_path_input.rs`,
  production visibility/wrapper projection plus its existing source-shape proof;
- test-only
  `app/slug_core_v2/src/runtime/root_apparent_repository_source_observation.rs`,
  for one sibling surface smoke.

Every third Rust/API/export/caller file, fixtures/oracles, Cargo/BUILD and the
four orchestration records are read-only during implementation.

## Exact owner surface

In source path, make exactly
`HostRootApparentRepositorySourcePathInputObservationKey` `pub(super)` while
keeping its tuple field private. Make its existing constructor `pub(super)`
with the unchanged signature:

`pub(super) fn new(NormalizedAbsolutePath, ApparentRepoName, PathBuf) -> Option<Self>`.

Preserve root-name rejection, requested `PathBuf` identity and Display. For
`/workspace`, `@first`, `pkg/file.bzl`, the exact string is:

`observed-HostRootApparentRepositorySourcePathInputKey { workspace: NormalizedAbsolutePath { path: "/workspace" }, apparent_repo: ApparentRepoName("first"), requested_path: "pkg/file.bzl" }`.

Make exactly `ObservedHostRootApparentRepositorySourcePathInput`
`pub(super)`, retaining its existing derives/Dupe and private fields. Make only
its existing borrowed accessors `pub(super)`, with exact concrete signatures:

- `result(&self) -> &Arc<HostRootApparentRepositorySourcePathInputResult>`;
- `observations(&self) -> &PathObservationEpoch`.

Add no observation Result/Outcome alias. Existing
`HostRootApparentRepositorySourcePathInputResult`/Outcome aliases and all
legacy visibility remain unchanged.

Rename the existing typed outer to private
`RootApparentRepositorySourcePathInputObservationError`, retaining exactly
`Source(HostRootApparentRepositorySourceInputObservationError)`, its current
derives and manual Dupe. The driver outcome and child terminal mapping use only
this private inner.

Add same-derived/manual-Dupe field-private
`pub(super) struct HostRootApparentRepositorySourcePathInputObservationError(
RootApparentRepositorySourcePathInputObservationError
);`. Wrap only the observed Key's
`SourcePreparationOutcome::Complete(Err(inner))` projection. Add no outer
constructor, conversion, inspector or public field; expose no inner or variant.
Need remains carrierless and every success path is byte-semantically unchanged.

## Exact proof

In existing
`production_edge_is_path_then_source_input_only`, change only the wrapper source
shape: require exactly one
`RootApparentRepositorySourcePathInputObservationError::Source(error)` child
mapping and exactly one
`HostRootApparentRepositorySourcePathInputObservationError(error)` Key
projection. Preserve every other assertion. Do not change any of the three
accepted observation tests, their helpers or legacy source-path tests.

In the source-observation test module, add exactly one test named
`root_apparent_repository_source_path_input_observation_surface_is_sibling_usable`.
Its test-only imports name exactly the promoted key, carrier and opaque outer,
plus `PathObservationEpoch`; production imports remain unchanged.

The smoke constructs only the observed key from `/workspace`, `@first` and
`pkg/file.bzl`, then asserts the exact Display above. One nonexecuted function
and exact function-pointer cast must accept:

- `&<HostRootApparentRepositorySourcePathInputObservationKey as Key>::Value`;
- `&ObservedHostRootApparentRepositorySourcePathInput`;
- `&HostRootApparentRepositorySourcePathInputObservationError`.

Inside it, assign the carrier accessors to exact
`&Arc<HostRootApparentRepositorySourcePathInputResult>` and
`&PathObservationEpoch`. The associated Value/function-pointer spelling proves
the carrier and opaque outer without an alias. The smoke must not construct or
inspect carrier/outer, name the private inner/variant, compute any key, invoke
source observation or activate an edge.

## Baselines, caps and validation

Entry baselines are:

- source path: 1,687 physical lines, `#[cfg(test)]` at 481, SHA-256
  `bba8073d34fc9cf13d6c8c9b2572a30bbf8d96764d948509980735a110ad4371`;
- source observation: 899 physical lines, `#[cfg(test)]` at 340, SHA-256
  `47f16b844ae86a4707e77af27679f8faae484f09bdfdd36d60a8b34399f0b937`.

Caps are <=80 source-path production additions, <=50 source-path proof
additions, <=80 source-observation proof additions, <=210 aggregate additions
and physical <=1,787/979. Add no production helper or source-path test and
exactly one sibling smoke below 100. Enlarge no accepted test/helper except the
bounded source-shape assertions above. Add no `rustfmt::skip`; there is no
format, cap or test waiver. Both files remain cohesive and below 2,000 lines;
no hot-path or retained-representation change applies.

Run serially:

1. the exact sibling smoke and owner source-shape proof;
2. the three accepted observed source-path tests;
3. protected legacy source-path/source-observation and observed-source-input
   suites;
4. full `cargo test -p slug_core_v2`; its only admissible failure is the
   byte-identical accepted library query diagnostic baseline;
5. separately, `cargo test -p slug_core_v2 --test runtime`; its only admissible
   failure is the accepted `c8d2d0b5`-identical `PathObservationEpochKey`/
   configured-analysis-Needs baseline while the other 12 tests pass;
6. direct `cargo check -p slug_commands_v2`;
7. `cargo fmt --all -- --check`; and
8. exact two-file allowlist, entry SHA/accounting/physical/test-size/effective-
   visibility/wrapper/source-shape/no-skip checks and `git diff --check`.

Those two known failures are baseline accounting, not validation waivers.
Capture and compare their exact diagnostics; any changed or additional failure
is a STOP.

Reuse the accepted source-path owner and prior opaque-wrapper sibling-smoke
evidence. A visibility-only change has no Bazel oracle or fixture gap.

## Compatibility and stops

Path normalization, requested/relative-path identity, source-input projection,
admitted values/terminals/order, equality/invalidation and lower event
ownership remain **exact** Bazel 9 compatibility. The opaque same-crate Result-
Arc+transaction-local epoch handoff is **Slug-native**. Source-observation
ownership/activation and later carrier, public command/bootstrap activation and
exact Bazel configuration/output/ActionKey bytes remain
**unsupported/deferred**.

STOP third file/type/key/carrier/adapter, crate-public/root export, public field/
alias/private-inner/variant/inspector, legacy alias/visibility change, source-
observation production/import/compute/caller, semantic/path/order/event/
equality/epoch/retention/lifecycle drift, proof beyond exact wrapper source
shape plus one smoke, formatter/cap/test waiver, Cargo/BUILD, fixture/oracle,
upper/public/bootstrap work, milestone closure, M8/M7B or exact identity work.
STOP any changed/additional library or runtime-integration failure. REPLAN
before widening, baseline drift or any required format exception.

## Terminal

ACCEPT returns only to a docs-only root source-observation observation-owner
design/frontier audit. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Committed audit `1b046a22` selects this visibility boundary as uniquely smaller
than callerless source-observation ownership and freezes the exact two-file
baselines/caps above.
