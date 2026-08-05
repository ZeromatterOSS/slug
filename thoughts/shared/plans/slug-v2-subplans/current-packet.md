# Current Slug V2 Packet

Packet: `WP-6-m2-native-value-cohort-and-rendering-design`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: freeze the exact pure native-value boundary before executing converters
or retaining dynamic configuration values.
Predecessor: accepted metadata/cache grammar `b043d54d` and the complete Bazel
9.2 target-configuration input ledger `0887d2b2`.

This is pinned-source/documentation design only. It must:

- route all 341 descriptors into disjoint converter cohorts and reconcile the
  audited 288 pure / 7 unsupported Java-regex / 5 Host-dependent / 41
  repository-package-loading split, recording every exception;
- separately inventory the command-owned occurrence metadata—45 repeatable,
  13 old-name, six expansion, and two implicit-requirement rows—and freeze that
  `slug_configuration_v2` consumes only command-flattened ordered occurrences;
  it never expands RC/`--config`, old names, boolean negation, repeats,
  expansions, implicit requirements, or aliases;
- define special annotation `"null"` behavior, repeatable empty defaults,
  `runs_per_test`'s converted default, and the six symbolic label defaults;
- define a closed structural value algebra and exact Java `value.toString()`
  projection for every admitted pure family, including list/entry/map/env/enum/
  duration behavior, `NULL` versus empty-list `EMPTY`, escaping, equality, and
  deterministic Java UTF-16 ordering; and
- separate pure P/C/T members from label-bearing `platforms` and `flag_alias`
  while preserving Bazel's convert-every-occurrence-before-normalize errors.

Apply `.codex/skills/slug-buck2-utility-reuse/SKILL.md`. Decide, with source
and size evidence, whether dynamic values use `CompactString`, immutable
`Arc<[T]>`, `Dupe`, and `Allocative`; prohibit a runtime descriptor map, global
interner, cache, weak identity hash, accidental deep cloning, or Rust-derived
`Debug`/`Display` cache bytes.

Documentation allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- scheduling synchronization in this manifest and the canonical plan
- one terminal routing row and required bounded-history rotation

Cap the design at 720 formatted documentation lines. No Rust, tests, fixtures,
oracle, dependency, generated data, command/wire, DICE, checksum, analysis,
configured-path/platform/ActionKey/aquery, execution, or lockfile change is
authorized.

Stop on unresolved Java regex generation/rendering; a silent Rust UTF-8/lone-
surrogate substitution; any converter whose Host or repository context is
ambiguous; partial normalization that can hide an invalid later occurrence;
generic map/record rendering without source-backed order; configuration-owned
argv expansion/repeat/alias logic; an incomplete cohort; retained-state
ownership ambiguity; or cap breach. Configured-target dependency cycles remain
deferred with user approval.
