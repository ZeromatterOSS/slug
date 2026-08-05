# Current Slug V2 Packet

Packet: `WP-6-m2-implicit-default-info-provider-oracle`
Milestone: M2 configured Starlark provider normalization
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: isolated Bazel 9.2 positive oracle
Predecessor: clean `REPLAN` of the string build-setting transition
implementation at `7d39c759`; accepted transition fixture `b12774b9`.

Create one isolated fixture proving Bazel 9.2's successful configured-target
semantics when a Starlark rule omits `DefaultInfo`. Add exactly six regular
files and zero links under
`tests/v2_oracle/fixtures/implicit-default-info-provider/`: `fixture.toml`,
generated `expected/oracle.json`, and workspace `MODULE.bazel`, `BUILD.bazel`,
`defs.bzl`, and `cquery_format.bzl`. Do not edit any existing fixture, harness,
Rust, Cargo, command, or plan file in the evidence worker.

The workspace contains:

1. one rule returning only `CustomInfo(value = "implicit")`;
2. one rule returning `CustomInfo(value = "explicit")` and `DefaultInfo()`;
3. one consumer that indexes both dependencies by the exported `CustomInfo`
   and `DefaultInfo` constructors and returns a custom summary provider only;
   and
4. a formatter that reads only the exact known `providers(target)` keys and
   emits stable canonical label, named custom value, `DefaultInfo` presence,
   and `DefaultInfo.files` length. It must not enumerate provider keys.

Record exactly four successful commands against one retained Bazel 9.2 server:

1. the implicit target exposes custom value `implicit`, a present
   `DefaultInfo`, and zero files;
2. the explicit-empty target exposes custom value `explicit`, a present
   `DefaultInfo`, and zero files;
3. the consumer proves both custom values and successful `DefaultInfo`
   indexing with zero files on both dependencies; and
4. an unchanged warm consumer replay is byte-identical.

Pin Bazel 9.2 source anchors for `StarlarkRuleConfiguredTargetUtil` return
decoding and implicit empty default creation, `DefaultInfo`,
`AbstractConfiguredTarget` indexing/membership/query dictionary synthesis,
`StarlarkProvider.Key`, `StarlarkOutputFormatterCallback#providers`, and the
configured-query provider integration test. Generate and replay without
updates using `/usr/bin/bazel` 9.2. Run fixture list, JSON, inventory/cap,
provenance, credential-pattern, archive, and diff checks and obtain independent
fixture review.

Caps are six regular files, zero links, 220 authored non-generated lines, 420
generated total lines, and four commands. This is one demonstrated provider-
normalization prerequisite, not new fixture breadth requiring a growth
checkpoint.

Stop if output exposes a configuration ID or configured path, platform,
action key, mnemonic, or other unstable identity; if named semantic fields
cannot prove implicit and explicit-empty `DefaultInfo`; or if the fixture needs
provider-key enumeration, outputs/runfiles/executable behavior, builtin-only
or non-Starlark-convertible providers, aliases, aspects, output groups,
missing/duplicate/unnamed/wrong returns, any failure diagnostic, Rust, public
cquery support, execution, or harness changes.

After accepted evidence, design the exact analysis decoder normalization owner
before resuming the transition implementation. Do not choose permissive
configured-target absence or synthesize a provider in Rust in this packet.
