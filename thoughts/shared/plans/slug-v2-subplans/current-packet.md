# Current Slug V2 Packet

Packet: `WP-1-6-7A-exec-group-action-owner-context-evidence`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: `01-compliance-oracle-harness.md` and
`06-analysis-toolchains-and-actions.md`
Scheduling base: `35e84646`
Accepted Rust base: `51127df8`
Result: generate the bounded Bazel 9.2 default/named exec-group action-owner
discriminator required before the immutable Rust owner design.

## Exact authority and caps

Write exactly:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`,
2. this manifest,
3. `thoughts/shared/plans/slug-v2-subplans/01-compliance-oracle-harness.md`,
4. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`,
5. `tests/v2_oracle/fixtures/exec-groups-action-platform/fixture.toml`,
6. its `workspace/MODULE.bazel`,
7. its `workspace/BUILD.bazel`,
8. its `workspace/defs.bzl`, and
9. its `expected/oracle.json`.

Caps against `35e84646`: canonical <=40 net/1,694 physical, this manifest
<=180/287, Stage 1 <=120/1,532, Stage 6 <=160/7,679, the four authored fixture
files <=200 net/248 physical combined, generated expected output <=450/456,
and aggregate <=1,150 semantic/11,896 physical. No tenth file.

## Accepted owner audit

M1 is accepted and no lower DICE/publication prerequisite remains. The first
M7A gate is immutable creation-time action ownership, but live Slug retains
plain `ActionSpec` separately from `ToolchainTopology`. `ActionSpec` has
intrinsic action fields plus an optional exec-group string but no configured
owner/configuration, selected platform/toolchain or aspect provenance.
`configured_file_write_actions` later reconstructs one topology-derived
platform, exposes only `ConfiguredActionExecGroup::Default`, and rejects named
groups and per-action execution fields. FileWrite identity, aquery and REAPI
therefore consume a borrowed reconstruction rather than one retained row.

The future natural owner is an analysis-owned immutable configured-action row,
not a configured identity embedded into build-API `ActionSpec` and not another
DICE key. Existing evidence is insufficient: `actions-api-basic` proves action
kind summaries, `toolchain-resolution-first-platform` proves an actions-free
selection, and `exec-groups-action-platform` is an ungenerated one-action
scaffold whose expected record contains no commands.

## Exact Bazel 9.2 evidence contract

Keep the existing fixture identity and replace the scaffold with one small
workspace in which a single configured rule owner registers two actions:

1. one action that omits Starlark's `exec_group` argument but retains explicit
   default-group identity in the evidence model; and
2. one named `compile`-group action.

Give the actions distinct fixed mnemonics and outputs. The rule declares a
default toolchain type and a different named-group type. Register one default
platform and two compatible named-group platforms with distinct constraint/
property markers; ordered registration selects named platform A cold, then a
literal MODULE mutation selects B, then restoration selects A again. Keep the
default action on its distinct platform throughout. The source must not infer
platform identity from output paths or command completion order.

Use pinned Bazel 9.2.0 from commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. Add immutable provenance anchors
for rule/aspect action ownership, `RuleContext#getActionOwner(execGroup)`,
default/named exec-group toolchain contexts, execution-platform selection and
exec-property merge. The translation note must state that Bazel constructs a
per-group action owner from configuration, aspect descriptors, properties and
the selected platform; Slug's later representation remains Rust-native.

Run an aquery representation that exposes both owner/action rows, mnemonics,
outputs, selected execution platforms, execution-info fields and opaque
ActionKey tokens. Bazel 9.2's `analysis_v2.Action` does not serialize the
owner's merged exec-properties map, so do not claim it does. Source-pin the
property merge and prove it indirectly: edit and restore one property on the
same selected platform and require the affected action's opaque ActionKey to
change and restore while the other action stays fixed. Do not compare or infer
the ActionKey bytes themselves; exact bytes remain M9. Textproto or jsonproto
is allowed only with deterministic fixture-owned normalization already
supported by the harness. A summary alone is forbidden. Pin exact action order,
the property A -> B -> A sequence, and selected-platform A -> B -> A
restoration. Add a build row only if it discriminates the same owner facts or
output bytes without host shell behavior.

Aspect provenance is source-pinned but no applied-aspect action is admitted by
this bootstrap evidence packet. The future owner row must represent explicit
absent provenance; applied aspects remain unsupported until a separate exact
fixture is accepted. Likewise do not widen this packet into rules_rust,
additional action kinds, backend execution or M9 identity evidence.

## Validation and lifecycle

Generate expected output once with the pinned Bazel binary, then replay the
fixture clean. Verify the expected record is marked generated, contains every
declared command, distinguishes both actions and all A/B/A rows, and contains
no credentials, machine paths, unstable durations, invocation URLs or raw
output-base material. Run the fixture manifest/schema checks and diff hygiene.
Clean all temporary output bases/processes.

No Slug command is required: this is just-in-time Bazel evidence for the next
Rust design. Do not reinterpret a missing Slug implementation as oracle
failure and do not update any unrelated expected record.

## Compatibility and terminal

Exact: Bazel 9.2 owner/action order, default/named group selection, selected
platform/toolchain/property semantics and the admitted normalized aquery facts.

Slug-native: the future compact immutable Rust owner row and collision-safe
configuration/action identity. Bazel checksum/ActionKey bytes remain M9.

Unsupported/deferred: applied-aspect actions, unrelated action kinds/rulesets,
M7B commands, broader REAPI/backend behavior and exact identity bytes.

End with exactly one result:

1. accept the generated evidence and schedule one docs-only immutable
   action-owner-context design;
2. identify one uniquely smaller source/normalization prerequisite; or
3. formal `REPLAN` if the required distinctions cannot be generated within
   the fixture/caps.

At most one successor. STOP Rust/Cargo, another fixture, harness changes,
summary-only evidence, inferred fields, nondeterministic normalization,
credentials/network metadata, direct owner implementation, M8, M7B, M9, JVM
production code, cap excess, partial validation or multiple successors.
