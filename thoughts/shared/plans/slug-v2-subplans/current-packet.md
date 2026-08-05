# Current Slug V2 Packet

Packet: `WP-6-m2-pure-native-family-byte-contract-ledger`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: freeze only the source-complete converter/value/equality/Java-byte family
rules before remapping 287 descriptors or attempting Rust.
Predecessor: the combined 287-row source-closure ledger reached `REPLAN` after
its permitted correction still omitted exact renderer owners/bytes and concrete
default discriminators; the unaccepted Stage 6 diff was discarded cleanly.

Using pinned Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`,
add a Stage 6 family contract for the six built-in type families and every
explicit pure converter. Do not add descriptor rows in this packet.

For each family record:

- exact converter, retained value/type, and Java renderer owner paths/lines,
  or the named versioned Java SE 21 API anchor for a JDK-owned renderer;
- complete explicit input grammar, ASCII/case/radix rules, aliases, and
  rejection boundary;
- retained semantic kind and structural equality, including typed enum kind
  plus member/parameter rather than rendered-string identity;
- exact Java `value.toString()` text and exact outer `OptionsBase.mapToCacheKey`
  bytes for concrete accepted/rejected discriminators; and
- special-null/literal/repeat default interaction, while reserving descriptor-
  specific default mapping for the later 287-row packet.

The contract must cover Boolean, Integer, raw String, TriState, absent-only
Void, comma list/set, assignment, EnvVar records, DottedVersion, empty list,
fission, TestTimeout/EnumMap/Duration, open `PlatformType`, nominal/forced
sharding, and all fifteen finite enum kinds/splits. It must spell exact bytes
for `Set[name=N, value=V]`, `Inherit[name=N]`, `Unset[name=N]`; default and
mixed-unit timeout maps; an executable valid-Unicode supplementary-versus-BMP
UTF-16 sort case; ordinary uppercase enums versus lowercase overrides; full
BoolOrEnum aliases; and every finite member/output.

Each finite enum entry must name all three chains: exact converter class/owner
(shared or custom), typed value declaration/identity, and renderer (the
versioned Java SE 21 `java.lang.Enum#toString` contract for ordinary names or
the exact override). Sharding must name converter, non-forced
enum, forced value/renderer, and Integer.decode. DottedVersion must name its
converter, grammar/value/equality, and raw-string renderer. Duration must name
the versioned Oracle Java SE 21 API contract for
`java.time.Duration#toString`,
<https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/time/Duration.html#toString()>,
which specifies `PTnHnMnS` and zero-section omission rather than infer raw
seconds. Ordinary enum rendering uses the corresponding versioned API anchor,
<https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/Enum.html#toString()>.

Define concrete discriminator IDs for every family and for default routes:
special-null absence→`NULL`, repeatable special-null→`EMPTY`, and literal
conversion→the family’s exact quoted/escaped text. Use unambiguous escaped
Unicode scalars/code units, not labels such as `U+10000` standing in for input.

Allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

Cap: 480 formatted net documentation lines total, at most 380 in the Stage 6
family section. No descriptor-row ledger, Rust, test, fixture, source
generation, Cargo/dependency, oracle, command, DICE, registry, or downstream
change.

Stop and `REPLAN` if any family lacks a pinned converter/value/renderer chain;
descriptors thought to share a family differ semantically; exact bytes require
a live JVM oracle or authority beyond pinned Bazel source and the named
versioned Java SE 21 API contracts; a new representation,
dependency, contextual converter, Java regex, Host/repository/loading access,
command, normalization, checksum, wire, or DICE behavior is needed; or the cap
is exceeded. Preserve 287/8/5/41 and defer configured-target cycles by approval.
