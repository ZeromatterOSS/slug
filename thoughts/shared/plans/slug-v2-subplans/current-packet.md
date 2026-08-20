# Current Slug V2 Packet

Packet: `WP-6-7A-host-selected-extension-definition-load-requests-observation-carrier-promotion-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `f622babe`
Accepted predecessor: `e82057f2`

## Goal

Expose only the already accepted observed definition-load-request key, carrier
and opaque typed outer through `slug_bzlmod_v2`'s doc-hidden API so the later
loading owner can consume them. Change no semantic computation, caller, event,
DICE dependency, retained state or Bazel-visible behavior.

## Write authority and caps

Write exactly:

- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`; and
- new
  `app/slug_bzlmod_v2/tests/definition_request_observation_api.rs`.

Every other file is read-only, including loading, Cargo/BUILD metadata,
fixtures, oracles and planning documents until terminal rollover.

Baselines are 11,657 selected-repo-spec lines with first `#[cfg(test)]` at
4,484 and 403 lib lines. Caps are <=70 production, <=40 colocated proof, <=60
external proof and <=170 aggregate semantic lines. Physical caps are
11,730/415/60 for selected-repo-spec/lib/API smoke. Helpers/tests remain below
100 lines.

## Exact implementation

In `selected_repo_spec.rs`:

1. Make the existing
   `HostSelectedExtensionDefinitionLoadRequestsObservationKey` doc-hidden
   public and make only its existing `new(NormalizedAbsolutePath)` constructor
   public. Preserve fields, derives, Hash and Display identity.
2. Make `ObservedHostSelectedExtensionDefinitionLoadRequests` doc-hidden
   public with private fields. Make only `result()` and `observations()`
   public. Spell `result()` with the concrete public
   `Arc<Result<HostSelectedExtensionDefinitionLoadRequests,
   HostSelectedExtensionDefinitionLoadRequestsError>>` return type; do not
   expose the private alias.
3. Keep `DefinitionLoadRequestsObservationError` private and unchanged. Add a
   doc-hidden public
   `HostSelectedExtensionDefinitionLoadRequestsObservationError` newtype
   around it with the same Debug/Clone/PartialEq/Eq/Allocative/Dupe traits.
   Wrap the private error only when the observation key projects its public
   `Key::Value`. The driver and mapping error graph remain private.
4. Update only the existing same-file evaluation-input child and proofs needed
   to carry/match the opaque wrapper. Do not alter request computation,
   Complete/Need/outer projection, equality/validity, Result/epoch Arc identity,
   error precedence or lifecycle behavior.

In `lib.rs`, add doc-hidden reexports for exactly the observation key, observed
carrier and public opaque observation error. Do not reexport the result alias,
private error kind, evaluation-input observation types or mapping internals.

## Proof and validation

Add one external integration smoke that:

- imports/names exactly the three new reexports;
- constructs the observation key from a normalized absolute workspace and pins
  its existing Display text;
- type-checks calls to the carrier Result/epoch accessors and names the opaque
  outer without constructing either value; and
- contains no DICE compute, semantic caller, private-source scan or test hook.

Reuse the existing focused observed-definition-request tests for behavior,
identity, event, Need/outer, cancellation and A -> B -> A proof. Run serially:

1. `cargo test -p slug_bzlmod_v2 observed_definition_requests --quiet`;
2. `cargo test -p slug_bzlmod_v2 --test definition_request_observation_api --quiet`;
3. `cargo test -p slug_bzlmod_v2 --quiet`;
4. `cargo check -p slug_loading_v2 --quiet`;
5. `cargo fmt --all -- --check`; and
6. `git diff --check`.

Do not add an oracle: visibility/wrapping changes no admitted Bazel surface.

## Compatibility, lifetime and events

Exact compatibility remains existing request semantic values, errors, order and
lower child events. The hidden Rust/DICE key/API/carrier—including key identity,
Result/epoch association, shared Arc and opaque typed outer—is Slug-native and
must remain unchanged except for visibility and the nominal wrapper.

The observation parent stays eventless and retains exactly one local request
Result Arc plus its compact epoch. Need/outer/cancellation remains carrierless;
warm reuse remains silent. No child carrier, mapping result, traversal/event
scratch, map, cache, interner, store, lock, task, direct Host read, revision or
certificate state may be added or retained.

## Terminal and stops

Implementation ACCEPT records exact accounting and validation, then activates
only `WP-6-7A-loaded-module-extension-definitions-observation-design`.
REPLAN before any wider or semantically different change.

STOP loading changes or imports; a caller or DICE compute; a second key,
adapter, owner or result alias; public fields; mapping-error or evaluation-input
observation exports; semantic/error/order/equality/event/lifecycle drift;
Cargo/BUILD changes; fixture/oracle work; proof/cap waiver; milestone closure;
M8/M7B or exact identity-byte work. M7 remains partial and M7A -> M8 -> M7B
remains.

## Immediate predecessor

Audit/design `f622babe` selects this uniquely smaller visibility prerequisite
over accepted Rust `e82057f2`. The eventual loaded-definition owner remains in
loading and its observed Host Bzl child is already crate-local; no loading or
upper-owner work is part of this packet.
