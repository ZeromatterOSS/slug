# Current Slug V2 Work Packet

Packet: WP-7A-canonical-package-diagnostic
Status: ready

## Result and owner

Replace the terminal `[diagnostic incomplete: CanonicalPackage]` in the authentic
configured CLI proof with a bounded causal projection of the underlying private
repository-package load error. Stage 5 owns the diagnostic boundary; fixture and
Stage 6 semantics remain unchanged. Use
[configured-cli-fixture.md](./configured-cli-fixture.md) for the preserved F1/F2
checkpoint and the exact F3 blocker receipt.

Slug-native: bounded borrowed diagnostic rendering and its poison/overflow
behavior. Exact/deferred: upstream semantics, new source producers, toolchain
registration behavior, and any later payload demand exposed by the causal error.

## Scope

Allowed changes:

- `app/slug_loading_v2/src/registration_diagnostic.rs` and its focused tests for
  the bounded rendering and traversal;
- `app/slug_loading_v2/src/bzl_module.rs` only for a private borrowed projection
  of `RepositoryPackageLoadError`; do not change loading or expose owned errors;
- this manifest, canonical status, and the configured-fixture gate ledger at a
  genuine acceptance or newly named blocker.

Start with these evidence handles:

- `app/slug_loading_v2/src/registration_diagnostic.rs`: the existing bounded
  borrowed traversal and current `CanonicalPackage` incomplete branch;
- `app/slug_loading_v2/src/bzl_module.rs`: the private
  `RepositoryPackageLoadError` variants whose safe scalar identity is needed;
- `app/slug_loading_v2/src/registration_diagnostic_tests.rs`: natural malformed
  external-package coverage, including poison and deterministic truncation;
- [F3 invocation contract](./configured-cli-fixture.md#f3-invocation-contract):
  rerun the same portable proof after the focused diagnostic checks pass.

No loading, DICE-key, registration, source-policy, toolchain, or R2 semantic
change belongs in this packet. Do not render an unbounded recursive `Display`
chain into the diagnostic. Preserve the existing fixed-capacity ASCII output,
poison resistance, and deterministic incomplete/truncated classifications.

## Work and validation

1. Add the smallest borrowed projection needed for
   `RepositoryPackageLoadError`; render variant identity and bounded safe scalar
   context without cloning, recursive allocation, or semantic evaluation.
2. Replace only the `CanonicalPackage` incomplete leaf in the registration
   diagnostic traversal. Prove the natural malformed-package case is causal,
   poison-free, deterministic, and bounded; preserve all existing leaf behavior.
3. Run focused owner/dependent checks within 12/15 seconds per test process.
   Compile the observer harness separately within 60 seconds, then rerun the
   unchanged authentic fresh-root F3 proof once.
4. Record one terminal receipt: configured closure, an exact demanded input, or
   an exact unsupported semantic owner. If the projection itself cannot remain
   bounded and borrowed, stop with that implementation blocker.

Invocation/compiler/test-selection corrections within this contract use the
orchestration skill. New source semantics or evidence contradicting the bounded
diagnostic design selects a smaller prerequisite rather than widening this
packet. Preserve the F1/F2 fixture checkpoint and do not acquire another payload
until the same proof names its demand.

## Immediate predecessor and durable candidate

Authentic fixture F2 is accepted at the immediately preceding checkpoint: 19
objects / 901,651 bytes, 177 bundled metadata entries, fresh-root offline
assembly, and negative missing/hash/patch checks. F1 remains partial only because
the next payload demand is hidden behind the current diagnostic. The September
14 F3 run selected/executed one test, retained valid observer/cleanup evidence,
and failed at `toolchains registration row 5: [diagnostic incomplete:
CanonicalPackage]` after selecting `rules_cc@0.2.17`.

R2 snapshot: branch `review/output-conflict-r2`, commit `27e9e9c0c`,
base `97dffd5d4`, original patch SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`.
`git show 27e9e9c0c:review-evidence/validation.txt` retrieves the original receipt.
The snapshot must incorporate relevant landed prerequisites before new evidence;
its preservation metadata never enters production integration.
