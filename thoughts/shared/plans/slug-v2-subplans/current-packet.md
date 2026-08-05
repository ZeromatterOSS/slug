# Current Slug V2 Packet

Packet: `WP-6-m2-java-guava-renderer-authority-evidence`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: close only the exact Java/JDK/Guava renderer authority used by pure native
values before retrying the family byte contract.
Predecessor: the family-byte draft reached `REPLAN` because standard-library and
Guava renderer chains were still incomplete and pinned source corrected timeout
map keys from uppercase to lowercase; its unaccepted Stage 6 diff was discarded.

Record a compact renderer authority matrix using:

- Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a` and Bazel 9.2's
  actual server runtime from `bazel info java-runtime`;
- the versioned Java SE API/spec contracts for Boolean, Integer, String,
  Enum, Duration, AbstractCollection/List, AbstractMap/EnumMap, records, and
  outer string conversion;
- Bazel's pinned `com.google.guava:guava:33.5.0-jre` coordinate and
  `maven_install.json` JAR SHA-256
  `1e301f0c52ac248b0b14fdc3d12283c77252d4d6f48521d572e7d8c4c2cc4ac7`,
  bound to the official Guava `v33.5.0` source tag for `ImmutableList`,
  `Maps.immutableEntry`, and their concrete renderers; and
- pinned Bazel overrides, especially `CompilationMode`, `StripMode`,
  `PlatformType`, and `TestTimeout.toString()` lowercase keys.

Run one nonpersistent temporary Java probe with the exact Bazel 9.2 runtime.
It may compile/print same-shaped `Set`, `Inherit`, and `Unset` records and the
standard JDK collection/map/duration/enum cases. Record exact `java -version`,
commands, stdout bytes, and cleanup. It is evidence only: no persistent fixture,
Java source/class, JVM dependency, or delegation is Slug architecture. Guava
outputs must additionally be proved by the cryptographically bound source
chain; download a temporary verified JAR/source only if needed and delete it.

The matrix must close exact renderer/equality authority and bytes for:

- Boolean/Integer/String and outer quote/backslash escaping;
- empty, singleton-empty, and multi-element immutable lists;
- immutable `Map.Entry` with an embedded `=`;
- `Set[name=N, value=V]`, `Inherit[name=N]`, `Unset[name=N]`;
- ordinary enum names and lowercase Bazel overrides;
- lowercase-keyed TestTimeout EnumMap text, including
  `{short=PT1M, moderate=PT5M, long=PT15M, eternal=PT1H}` and a mixed duration;
  and
- valid supplementary-versus-BMP Java UTF-16 ordering using actual runtime
  scalars/code units and exact list bytes.

Allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

Cap: 300 formatted net documentation lines total, at most 240 in Stage 6. No
descriptor rows, family grammar beyond renderer authority, Rust, persistent
test/fixture, generated artifact, dependency, registry, command, DICE, or
downstream change.

Stop and `REPLAN` if the Guava source cannot be bound to the pinned JAR; two
declared Java runtimes disagree; the temporary probe contradicts source/spec;
exact output needs a persistent Java fixture or production JVM dependency; or
scope/cap is exceeded. Preserve 287/8/5/41 and defer contextual conversion,
normalization, checksums, wire, and configured-target cycles.
