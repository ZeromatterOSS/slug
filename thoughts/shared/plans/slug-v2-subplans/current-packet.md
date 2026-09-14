# Current Slug V2 Work Packet

Packet: WP-7A-authentic-bazel-skylib-payload
Status: ready

## Result and owner

Add the exact bazel_skylib 1.8.2 payload demanded while the authentic configured
CLI proof loads `@@bazel_skylib+//lib:modules.bzl` during bazel_features extension
evaluation. Stage 1 owns fixture assembly and Stage 5 owns the accepted demand
diagnostic; Stage 6 semantics remain unchanged. Use
[configured-cli-fixture.md](./configured-cli-fixture.md) for the preserved F1/F2
checkpoint and the exact F3 blocker receipt.

Exact: archive URL, SHA-256, strip prefix, verbatim bytes, and upstream notice.
Deferred: any later payload demand, new source producer, or unsupported semantic
owner exposed after this archive is present.

## Scope

Allowed changes:

- `tests/v2_oracle/fixtures/configured-cli-authentic/` for the exact
  bazel_skylib archive, copied license, notice, and manifest rows;
- `tests/v2_oracle/test_configured_cli_fixture.py` only for updated fixed
  inventory counts/hash and meaningful corruption validation;
- this manifest, canonical status, and the configured-fixture gate ledger at a
  genuine acceptance or newly named blocker.

Start with these evidence handles:

- bazel_skylib 1.8.2 BCR `source.json`: URL, empty strip prefix, and integrity
  already verified in the deterministic metadata bundle;
- accepted terminal demand: `@@bazel_skylib+//lib:modules.bzl` with selected
  registry materialization transport failure;
- [F3 invocation contract](./configured-cli-fixture.md#f3-invocation-contract):
  rerun the same portable proof after the focused diagnostic checks pass.

No production Rust, route selection, DICE-key, source-policy, toolchain, or R2
semantic change belongs in this packet. Do not add other bazel_skylib versions
or catalog-declared archives. Preserve the 16 MiB fixture cap and fail-closed
hash/assembly behavior.

## Work and validation

1. Acquire the exact pinned archive from its BCR URL, verify SHA-256 before
   installation, and copy its upstream license with provenance.
2. Add archive/license manifest rows, update total bytes and fixed inventory
   identity, and prove fresh-root offline assembly plus corruption rejection.
3. Rerun the unchanged authentic F3 proof once. Record configured closure, the
   next exact demanded input, or the exact unsupported semantic owner.

Invocation/compiler/test-selection corrections within this contract use the
orchestration skill. New source semantics, provenance mismatch, or a different
selected URL selects a smaller prerequisite rather than widening this packet.
Preserve the F1/F2 fixture checkpoint and do not acquire another payload until
the same proof names its demand.

## Immediate predecessor and durable candidate

Typed external route retention passed the 11-test diagnostic slice and natural
recursive route test; the observer harness compiled in 40.12 seconds. The same
F3 run selected/executed one test with valid observer/cleanup evidence and named
the bazel_skylib 1.8.2 payload in 9.36 seconds. Its pinned archive SHA-256 is
`6e78f0e57de26801f6f564fa7c4a48dc8b36873e416257a92bbb0937eeac8446`;
typed route identity is accepted and F1/F3 depend on adding that exact payload.

R2 snapshot: branch `review/output-conflict-r2`, commit `27e9e9c0c`,
base `97dffd5d4`, original patch SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`.
`git show 27e9e9c0c:review-evidence/validation.txt` retrieves the original receipt.
The snapshot must incorporate relevant landed prerequisites before new evidence;
its preservation metadata never enters production integration.
