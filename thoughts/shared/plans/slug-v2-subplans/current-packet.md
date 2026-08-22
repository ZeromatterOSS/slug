# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-source-path-input-observation-carrier-visibility-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Audit base: pending docs commit / `bc95291a`

## Goal and authority

Design only the smallest same-crate visibility handoff from the accepted
private source-path observation to its sole future sibling source-observation
consumer. Freeze exact nominal visibility, opaque error projection, borrowed
accessors and a nonactivating sibling compile proof. Do not edit Rust, tests,
APIs, exports, fixtures, Cargo/BUILD or callers in this packet.

Documentation authority is only the canonical plan, this manifest, Stage 6
and the routing log, capped at <=40/<=180/<=220/<=30 additions respectively
and <=470 aggregate additions. Every other file is read-only.

## Learned frontier and decision

Accepted `bc95291a` changes only
`root_apparent_repository_source_path_input.rs` by +861/-63 and leaves it at
1,687 physical lines with tests beginning at line 481. Its private observation
surface is:

- `HostRootApparentRepositorySourcePathInputObservationKey`, with private
  three-argument `new` and exact `observed-{legacy Display}`;
- `ObservedHostRootApparentRepositorySourcePathInput`, retaining the concrete
  source-path Result Arc and transaction-local `PathObservationEpoch`; and
- `HostRootApparentRepositorySourcePathInputObservationError::Source`, naming
  the lower opaque source-input outer.

These observed names have zero production consumers outside their owner. The
legacy source-path key has exactly one production consumer: sibling
`root_apparent_repository_source_observation.rs` imports it at line 28 and
computes it first at line 234. That source-observation key has zero production
callers. It later computes `HostRepositorySourceObservationKey`, but cannot
preserve or merge the accepted source-path epoch through the private observed
associated Value.

Public command analysis is a parallel branch: `runtime/dice.rs:4476-4494`
uses public Bzlmod `RootRepositoryRouteKey` and
`RootRepositoryRouteObservationKey`, not either Host source-path key. Root
bootstrap remains dormant. Neither the callerless upper owner nor the public
route supplies effective visibility, so neither is a prerequisite.

Choose exactly this same-crate visibility design before source-observation
ownership. It is uniquely smaller than activating or redesigning the
callerless source-observation owner.

## Required design freeze

Freeze only `pub(super)` visibility for the existing observed key, its exact
constructor, the carrier and concrete borrowed Result-Arc/epoch accessors, and
one field-private opaque outer. Keep the key field, carrier fields and terminal
variant private. Directly exposing the current enum would reveal the `Source`
variant; the design must rename it to a private inner and project it through a
nominal opaque wrapper only at the observed Key boundary.

Add no observation Result alias. Existing legacy aliases and visibility remain
unchanged. Add no crate-root reexport, public field, constructor/conversion/
inspector for the outer, adapter, copied carrier, caller or compute change.

Preserve exact key identity, three-argument Option construction/root-name
rejection and Display. For `/workspace`, `@first`, `pkg/file.bzl`, Display is:

`observed-HostRootApparentRepositorySourcePathInputKey { workspace: NormalizedAbsolutePath { path: "/workspace" }, apparent_repo: ApparentRepoName("first"), requested_path: "pkg/file.bzl" }`.

Freeze exactly one test-only sibling smoke in
`root_apparent_repository_source_observation.rs`. It may construct only the
observed key, assert exact Display and type-check the associated Value,
carrier, opaque outer and concrete borrowed accessor signatures through a
nonexecuted function pointer. It may not construct or inspect carrier/outer,
compute a key or activate source observation. Production sibling imports stay
unchanged. Only wrapper spelling in the owner's existing source-shape proof may
change; every accepted semantic/identity/family/event/lifecycle assertion and
test/helper identity stays frozen.

## Baselines, caps and validation

Prospective Rust authority is exactly:

- `app/slug_core_v2/src/runtime/root_apparent_repository_source_path_input.rs`,
  1,687 physical lines, `#[cfg(test)]` at 481, SHA-256
  `bba8073d34fc9cf13d6c8c9b2572a30bbf8d96764d948509980735a110ad4371`;
- test-only
  `app/slug_core_v2/src/runtime/root_apparent_repository_source_observation.rs`,
  899 physical lines, `#[cfg(test)]` at 340, SHA-256
  `47f16b844ae86a4707e77af27679f8faae484f09bdfdd36d60a8b34399f0b937`.

Prospective caps are <=80 owner production, <=50 owner proof, <=80 sibling
proof and <=210 aggregate additions, with physical ceilings <=1,787/979. Add
no production helper or owner test and exactly one sibling smoke below 100;
enlarge no accepted test/helper. Add no `rustfmt::skip` and allow no formatter,
cap or test waiver. Both files remain cohesive existing semantic/test owners,
stay below 2,000 lines and change no hot-path or retained representation.

The implementation packet must validate the exact owner identity/source-shape
and sibling smoke; protected three observed source-path tests, legacy source-
path/source-observation and observed-source-input suites; full
`cargo test -p slug_core_v2`; direct `cargo check -p slug_commands_v2`;
`cargo fmt --all -- --check`; exact two-file allowlist/baseline-SHA/accounting/
physical/test-size/effective-visibility/wrapper/source-shape checks; and
`git diff --check`, serially. Reuse accepted owner and opaque-wrapper proof; no
new Bazel fixture or oracle is needed for a visibility-only handoff.

## Compatibility and stops

Path normalization, requested/relative-path identity, source-input projection,
admitted family values, terminals/order, equality/invalidation and lower event
ownership remain **exact** Bazel 9 compatibility. The opaque same-crate Result-
Arc+transaction-local epoch handoff is **Slug-native**. Source-observation
ownership/activation, its later carrier, public command/bootstrap activation
and exact Bazel configuration/output/ActionKey bytes remain
**unsupported/deferred**.

STOP implementation in this design packet, a third file/type/key/carrier/
adapter, crate-public/root export, public field/alias/private-inner/variant,
legacy alias/visibility change, source-observation production/compute/caller,
semantic/path/order/event/equality/epoch/retention/lifecycle drift, proof beyond
wrapper spelling plus one smoke, formatter/cap/test waiver, Cargo/BUILD,
fixture/oracle, upper/public/bootstrap work, milestone closure, M8/M7B or exact
identity work. REPLAN before widening or baseline-hash drift.

## Terminal

If the exact opaque surface and two-file proof remain feasible, ACCEPT
schedules exactly
`WP-6-7A-host-root-apparent-repository-source-path-input-observation-carrier-visibility-implementation`,
then returns to callerless root source-observation owner design. M7 remains
partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted `bc95291a` completes and validates the private source-path observation
owner at the retained full hash and exact +861/-63 accounting above. Its packet
terminal requires this docs-only carrier-visibility/source-observation audit.
