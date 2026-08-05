# Current Slug V2 Packet

Packet: `WP-6-m2-forced-sharding-identity-evidence`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: decide whether forced-sharding Java object identity reaches semantic
configuration/incremental identity or is canonicalized away before that point.
Predecessor: the family-contract retry reached `REPLAN` because each
`forced=N` conversion allocates a Java object without `equals`/`hashCode`,
contradicting the assumed structural native `Forced(i32)` equality. Its
unaccepted ledger was discarded.

Trace this bounded chain at Bazel commit
`8220c6198837d5c13d53fea211cf3282aa12408a`:

- `TestShardingStrategy.ShardingStrategyConverter` and
  `TestShardingStrategyForced`: fresh allocation, count, text, and missing
  equality/hash overrides;
- `OptionsBase.equals`, `hashCode`, and `cacheKey` field semantics;
- `FragmentOptions.clone` and any option-field cloning or reconstruction;
- `BuildOptions` construction, clone, equality/hash, checksum, and
  configuration identity;
- configured-target key consumers of that identity; and
- the corresponding live Slug configuration/root-key and daemon-replay owners.

Freeze one exact question: for identical `forced=0` source text, are two fresh
parses unequal while a clone preserves reference equality; do their object
hashes, cache keys, BuildOptions checksums/configuration IDs, or configured keys
differ; and can any difference affect warm daemon reuse?

Pinned source is primary. If it cannot close parse/clone/hash/checksum behavior,
one nonpersistent temporary Bazel-classpath Java probe may compare two fresh
parses and a clone, including field identity/equality, object hashes, cache
keys, and configuration identity under Bazel 9.2's exact runtime. Record exact
commands/output and delete every source/class/artifact. No broad command oracle,
persistent fixture, JVM dependency, or Java/Bazel delegation is Slug
architecture.

Because this concerns retained identity and incremental keys, follow the
Buck2-utility reuse boundary: do not choose a global counter, interner, mutable
singleton, `Arc` pointer identity, custom hash map, `Dupe` wrapper, or hidden
DICE state without exact lifecycle evidence. This packet designs no Rust and
does not edit the Stage 9 reuse ledger. Any later retained representation must
reuse existing compact/shared utilities where appropriate and preserve
`Allocative` before implementation review.

Allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

Cap: 220 formatted net documentation lines total, at most 150 in Stage 6. No
family/descriptor ledger reconstruction, Rust, production/protected fixture,
dependency, registry, command, DICE, or downstream change.

Stop and `REPLAN` if identity survives into BuildOptions/configured-key/DICE
semantics and needs scoped ownership, allocator/interner/cache design; source,
probe, and live Slug lifecycle disagree; clone/replay scope remains ambiguous;
another material identity distinction appears; or scope/cap is exceeded.

On acceptance record exactly one outcome: if identity is discarded or
canonicalized before semantic configuration identity, resume
`WP-6-m2-pure-native-family-byte-contract-ledger-retry` with structural
`Forced(i32)` and the evidence citation. If identity survives, advance only to
a forced-sharding retained-identity representation design. Preserve 287/8/5/41
and defer contextual/regex/Host/repository conversion, normalization,
checksum/wire implementation, DICE changes, and configured-target cycles.
