# Current Slug V2 Work Packet

Packet: WP-7A-path-observation-projection
Status: ready

## Result and owner

Make each exact `PathObservationDemand` a DICE projection of the injected
`PathObservationEpoch`, so adding an unrelated observation does not force every
completed registry, module, and source chain to recheck. This is Slug-native
request scheduling owned by `slug_workspace_v2`; path values, source
certificates, retry ordering, and every Bazel-visible result remain unchanged.

Exact: the demand selects only its matching epoch entry, retains transient
`Need` invalidity, and cuts off when that entry is unchanged.
Deferred: batching new demands, replacing the epoch owner, source or repository
semantics, and optimization of any other DICE key family.

## Scope

Allowed changes:

- `app/slug_workspace_v2/src/path_observation.rs` for the projection key/helper
  and focused invalidation tests;
- existing `PathObservationKey` consumers in `app/slug_workspace_v2`,
  `app/slug_bzlmod_v2`, and `app/slug_core_v2` only to use the projection helper;
- focused dependent tests needed to preserve Need/Complete/error behavior;
- this manifest, canonical status, and the configured-fixture gate ledger after
  the unchanged F3 proof names the next boundary.

Start with these evidence handles:

- `dice/dice/src/api/computations.rs`: `compute_opaque` and `projection` make the
  dependency the projected value rather than the entire opaque value;
- `dice/dice/src/api/projection.rs`: projection equality and validity contracts;
- the accepted `PathObservationEpochKey`/`PathObservationKey` behavior and tests
  in `app/slug_workspace_v2/src/path_observation.rs`;
- [configured fixture ledger](./configured-cli-fixture.md): the authentic
  bazel_skylib run and bounded fanout receipt.

Do not add a cache, side store, direct filesystem read, fallback scan, mutable
per-command registry, or a second path epoch. The existing epoch remains the
injected DICE-retained semantic owner; the projection retains only the exact
demand and its derived `PathOutcome`. Request revision publication,
create/edit/delete/recreate observation behavior, repository materialization,
source certificates, overlap rules, cancellation, and teardown remain intact.

## Work and validation

1. Express the exact-demand lookup as a `ProjectionKey` over the opaque
   `PathObservationEpochKey`, with the current display identity, equality, and
   invalid `Need` behavior.
2. Route all production consumers through one helper. Add a focused DICE test
   proving that an unrelated additive epoch update does not re-evaluate a
   completed consumer while the newly supplied demand becomes complete.
3. Run the workspace path-observation slice and the smallest affected
   bzlmod/Core slices within the 12-second test limit; compile the observer
   harness separately within 60 seconds if its dependency graph changed.
4. Rerun the unchanged authentic F3 proof once. It must finish before the
   12-second deadline and name configured closure, the next exact payload, or an
   unsupported semantic owner. A later unattributed deadline is a new blocker,
   not authority to widen this packet.

The fixed performance discriminator is removal of the additive-epoch
invalidation fanout: the focused key counter stays unchanged for an unrelated
epoch append. The end-to-end discriminator is the existing F3 wall gate; do not
raise it or use diagnostic logging for acceptance.

## Immediate predecessor and durable candidate

The bazel_skylib checkpoint verifies 26 objects / 7,878,817 source bytes at
inventory SHA-256
`5800c9ed0df22c05229ddd908812304efa13e31609c5dd7377fd06d43d823818`.
All three focused fixture tests pass. The unchanged proof selected one test and
reached its 12-second wall deadline in `RootCompute` with valid observer and
complete cleanup evidence. A bounded scratch trace completed 92,500 DICE key
outcomes before that boundary: all 184 registry-file keys, 157 discovered-module
keys, and 156 module-source keys were revisited 115 times, while the run was
still advancing through rules_cc loads. This matches the previously recorded
global path-epoch fanout and selects the exact-demand projection owner; it is
not evidence of a semantic loop or F3 acceptance.

R2 snapshot: branch `review/output-conflict-r2`, commit `27e9e9c0c`,
base `97dffd5d4`, original patch SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`.
`git show 27e9e9c0c:review-evidence/validation.txt` retrieves the original receipt.
The snapshot must incorporate relevant landed prerequisites before new evidence;
its preservation metadata never enters production integration.
