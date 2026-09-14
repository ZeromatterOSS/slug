# Current Slug V2 Work Packet

Packet: WP-7A-repository-ignore-diagnostic
Status: ready

## Result and owner

Replace the terminal `error=RepositoryIgnore` for
`@@protobuf+//bazel/private/toolchains/prebuilt` in the authentic configured CLI
proof with a bounded causal projection of `HostRepositoryIgnoreError`. Stage 5
owns the source diagnostic boundary; fixture and Stage 6 semantics remain
unchanged. Use
[configured-cli-fixture.md](./configured-cli-fixture.md) for the preserved F1/F2
checkpoint and the exact F3 blocker receipt.

Slug-native: bounded borrowed diagnostic rendering and its poison/overflow
behavior. Exact/deferred: upstream semantics, new source producers, toolchain
registration behavior, and any later payload demand exposed by the causal error.

## Scope

Allowed changes:

- `app/slug_bzlmod_v2/src/repository_ignore.rs` and a focused diagnostic
  module/test for the typed ignore-error projection;
- existing repository-source observation rendering only to reuse bounded source
  and observation leaves without changing semantics;
- `app/slug_bzlmod_v2/src/host_package_registration_diagnostic.rs` and focused
  tests only to consume the new borrowed ignore projection;
- this manifest, canonical status, and the configured-fixture gate ledger at a
  genuine acceptance or newly named blocker.

Start with these evidence handles:

- `app/slug_bzlmod_v2/src/repository_ignore.rs`: private ignore-error variants
  and routed `.bazelignore` source stages;
- `app/slug_bzlmod_v2/src/source_preparation/repository_source_observation/registration_diagnostic.rs`:
  existing bounded request/source observation rendering;
- the accepted `ExternalRepositoryPackageLookupError::RepositoryIgnore` handoff
  in `app/slug_bzlmod_v2/src/host_package_registration_diagnostic.rs`;
- [F3 invocation contract](./configured-cli-fixture.md#f3-invocation-contract):
  rerun the same portable proof after the focused diagnostic checks pass.

No loading, DICE-key, registration, source-policy, toolchain, or R2 semantic
change belongs in this packet. Do not render an unbounded recursive `Display`
chain into the diagnostic. Preserve the existing fixed-capacity ASCII output,
poison resistance, and deterministic incomplete/truncated classifications.

## Work and validation

1. Add the smallest borrowed projection needed for `HostRepositoryIgnoreError`;
   render variant identity and bounded safe scalar context without cloning,
   recursive allocation, or semantic evaluation.
2. Reuse the existing bounded source-observation/request rendering for routed
   `.bazelignore` failures. Replace only the generic `RepositoryIgnore` leaf in
   the accepted package-source handoff. Prove natural errors are causal,
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

The bounded source projection passed 2 focused package-source tests, 7 existing
request/observation grammar tests, and the 11-test dependent diagnostic slice.
The same F3 run selected/executed one test with valid observer and cleanup
evidence, then reached `CanonicalPackage: Lookup
package=@@protobuf+//bazel/private/toolchains/prebuilt error=RepositoryIgnore`
in 5.89 seconds. The source projection is accepted; F3 remains blocked until the
ignore error names its exact variant and input.

R2 snapshot: branch `review/output-conflict-r2`, commit `27e9e9c0c`,
base `97dffd5d4`, original patch SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`.
`git show 27e9e9c0c:review-evidence/validation.txt` retrieves the original receipt.
The snapshot must incorporate relevant landed prerequisites before new evidence;
its preservation metadata never enters production integration.
