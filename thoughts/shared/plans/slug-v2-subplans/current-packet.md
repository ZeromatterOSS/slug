# Current Slug V2 Packet

Packet: `WP-6-m2-general-target-configuration-input-chain-design`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: design the first serial identity prerequisite without implementing it.
Predecessor: accepted action-query identity evidence `f00e99db`, internal
string-setting configurations `dfc1705e`, and root build/cquery consumers that
still construct `target:first-build`.

This is documentation/source design only. Determine whether an exact shared
Bazel 9 target-configuration input and identity chain can be bounded across:

- typed command normalization for every configuration-affecting native option;
- canonical Starlark build-setting values and option scopes;
- CPU/host CPU, target/host platform, platform mapping, and selected flags;
- one daemon request identity matching the one-shot command semantics;
- one DICE-owned transactional producer computed before build or cquery root
  keys are constructed; and
- authoritative `ConfigurationKey` equality, serialization, checksum, and
  recursive transition behavior.

Enumerate all fourteen Bazel 9.2 `FragmentOptions` classes, their default
cache-key inputs and ordering, plus every non-native input to
`BuildOptions#checksum`. Map each input to a future command, environment, Host
observation, or graph owner; explicitly reject any unmodeled input before
analysis. Freeze the Need/error/invalidation/equality and same-daemon
`C0 -> C1 -> C0` contract. Decide whether one implementation packet is
truthful and bounded; otherwise return `REPLAN` with exact serial prerequisite
ledgers.

Potential future owners to adjudicate, not edit:

- `app/slug_commands_v2/src/{common,build,cquery}.rs`;
- `app/slug_server_v2/src/server.rs` and matching protocol request identity;
- `app/slug_core_v2/src/runtime/{mod,dice}.rs`; and
- `app/slug_analysis_v2/src/key.rs`.

Documentation allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- scheduling synchronization in this manifest and the canonical plan
- one terminal routing-log row after review

Cap the design record at 380 formatted documentation lines. No production,
test, fixture, oracle, dependency, generated-file, command-wire, or DICE
change is authorized.

Stop on a partial or fixture-specific option inventory; hard-coded/truncated
checksums; exposing `first-build`; ignoring defaults, host inputs, platform
mappings, Starlark values, or scopes; borrowing Bazel/Java at runtime; a new
key/cache/global/lock/interner before complete ownership is frozen; configured
artifact paths; execution-platform or ActionKey representation; cquery/aquery
formatting; REAPI/execution/cache/materialization; V1/Buck configuration
semantics; or any claim beyond pinned Bazel 9.2 source and accepted evidence.
