# Current Slug V2 Work Packet

Packet: WP-7A-authentic-bazel-features-payload
Status: ready

## Result and owner

Add the exact bazel_features 1.42.1 payload demanded while the authentic
configured CLI proof loads `@@bazel_features+//:features.bzl` from protobuf's
prebuilt-toolchain package.
Stage 1 owns fixture assembly and Stage 5 owns the accepted demand diagnostic;
Stage 6 semantics remain unchanged. Use
[configured-cli-fixture.md](./configured-cli-fixture.md) for the preserved F1/F2
checkpoint and the exact F3 blocker receipt.

Exact: archive and BCR patch URLs, SHA-256 values, strip prefix, patch strip,
verbatim bytes, and upstream notice.
Deferred: any later payload demand, new source producer, or unsupported semantic
owner exposed after this archive is present.

## Scope

Allowed changes:

- `tests/v2_oracle/fixtures/configured-cli-authentic/` for the exact
  bazel_features archive, patch, copied license, notice, and manifest rows;
- `tests/v2_oracle/test_configured_cli_fixture.py` only for updated fixed
  inventory counts/hash and meaningful corruption validation;
- this manifest, canonical status, and the configured-fixture gate ledger at a
  genuine acceptance or newly named blocker.

Start with these evidence handles:

- bazel_features 1.42.1 BCR `source.json`: URL, strip prefix, patch strip, and
  archive/patch integrity already
  verified in the deterministic metadata bundle;
- accepted terminal diagnostic: `SourceObservation @@bazel_features+//:features.bzl:
  CanonicalRequest: Request.Materialization path=hex:66656174757265732e627a6c`;
- [F3 invocation contract](./configured-cli-fixture.md#f3-invocation-contract):
  rerun the same portable proof after the focused diagnostic checks pass.

No production Rust, loading, DICE-key, registration, source-policy, toolchain,
or R2 semantic change belongs in this packet. Do not add other bazel_features versions
or catalog-declared archives. Preserve the fixture byte cap and fail-closed
hash/assembly behavior.

## Work and validation

1. Acquire the exact pinned archive and patch from their BCR URLs, verify both
   SHA-256 values before installation, and copy the upstream license with
   provenance. Do not read a personal cache or use implicit runtime fallback.
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

The protobuf checkpoint verified 21 objects / 7,793,087 bytes at inventory
SHA-256 `57a78210ed99a85f7461bef726e8153174ad10d62a64a0f65b263fb696f126fd`
and passed all 3 fixture tests. The same F3 run selected/executed one test with
valid observer/cleanup evidence, advanced through protobuf, and named the
bazel_features 1.42.1 payload in 8.43 seconds. Its pinned archive SHA-256 is
`8189bac9a6bf9cc155a854c4cbebfebf58b9ca7a2d0a67645f7d0c1f83c523ac`;
its BCR patch SHA-256 is
`b69c27e64c4ac5043a3f254d88ef2d8383bbfaefd20881a24ed5b6eb13d4b818`.

R2 snapshot: branch `review/output-conflict-r2`, commit `27e9e9c0c`,
base `97dffd5d4`, original patch SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`.
`git show 27e9e9c0c:review-evidence/validation.txt` retrieves the original receipt.
The snapshot must incorporate relevant landed prerequisites before new evidence;
its preservation metadata never enters production integration.
