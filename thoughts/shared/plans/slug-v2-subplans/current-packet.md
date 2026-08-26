# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-config-string-list-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 complete `config` global and retained rule-definition semantics
Base: `573c25c7`

Result: authenticate the next live rules_rust build-setting descriptor,
`config.string_list`, including its `repeatable` dimension, and select one
bounded definition-loading implementation or `REPLAN`. This packet is
read-only and changes no Rust.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority:

- `StarlarkConfigApi.string_list` declares named-only boolean `flag` and
  `repeatable` parameters, both defaulting to `False`.
- `StarlarkConfig.stringListSetting` rejects `repeatable = True` unless
  `flag = True`, then creates a `STRING_LIST` descriptor retaining
  repeatability.
- `RuleClass.Builder` derives mandatory, nonconfigurable
  `build_setting_default` from the descriptor type. Existing config tests
  authenticate list defaults, repeatable command-line accumulation and the
  repeatable-without-flag error; target/CLI semantics are not admitted here.
- The fixed `config` bootstrap remains `.bzl`-only. BUILD must retain Slug's
  accepted string-only compatibility projection and must not gain
  `string_list`.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
After two `config.bool(flag = True)` definitions, source order reaches
`rust/private/rustc.bzl:3093` and `:3108`, both using
`config.string_list(flag = True)` with nonrepeatable default behavior. Line
3120 is the first `repeatable = True` occurrence, followed by both forms in
the same module and other rules_rust files. The audit must not silently treat
repeatable and nonrepeatable descriptors as identical.

Commit `573c25c7` accepted the preceding boolean definition slice. Slug now
retains `Option<BuildSettingKind>` with String/Boolean variants through rule
definition, freeze, equality and typed default schema; boolean invocation
still fails before target recording. Loading already owns exact string-list
attribute coercion, but the build-setting kind and descriptor do not yet own a
StringList or repeatability fact.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architecture guidance only.
Its complete typed semantics owner and narrow projections require the audit to
choose one retained owner for every admitted repeatability-affecting fact,
rather than adding evaluator-local or analysis-side markers. Copy no Zabel
code, representation, scheduler, runtime or behavior; Bazel remains the sole
compatibility authority.

The Buck2 utility-reuse skill remains applicable if the selected packet
changes retained descriptor data. The audit must decide whether the existing
small enum plus a compact repeatability fact is sufficient or whether a
reviewed Buck2-derived utility is warranted; it may not introduce a map,
interner, hash domain or allocation without evidence.

## Audit questions and decision rule

Trace pinned Bazel source/tests from `string_list` construction through
`BuildSetting`, `RuleClass.Builder`, default coercion, equality and the first
analysis/transition consumers. Inventory every rules_rust occurrence in
source order and separate the first nonrepeatable declaration from the later
repeatable family.

Inspect Slug's complete config-global owner, `BuildSettingKind`, builtin
schema, raw/coerced list attribute representation, freeze/equality path,
string-only analysis accessor and boolean fail-closed invocation boundary.
Determine the smallest packet that can:

- expose only the authenticated `.bzl` call shape;
- retain all admitted type/repeatability facts structurally;
- select a list-typed `build_setting_default` schema;
- keep BUILD and existing string/bool behavior unchanged; and
- fail before target recording for every unimplemented list-setting target.

Prefer a definition-only slice for the first nonrepeatable rules_rust use if
it remains honest and does not collapse a later repeatable identity. Otherwise
select one bounded descriptor-identity prerequisite or write `REPLAN`.

Do not implement list-setting target invocation, analysis, CLI parsing,
transitions, `config_setting`, `ctx.build_setting_value`, later config
families, toolchains or actions. Do not infer repeatability from a default
value and do not reuse String identity for StringList.

## Files, proof and validation

This audit may edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`.

Record exact base hashes, final line ceilings, addition caps and a bounded
allowlist for any selected implementation. Reuse pinned Bazel tests and the
accepted rules_rust archive; add oracle evidence only for a demonstrated gap.
Record the existing real-smoke public repository-session boundary separately
from the source-order terminal and do not overclaim it as a successful public
load.

Run `git diff --check`, plan/current alignment checks and
`scripts/v2_archive_status.sh`, preserving only its known three-path thoughts
classification if unchanged. No Cargo, Bazel, daemon or network command is
required by this docs-only audit.

## Compatibility and STOP

- **Exact:** only the already accepted `.bzl` string/bool descriptor slices;
  the audit may authenticate exact Bazel `string_list` call shape, identity and
  list-default rules but cannot claim them implemented.
- **Slug-native:** prospective Rust representation, valid-Unicode handling and
  nonrequired diagnostics.
- **Unsupported/deferred:** all `config.string_list` behavior until a reviewed
  implementation lands, including repeatable descriptors, targets, analysis,
  CLI values and transitions; every later rules_rust/toolchain/action surface,
  M8/M7B and exact output bytes.

STOP on Rust changes, an implementation packet that omits repeatability from
semantic identity, BUILD-global widening, list-target publication, behavior
claims sourced from Zabel, new oracle work without a demonstrated gap, dirty
overlap, or inability to state one bounded implementation/`REPLAN` with exact
authority and caps.
