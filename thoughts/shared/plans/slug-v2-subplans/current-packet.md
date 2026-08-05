# Current Slug V2 Packet

Packet: `WP-6-m2-root-configuration-identity-design`
Milestone: M2 configuration identity prerequisite for the first M4 cquery consumer
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: design-only authoritative root target configuration boundary
Evidence: accepted recursive configured analysis; retained Bazel 9.2 default,
explicit-label, missing-target, and same-server recovery evidence; pinned Bazel
9.2 `BuildOptions` checksum ownership; two independent live-code REPLAN audits.

Do not edit Rust, tests, fixtures, generated oracle records, wire schema, or
harness code. Audit the current `ConfigurationKey`, its two `first-build`
production constructors, command input normalization, configured-analysis key
identity, and retained transaction input ownership. Reconcile them with pinned
Bazel 9.2 `BuildOptions.checksum()`/`shortId()` semantics.

The design must answer all of these before any implementation packet:

1. What bounded V2-owned value represents every option input needed for the
   accepted no-extra-flags root target configuration, without embedding the
   observed `a7a71fd` digest or delegating to Bazel/Java?
2. Which DICE input/key owns that value, and which command-line, Starlark build
   setting, scope, platform/toolchain, repository-mapping, and environment
   changes must invalidate it now versus remain explicit unsupported inputs?
3. Can Bazel 9.2 native fragment `cacheKey()` ordering/defaults be reproduced
   exactly within a bounded Rust packet, or does even the default fixture
   require a broader configuration-model prerequisite?
4. How does the authoritative checksum replace `first-build` in both existing
   entry points while preserving configured-target equality, recursive
   dependency reuse, and no lock across DICE computation?
5. Which focused checksum discriminator and retained lifecycle tests would
   prove identity changes, equality restoration, and cquery short-ID output?

Preserve the accepted later route: cquery will drive the existing
`RootConfiguredTargetAnalysisKey` through `NativeCommandRoot`, format its
accepted result, and perform no second analysis/evaluator call. A dedicated
daemon cquery request is a later public schema packet, not part of this design.

Return `ACCEPT` only with a complete, exact, line-bounded producer/consumer
contract and file allowlist. Return `REPLAN` if exact default configuration
identity needs the unmodeled general Bazel option universe. Never truncate
`first-build`, substitute a mnemonic, hard-code oracle bytes, or weaken the
accepted default/explicit-label contract.

Stops: no code or fixture change; no new oracle command without a reviewed
evidence successor; no transition, toolchain/platform, repository-mapping,
provider, aquery, action, execution, REAPI, or cycle implementation; no
credential inspection or Bazel delegation.
