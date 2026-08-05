# Current Slug V2 Packet

Packet: `WP-6-m2-java-guava-renderer-authority-evidence-retry`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: retry only the exact Java/JDK/Guava renderer authority ledger with a
genuinely discriminating Java UTF-16 ordering probe.
Predecessor: the first renderer ledger reached terminal `REPLAN` after its
correction probe inserted U+10000 before U+E000, already Java UTF-16 order, so
it did not prove the claimed distinct-then-sort path. Its unaccepted Stage 6
diff and all temporary artifacts were discarded.

Rebuild the compact renderer authority matrix using:

- Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a` and Bazel 9.2's
  actual runtime from `bazel info java-runtime` and `java-home`;
- versioned Java SE API/spec contracts for Boolean, Integer, String, Enum,
  Duration, AbstractCollection/List, AbstractMap/EnumMap, records, String
  UTF-16 comparison, and outer string conversion;
- Bazel's pinned `com.google.guava:guava:33.5.0-jre`, lockfile JAR SHA-256
  `1e301f0c52ac248b0b14fdc3d12283c77252d4d6f48521d572e7d8c4c2cc4ac7`,
  and official Guava `v33.5.0` source tag for `ImmutableList`,
  `Maps.immutableEntry`, and their concrete renderers; and
- pinned Bazel overrides for CompilationMode, StripMode, PlatformType, and
  lowercase `TestTimeout.toString()` keys.

One nonpersistent temporary Java probe may compile same-shaped records and
standard JDK collection/map/duration/enum cases. Compile with a recorded
compatible compiler and execute only with Bazel 9.2's exact runtime. Record
commands, versions, stdout bytes, source/hash bindings, and cleanup. This is
evidence only: no persistent source/class/JAR, JVM dependency, or Java/Bazel
delegation is Slug architecture.

The UTF-16 discriminator is frozen: construct actual scalar strings in reverse
UTF-16 order, U+E000 first and U+10000 second; pass them through the
production-equivalent `distinct().sorted()` or identical natural-order path;
then prove output code units `D800 DC00,E000` and exact bracketed cache bytes
`x="[𐀀, ]", ` (space after the comma). An already ordered input,
joined-string shortcut, or list without the sort path is not evidence.

The matrix must also close exact authority and bytes for empty,
singleton-empty, and multi-element lists; embedded-`=` immutable entries; all
three EnvVar records; Boolean/Integer/String and escaping; ordinary and
overridden enums; lowercase TestTimeout EnumMap defaults and mixed durations;
null/empty handling; and every concrete renderer owner.

Allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

Cap: 300 formatted net documentation lines total, at most 240 in Stage 6. No
descriptor rows, family conversion grammar, Rust, persistent fixture,
generated artifact, dependency, registry, command, DICE, or downstream change.

Stop and `REPLAN` on source/JAR/runtime disagreement, a non-discriminating
UTF-16 path, retained Java material, production JVM implication, any second
correction, or scope/cap breach. Preserve 287/8/5/41 and defer contextual and
regex conversion, normalization, checksums, wire, and configured-target cycles.
