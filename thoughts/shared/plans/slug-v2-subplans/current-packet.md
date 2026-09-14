# Current Slug V2 Work Packet

Packet: WP-7A-repository-package-source-diagnostic
Status: ready

## Result and owner

Replace the terminal `CanonicalPackage: Source` in the authentic configured CLI
proof with a bounded causal projection of the opaque
`RepositoryPackageSourceError`. Stage 5 owns the source diagnostic boundary;
fixture and Stage 6 semantics remain unchanged. Use
[configured-cli-fixture.md](./configured-cli-fixture.md) for the preserved F1/F2
checkpoint and the exact F3 blocker receipt.

Slug-native: bounded borrowed diagnostic rendering and its poison/overflow
behavior. Exact/deferred: upstream semantics, new source producers, toolchain
registration behavior, and any later payload demand exposed by the causal error.

## Scope

Allowed changes:

- `app/slug_bzlmod_v2/src/host_package.rs` and a focused diagnostic module/test
  for the opaque source-error projection;
- the existing repository-source observation diagnostic only to reuse its
  bounded `RepositorySourceFileError` rendering without changing semantics;
- `app/slug_loading_v2/src/bzl_module.rs` and focused registration tests only to
  consume the new borrowed source projection;
- this manifest, canonical status, and the configured-fixture gate ledger at a
  genuine acceptance or newly named blocker.

Start with these evidence handles:

- `app/slug_bzlmod_v2/src/host_package.rs`: private source-error variants and
  exact source selection/read stages;
- `app/slug_bzlmod_v2/src/source_preparation/repository_source_observation/registration_diagnostic.rs`:
  existing bounded request/source observation rendering;
- the accepted `RepositoryPackageLoadDiagnosticLeaf::Source` handoff in
  `app/slug_loading_v2/src/bzl_module.rs`;
- [F3 invocation contract](./configured-cli-fixture.md#f3-invocation-contract):
  rerun the same portable proof after the focused diagnostic checks pass.

No loading, DICE-key, registration, source-policy, toolchain, or R2 semantic
change belongs in this packet. Do not render an unbounded recursive `Display`
chain into the diagnostic. Preserve the existing fixed-capacity ASCII output,
poison resistance, and deterministic incomplete/truncated classifications.

## Work and validation

1. Add the smallest borrowed projection needed for
   `RepositoryPackageSourceError`; render stage identity and bounded safe scalar
   context without cloning, recursive allocation, or semantic evaluation.
2. Reuse the existing bounded source-observation/request rendering for direct
   and observed source failures. Replace only the generic `Source` leaf in the
   accepted canonical-package handoff. Prove natural errors are causal,
   poison-free, deterministic, and bounded.
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

The bounded canonical-package projection passed its 11-test diagnostic slice and
the same fresh-root proof changed from `[diagnostic incomplete:
CanonicalPackage]` to `CanonicalPackage: Source` in 5.88 seconds. It selected and
executed one test, retained valid observer/cleanup evidence, and failed native
publication after selecting `rules_cc@0.2.17`. The projection is accepted; F3
remains blocked because the opaque source error does not yet name its input or
producer.

R2 snapshot: branch `review/output-conflict-r2`, commit `27e9e9c0c`,
base `97dffd5d4`, original patch SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`.
`git show 27e9e9c0c:review-evidence/validation.txt` retrieves the original receipt.
The snapshot must incorporate relevant landed prerequisites before new evidence;
its preservation metadata never enters production integration.
