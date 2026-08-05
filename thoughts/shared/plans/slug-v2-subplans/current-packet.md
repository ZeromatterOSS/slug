# Current Slug V2 Packet

Packet: `WP-6-m2-host-and-repository-conversion-context-design`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: docs-only design for the smallest immutable conversion-context boundary
needed by the deferred five Host and 41 repository/package/loading native
descriptors.

Predecessors are authoritative: the committed 341-row registry, exact
`287 + 8 + 5 + 41` cohort, accepted converter/default/rendering/source ledgers,
287-row attachment ledger, and private pure kernel in `e7067bfc`. Reuse their
descriptor classifications and source anchors. Add no Rust, fixture, generated
data, or duplicate oracle probe.

The design must inventory each of the 46 contextual descriptors and assign its
inputs, owner, retained value, and conversion phase. Keep these boundaries
explicit:

- the Host cohort is exactly `cpu` and `host_cpu` through
  `AutoCpuConverter`, `shell_executable` through `PathFragmentConverter`,
  `platform_mappings` through the explicit-path branch of
  `PlatformMappingKeyConverter`, and repeatable `default_test_resources`
  through `TestResourcesConverter`;
- the repository/package/loading cohort is exactly the accepted 41 label and
  conditional-label descriptors: `LabelConverter` 16, `LabelListConverter` 6,
  `LabelOrderedSetConverter` 1, `LabelMapConverter` 1,
  `LabelToStringEntryConverter` 1, `EmptyToNullLabelConverter` 5,
  `CoreOptionConverters.LabelConverter` 2,
  `CoreOptionConverters.EmptyToNullLabelConverter` 3,
  `HostPlatformConverter` 1, `LibcTopLabelConverter` 2,
  `RunUnderConverter` 1, `CustomFlagConverter` 1, and
  `FlagAliasConverter` 1; and
- label conversion must distinguish `Label.PackageContext`,
  `RepositoryMapping`, the first-round null context, and the six symbolic
  source defaults rather than treating source text as a context-free label.

Specify one smallest immutable context value or a justified closed split. For
every field, name the existing layer that observes and injects it, its valid
Unicode/path/label representation, structural equality requirements, and why
configuration conversion may consume it without filesystem or graph access.
Apply the Buck2 utility-reuse boundary to retained strings, compact collections,
clone cost, and allocation accounting. Do not select DICE keys or runtime
storage in this packet.

Preserve Bazel's convert-before-normalize ordering. Whole P/C/T normalization
waits until every fragment member is typed; label-list truncation or flag-alias
deduplication may not hide an invalid contextual value. Command flattening,
old names, boolean negation, repeats, expansions, and implicit requirements
remain owned by `slug_commands_v2`. Route the conditional non-label branches
explicitly, but keep their implementation outside a Host/repository packet
unless this design proves their typed value ownership.

Allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- routing only on `REPLAN`:
  `.codex/skills/slug-agent-orchestration/references/routing-log.md` and
  `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

Cap: 700 formatted documentation lines total. No Rust, test, registry, fixture,
dependency, lockfile, oracle, generated data, command/wire, DICE, checksum,
consumer, or downstream activation edit.

Acceptance requires a descriptor-complete `5 + 41` routing table, explicit
ownership for Host snapshot, path policy and user-home input, test-resource
Host facts, package context, repository mapping, first-round null context, and
symbolic defaults; a closed immutable context representation with structural
identity and injection direction; and one bounded implementation sequence or
`REPLAN`. Keep the eight Java-regex descriptors, pure-kernel activation,
command identity, checksum/wire, full-fragment normalization, and configured
target construction out of scope.

Stop and `REPLAN` on unresolved host path policy or `user.home` behavior,
repository-mapping ownership, symbolic-default interpretation, Java-regex or
lone-surrogate behavior, a need for IO/loading during conversion, a new DICE
key, or any route that requires creating a configured target. Configured-target
dependency cycles remain explicitly deferred by user approval; this packet
must create no target/configuration edge, dependency path, or cycle behavior.
