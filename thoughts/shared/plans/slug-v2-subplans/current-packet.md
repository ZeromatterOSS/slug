# Current Slug V2 Work Packet

Packet: WP-7A-external-bzl-route-identity
Status: ready

## Result and owner

Retain the typed `HostCanonicalRepositoryLoadRouteError` when external `.bzl`
resolution fails instead of flattening it into `Arc<str>`. Stage 5 owns loading
error identity and bounded diagnostic traversal; loading and Stage 6 semantics
remain unchanged. Use
[configured-cli-fixture.md](./configured-cli-fixture.md) for the preserved F1/F2
checkpoint and the exact F3 blocker receipt.

Slug-native: typed error identity and bounded borrowed traversal. Deferred: any
later payload demand, new source producer, or unsupported semantic owner exposed
after the causal route error becomes visible.

## Scope

Allowed changes:

- `app/slug_loading_v2/src/bzl_module.rs` for typed external route-error
  retention and equality;
- `app/slug_loading_v2/src/registration_diagnostic.rs` and focused tests to
  traverse the retained route error through the accepted bounded owner;
- this manifest, canonical status, and the configured-fixture gate ledger at a
  genuine acceptance or newly named blocker.

Start with these evidence handles:

- `external_load_resolution_error` and its four call sites: two typed route
  errors and two infrastructure/frontier errors currently flattened to text;
- existing `Node::Load` bounded traversal in `registration_diagnostic.rs`;
- [F3 invocation contract](./configured-cli-fixture.md#f3-invocation-contract):
  rerun the same portable proof after the focused diagnostic checks pass.

No route selection, DICE-key, source-policy, toolchain, or R2 semantic change
belongs in this packet. Preserve infrastructure failures as bounded scalar
leaves and the existing full `Display` behavior for ordinary callers.

## Work and validation

1. Introduce a typed external route-error carrier that retains successful
   `HostCanonicalRepositoryLoadRouteError` values and classifies compute/frontier
   infrastructure without recursive formatting.
2. Route the typed load failure into the existing iterative bounded traversal;
   prove equality, poison resistance, output bounds, and natural route behavior.
3. Compile and run focused owner/dependent checks, then rerun the unchanged F3
   proof once to record its next exact demand or semantic owner.

Invocation/compiler/test-selection corrections within this contract use the
orchestration skill. New source semantics, provenance mismatch, or a different
selected URL selects a smaller prerequisite rather than widening this packet.
Preserve the F1/F2 fixture checkpoint and do not acquire another payload until
the same proof names its demand.

## Immediate predecessor and durable candidate

The bazel_features checkpoint verified 24 objects / 7,822,792 bytes at inventory
SHA-256 `0ad0ac33e4275d51db0ad436cf639709170363b9a41bf82026d6c9e30f88b928`
and passed all 3 fixture tests. The same F3 run selected/executed one test with
valid observer/cleanup evidence and reached generated repo
`@@bazel_features++version_extension+bazel_features_globals` in 9.29 seconds.
Its preformatted route error exhausted the bounded diagnostic before naming the
cause; typed retention is the immediate prerequisite.

R2 snapshot: branch `review/output-conflict-r2`, commit `27e9e9c0c`,
base `97dffd5d4`, original patch SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`.
`git show 27e9e9c0c:review-evidence/validation.txt` retrieves the original receipt.
The snapshot must incorporate relevant landed prerequisites before new evidence;
its preservation metadata never enters production integration.
