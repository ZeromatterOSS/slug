# Current Slug V2 Work Packet

Packet: WP-7A-authentic-rules-java-payload
Status: ready

## Result and owner

Add the exact rules_java 9.1.0 payload demanded while the authentic configured
CLI proof loads `@@rules_java+//toolchains/REPO.bazel`. Stage 1 owns fixture
assembly and Stage 5 owns the accepted selected-registry demand diagnostic;
Stage 6 semantics remain unchanged. Use
[configured-cli-fixture.md](./configured-cli-fixture.md) for the preserved
F1/F2 checkpoint and exact F3 blocker receipt.

Exact: archive URL, SHA-256, empty strip prefix, verbatim bytes, and upstream
notice. Deferred: any later payload demand, source producer, or unsupported
semantic owner exposed after this archive is present.

## Scope

Allowed changes:

- `tests/v2_oracle/fixtures/configured-cli-authentic/` for the exact rules_java
  archive, copied license, notice, and manifest rows;
- `tests/v2_oracle/test_configured_cli_fixture.py` only for updated fixed
  inventory counts/hash and meaningful corruption validation;
- this manifest, canonical status, and the configured-fixture gate ledger at a
  genuine acceptance or newly named blocker.

Start with these evidence handles:

- bundled rules_java 9.1.0 BCR `source.json`: URL
  `https://github.com/bazelbuild/rules_java/releases/download/9.1.0/rules_java-9.1.0.tar.gz`,
  empty strip prefix, and archive SHA-256
  `4e1a28a25c2efa53500c928d22ceffbc505dd95b335a2d025836a293b592212f`;
- accepted terminal demand: `@@rules_java+//toolchains` `REPO.bazel`, with
  selected-registry materialization transport failure;
- [F3 invocation contract](./configured-cli-fixture.md#f3-invocation-contract):
  rerun the same portable proof after the focused fixture checks pass.

No production Rust, route selection, DICE key, source policy, toolchain, or R2
semantic change belongs in this packet. Do not add another rules_java version
or any catalog-declared archive that the proof did not demand. Preserve the
16 MiB fixture cap and fail-closed hash/assembly behavior.

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
Preserve the F1/F2 fixture checkpoint and acquire no further payload until the
same proof names its demand.

## Immediate predecessor and durable candidate

The native command now derives 64 fixed injected partitions from the sole full
`PathObservationEpoch`. Exact-demand lookup uses its partition and direct-epoch
callers retain the full-epoch lookup path. Add/change/remove and A/B/A tests
preserve exact results and stale-partition clearing. The workspace suite passes
46/46; focused bzlmod source observation and Core epoch-association tests pass.
The observer harness compiled in 48.22 seconds after bounded dependency checks.

The unchanged F3 proof selected and executed one test in 2.96 seconds with
valid observer and complete cleanup evidence, down from the prior 12-second
deadline. It named the rules_java 9.1.0 archive while loading
`@@rules_java+//toolchains/REPO.bazel`. No configured closure is accepted yet.

R2 snapshot: branch `review/output-conflict-r2`, commit `27e9e9c0c`,
base `97dffd5d4`, original patch SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`.
`git show 27e9e9c0c:review-evidence/validation.txt` retrieves the original receipt.
The snapshot must incorporate relevant landed prerequisites before new evidence;
its preservation metadata never enters production integration.
