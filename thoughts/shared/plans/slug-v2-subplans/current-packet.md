# Current Slug V2 Packet

Packet: `WP-6-m2-pure-native-converter-source-closure-ledger`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: close the pinned-source semantic, equality, and Java-rendering contract for
all 287 pure native descriptors before any third Rust implementation attempt.
Predecessor: the clean retry stopped before validation when ordinary enum-name
rendering disproved its generic lowercase enum value; all unaccepted Rust was
discarded and HEAD remains the accepted metadata plus REPLAN documentation.

Using pinned Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`,
extend the Stage 6 owner plan with:

- one compact row for each of the 287 pure descriptors, reusing the existing
  inventory ordinal/FQCN/name/type/converter rather than duplicating metadata;
- shared source-backed family rules for the six built-in field-type families
  and every explicit pure converter identifier; and
- a complete mapping from admitted input to retained semantic value/equality
  and exact Java `value.toString()` text before the accepted outer cache-field
  grammar.

Each descriptor row must record its family-rule ID, default route (special
`"null"`, literal conversion, repeat wrapping, or isolated positive runs seed),
explicit input grammar/rejection boundary, case rule, converter aliases (never
command option aliases), retained Java/Rust semantic kind, structural equality,
exact rendering rule, pinned source owner/line, and discriminator IDs. Family
rules must enumerate every finite input/member/output spelling and all aliases.

The ledger must preserve `287 pure / 8 Java-regex / 5 Host / 41 repository =
341`, plus the orthogonal 45 repeat/13 old-name/6 expansion/2 implicit command
metadata. It must prescribe typed enum identity (enum kind plus member or
parameterized value), never cache text as structural identity, and similarly
identify every non-enum structural value.

At minimum freeze later-test discriminators for:

- ordinary EnumConverter mixed-case input to uppercase enum-name text, custom
  lowercase `CompilationMode`/`StripMode`, typed cross-kind inequality,
  BoolOrEnum aliases, and nominal versus `forced=N` sharding;
- Boolean/TriState synonyms, Integer.decode sign/radix/bounds, raw text,
  comma-list empty/interior-empty, Java UTF-16 ordered-set sort/dedup,
  assignment/env, fission, timeout/Duration, and DottedVersion;
- special-null/repeat/default wrapping, positive/private runs seed, absent-only
  Void, `NULL`/`EMPTY`, escaping, every descriptor default cache text, one
  admitted nondefault per family, and every finite enum member/alias/output.

Pinned cross-cutting owners: `common/options/{Converters,EnumConverter,
BoolOrEnumConverter,Option,OptionDefinition,FieldOptionDefinition,OptionsBase}
.java`. Pure owners include `analysis/config/{CompilationMode,CoreOptions,
CoreOptionConverters}.java`, `analysis/test/{TestConfiguration,
TestShardingStrategy,TestShardingStrategyNotForced,TestShardingStrategyForced}
.java`, `packages/TestTimeout.java`, `rules/android/AndroidConfiguration.java`,
`rules/apple/{AppleCommandLineOptions,AppleConfiguration,DottedVersion,
DottedVersionConverter}.java`, `rules/cpp/{CppOptions,CppConfiguration}.java`,
and `rules/java/{JavaConfiguration,JavaOptions}.java`. Every row must name its
exact declaring/converter/value owner file and line; no grouped “relevant file”
citation is acceptable when another exact owner is discovered.

Allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

Cap: 1,050 formatted net documentation lines total, at most 900 in the Stage 6
ledger. No Rust, tests, fixtures, source generation, Cargo/dependency, oracle,
command, DICE, registry, or downstream change.

Stop and `REPLAN` if any pure row lacks a complete pinned converter/value/
`toString()` chain; descriptors sharing a proposed family differ semantically;
a row requires a new representation, dependency, contextual converter, Java
regex, Host/repository/loading access, or live oracle; counts change; command,
normalization, checksum, wire, or DICE behavior becomes necessary; or the cap
is exceeded. Configured-target dependency cycles remain deferred by approval.
