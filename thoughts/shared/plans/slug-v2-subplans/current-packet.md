# Current Slug V2 Work Packet

Packet: WP-7A-authentic-protobuf-payload
Status: ready

## Result and owner

Add the exact protobuf 33.4 payload demanded while the authentic configured CLI
proof reads `REPO.bazel` for `@@protobuf+//bazel/private/toolchains/prebuilt`.
Stage 1 owns fixture assembly and Stage 5 owns the accepted demand diagnostic;
Stage 6 semantics remain unchanged. Use
[configured-cli-fixture.md](./configured-cli-fixture.md) for the preserved F1/F2
checkpoint and the exact F3 blocker receipt.

Exact: archive URL, SHA-256, strip prefix, verbatim bytes, and upstream notice.
Deferred: any later payload demand, new source producer, or unsupported semantic
owner exposed after this archive is present.

## Scope

Allowed changes:

- `tests/v2_oracle/fixtures/configured-cli-authentic/` for the exact protobuf
  archive, copied license, notice, and manifest rows;
- `tests/v2_oracle/test_configured_cli_fixture.py` only for updated fixed
  inventory counts/hash and meaningful corruption validation;
- this manifest, canonical status, and the configured-fixture gate ledger at a
  genuine acceptance or newly named blocker.

Start with these evidence handles:

- protobuf 33.4 BCR `source.json`: URL, strip prefix, and integrity already
  verified in the deterministic metadata bundle;
- accepted terminal diagnostic: `RouteRepoFile.SourceObservation.CanonicalRequest:
  Request.Materialization path=hex:5245504f2e62617a656c kind=Transport`;
- [F3 invocation contract](./configured-cli-fixture.md#f3-invocation-contract):
  rerun the same portable proof after the focused diagnostic checks pass.

No production Rust, loading, DICE-key, registration, source-policy, toolchain,
or R2 semantic change belongs in this packet. Do not add other protobuf versions
or catalog-declared archives. Preserve the fixture byte cap and fail-closed
hash/assembly behavior.

## Work and validation

1. Acquire the exact pinned archive from the BCR URL, verify SHA-256 before
   installation, and copy its upstream license with provenance. Do not read a
   personal cache or use implicit runtime network fallback.
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

The routed `REPO.bazel` projection passed 2 focused tests and preserved the
repository-ignore handoff tests. The same F3 run selected/executed one test with
valid observer and cleanup evidence, then named the protobuf 33.4 archive capture
while reading `REPO.bazel` in 5.61 seconds. Its pinned SHA-256 is
`687e98a471973b5c5fd711750c40b8b82c0ade33f649db65e00b290f29345a2b` and
strip prefix is `protobuf-33.4`. The diagnostic chain is accepted; F1/F3 now
depend on adding this exact payload.

R2 snapshot: branch `review/output-conflict-r2`, commit `27e9e9c0c`,
base `97dffd5d4`, original patch SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`.
`git show 27e9e9c0c:review-evidence/validation.txt` retrieves the original receipt.
The snapshot must incorporate relevant landed prerequisites before new evidence;
its preservation metadata never enters production integration.
