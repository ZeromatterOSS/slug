# Current Slug V2 Packet

Packet: `WP-4-5-7A-selected-toolchain-context-cutover-implementation`

Milestone: M7A category 6, selected implementation exec-configuration and
`ctx.toolchains` payload cutover.

Base: `be1a6e581`. Category 4's provider-independent configured selection is
accepted in `568b0c698`; category 5's retained graph is accepted in
`5ce967d55`, and its evaluator adapter is accepted in the base commit.

Do not edit Rust until independent reserved-architecture review accepts this
manifest. This packet is the complete implementation contract; the immediately
preceding zero-Rust architecture packet and owner-plan record are rationale,
not separate scheduling authority.

## Observable result

Delete the marker-only post-selection bridge. For every selected category-4
row, analyze the declared implementation under the chosen platform's exact
structural exec configuration, retain its authenticated category-5 builtin
`ToolchainInfo` occurrence, publish ordered requested/actual/optional topology,
and expose the same occurrence through an immutable alias-aware multi-type
`ctx.toolchains`. No parser, BCR rule logic, ruleset-specific payload or new
DICE key is added.

## Pinned authority and learned facts

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole compatibility authority:

- `ConfiguredTargetFunction` lines 349-369 loads configured toolchain
  dependencies before the requesting rule;
- `DependencyProducer` lines 160-176 associates those dependencies with the
  chosen execution platform;
- `ResolvedToolchainContext.load` lines 48-105 extracts builtin
  `ToolchainInfo`, preserves mandatory/optional rows, permits other providers,
  rejects a missing required provider and represents an unresolved optional
  implementation as null;
- `StarlarkRuleContext.toolchains` lines 878-897 and
  `StarlarkToolchainContext` lines 69-151 define immutable Label/String lookup,
  membership, missing-optional `None` and unrequested-key failure; and
- `ResolvedToolchainContextTest.load_optional_missing`,
  `load_aliasedToolchain`, `load_withTemplateVariables`, and `RuleContextTest`
  supply discriminating tests for optional, alias, extra-provider and arbitrary
  field behavior.

The existing `ConfiguredToolchainResolution` already owns the chosen
`ConfiguredPlatform` whose actual key carries the correct `Exec` configuration
and ordered requested/actual mandatory/optional rows. Its selected declaration
package is already a tracked dependency, but each row currently discards the
declaration's implementation label. Retain that label in the provider-free
resolution row; do not reload or rediscover the declaration later.

The base category-5 graph supplies a cheap-clone `ProviderOccurrence`, deep
publication equality and an evaluator inverse adapter. The current marker
bridge instead accepts only one mandatory row, uses the requester's target
configuration, forbids ordinary selected-rule behavior, extracts one string,
and reconstructs a fresh provider. All of that is deleted.

`docs/developers/dice.md` applies. The configured-analysis key remains the sole
semantic producer; DICE owns deduplication/invalidation and no lock crosses a
compute. The existing configured-analysis cycle detector guards the complete
selected-child future for a rule key.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is concept/test guidance only. Its requested/post-alias row separation,
unresolved optional `None`, and caller-owned evaluator materialization are
useful. Do not copy its Zig layout, ordinals, stores, scheduler, error claims
or compatibility conclusions.

## Retained model and invariants

Replace the singular `ToolchainSelection`/marker tuple with these logical
shapes in `result.rs` (names may vary only mechanically):

```text
ConfiguredToolchainSelection {
  declaration: CanonicalLabel,
  implementation: ConfiguredTargetKey,        // requested dependency, Exec
  actual_implementation: ConfiguredTargetKey, // alias-resolved, Exec
  info: ProviderOccurrence,                    // builtin ToolchainInfo
}

ConfiguredToolchainContextRow {
  requested: ConfiguredTargetKey, // requesting target configuration
  actual: ConfiguredTargetKey,    // post-alias type, same configuration
  mandatory: bool,
  selected: Option<ConfiguredToolchainSelection>,
}

ConfiguredActionToolchainContext {
  execution_platform: ConfiguredTargetKey, // Exec
  rows: Arc<[ConfiguredToolchainContextRow]>,
}
```

Construction validates all of the following before publication:

- the execution platform and both implementation keys use `Exec`;
- requested and actual type keys use the requester's complete target
  configuration, and requested identities are unique while several requested
  aliases may share one actual type;
- a mandatory row is selected; an unresolved optional row has no declaration,
  implementation or provider; a selected row has all of them;
- the occurrence identity is exactly builtin `ToolchainInfo`, never a user
  provider with the same printable name;
- every selected implementation uses the context's execution-platform
  configuration and its actual key is the child result's authenticated actual
  configured target; and
- row order is the original rule requirement order.

`ToolchainTopology` owns candidate-platform order plus an optional shared
`Arc<ConfiguredActionToolchainContext>`; delete singular `selection()`. The
same Arc is used by requester topology, its default action context and evaluator
preparation. `ConfiguredActionExecutionState` is `SelectedToolchain` only when
at least one row is selected; an all-unresolved-optional context remains
`SelectedPlatformOnly`. A rule with no requirements retains no toolchain
context.

Retained rows remain cheap-clone and `Allocative`, but context equality is
manual. `ProviderOccurrence::eq` is intentionally Bazel-visible and is not a
publication boundary. Admit one narrow category-5 build-api helper that
compares an ordered iterator of occurrence pairs with one shared
`PublicationEqState`. `ConfiguredActionToolchainContext::eq` first compares
every non-payload field and selection shape, then routes every selected
`ToolchainInfo` pair through that single helper call. It must not derive or
compose per-row occurrence equality. This preserves the depset alias partition
across all fields and rows, so a publication-only selected payload change
invalidates the parent even when configured labels and Bazel-visible values
compare equal. Containing `Arc`/topology equality delegates to this context
boundary; no retained evaluator state or public equality-state type is added.

## Producer algorithm and deterministic order

Extend provider-free `ConfiguredToolchainResolutionRow` with
`implementation: Option<CanonicalLabel>`, populated from the already-loaded
selected `NativeToolchainTarget`. Declaration and implementation are either
both present or both absent. Eligibility and platform selection do not inspect
or analyze providers and remain unchanged.

Replace `prepare_marker_toolchain_bridge` with one general preparation:

```text
exec_platform = resolution.execution_platform.actual
require exec_platform.configuration.kind == Exec

specs = rows in declared requirement order:
  absent optional -> unresolved row spec
  selected -> ConfiguredNodeAnalysisKey(implementation,
                exec_platform.configuration.clone())

guard configured-analysis cycle detector around the complete selected join
compute selected children through the existing Legacy/Observed key family
aggregate every Need; otherwise choose first outer error, then first semantic
error, in original row order

for each selected child:
  accept ordinary Rule or alias-to-Rule result topology/actions/providers
  require providers.toolchain_info() by builtin identity
  retain requested key, child.actual_configured_target and occurrence
construct validated ordered context
```

Do not reject selected implementation dependencies, nested toolchain
requirements, transitions already admitted by configured analysis, actions,
outputs, diagnostics, capabilities, build-setting role, or additional
providers. They remain owned by the selected child; the requester holds only
the typed edge/context/provider reference. Unsupported category-5 payload
kinds still fail closed during the child rule's ordinary lowering.

Parent edges are deterministic:

1. existing ordinary/transitioned/visibility edges;
2. one `ToolchainRequirement` edge for every row, including an unresolved
   optional row, targeting its requested type key;
3. one `SelectedToolchainImplementation` edge for every selected row,
   targeting the requested configured implementation key; and
4. candidate execution platforms in existing order.

The actual implementation identity is retained in the row, never substituted
for dependency occurrence identity. Requirement aliases retain requested and
actual keys. No output token, checksum, path or digest participates.

## Definition context and evaluator adapter

Loading's `rule()` is the natural owner of the definition context needed to
interpret a string used by that rule's implementation. Retain one shared
`Arc<BzlModuleIdentity>` (canonical defining `.bzl` label plus selected
repository mapping; the existing workspace path remains observation
provenance) in `RuleDefinitionGen`, `FrozenRuleDefinition` and
`StarlarkRuleImplementation`. Include canonical label and mapping in loading
equality. Export a narrow read-only label-resolution helper that delegates to
the existing loading resolver; analysis must not parse apparent repositories
or infer a mapping.

Before evaluator entry, materialize each selected occurrence exactly once into
the module's `FrozenHeap` through `AnalysisValueMaterializer`; retain only the
resulting `FrozenValue` in evaluator-scratch rows. `AnalysisToolchains` stores
those rows plus the shared definition context. It implements:

```text
transform(Label)  -> canonical identity from StarlarkLabel
transform(String) -> loading resolver(raw, defining BzlModuleIdentity)
transform(other)  -> typed unsupported-index error

contains(key): transformed key matches requested or accepted post-alias type
index(key):
  no row       -> unrequested/configured-types error
  optional row -> None
  selected row -> pre-materialized exact ToolchainInfo value
```

Repeated and requested/post-alias lookups return the same evaluator value.
Fresh construction, dependency rematerialization and `ctx.toolchains` therefore
use the same loading-owned `StarlarkToolchainInfo` class and category-5 value
semantics. No evaluator value or `FrozenHeap` is retained in DICE output.

## Compatibility classification

- **Exact:** for the admitted category-5 graph, Bazel 9.2 selected
  implementation exec configuration, ordered required/optional rows,
  provider authentication, additional child providers/actions, requested and
  post-alias Label/String lookup, membership, missing optional `None`, repeated
  lookup identity, dependency topology and deterministic error precedence.
- **Slug-native:** Rust Arc layout, complete structural configuration identity,
  publication equality, configured-cycle wording, action-context row layout
  and unproved diagnostic text.
- **Unsupported/deferred:** retained evaluator functions in provider fields,
  `ToolchainTypeInfo` index objects, template-variable projection through
  `ctx.var`, exec/automatic exec groups, aspects/subrules, unadmitted
  transitions, broader actions and exact Bazel configuration/output bytes.

BCR Starlark owns all rule/control flow including `cc_internal`. `cc_common`
and future builtins are generic host/provider-ABI clients of this category-wide
value/context architecture, not parsers or Rust rule engines. Language
builtins such as `set` remain evaluator-global and do not enter retained
provider storage.

## Ownership, revision and memory behavior

The existing configured-analysis key owns resolution, selected child
dependencies, parent edges and the retained context. Registration, constraint,
command overlay, implementation BUILD/`.bzl`, structural configuration,
repository mapping and provider-payload changes invalidate through existing
tracked dependencies. Legacy and observed modes use the same algorithm. A/B/A
must restore complete result equality without evaluator replay.

Resolution/context rows, configured identities, definition identity and
provider occurrences are DICE/loading-retained semantic memory. Child/join
aggregation, cycle futures, materializer memo tables, frozen evaluator values
and toolchain views are phase scratch and release on success, failure or
cancellation before publication. There is no cache, interner, registry, lock,
background task or async transfer.

No new request input or external observation is introduced. Existing command
configuration, mapping and source owners remain responsible for overlapping
requests and final revision validation.

## Fallback deletion ledger

Delete, do not hide or retain:

- `root_apparent_type` and the single-root string-key assumption;
- `is_marker_leaf_target`, `validate_marker_toolchain`,
  `prepare_marker_toolchain_bridge` and marker-only `prepare_selected_toolchain`;
- `ConfiguredActionToolchainContext`'s `CompactString` marker and `marker()`;
- singular `ToolchainSelection`, `ToolchainTopology::selection()` and all
  one-row constructor shims;
- `AnalysisToolchains` marker allocation/reconstruction; and
- negative tests whose only purpose is to require marker leaf schema, zero
  dependencies/actions, exact two-provider cardinality or one named string
  field.

The violated invariant is exact provider/configuration identity. The deletion
condition is this packet's retained multi-row handoff, and the regressions below
must fail if any marker reconstruction or target-configured child returns.

## Required proof

Reuse accepted category-4/category-5 fixtures and pinned-source tests; no new
Bazel fixture is required. Proof must discriminate:

- target versus selected exec configuration by a target-scoped Starlark option
  absent in the selected child and an exec-propagated option present, with the
  child's configured key equal to the chosen platform configuration;
- two selected mandatory types plus one unresolved optional type in declared
  order; membership true for all requested labels, optional index `None`, and
  unrequested index failure;
- requested alias and post-alias Label/String lookup returning the same
  evaluator object; an external definition uses its retained repository
  mapping rather than the requester's package or root assumptions;
- one arbitrary admitted nested `ToolchainInfo` payload containing label,
  artifact, list/dict/struct/provider/depset values, with repeated lookup and
  direct/rematerialized equality; user `ToolchainInfo` collision never matches;
- selected implementation ordinary dependency, extra provider, output/action
  and child-owned edge/action behavior without copying actions into the parent;
- a selected implementation with its own toolchain requirement, plus a
  selected-toolchain cycle, deterministic failure, edit recovery and no hang;
- missing builtin `ToolchainInfo` despite a user same-name provider, mandatory
  absence, selected child semantic failure, outer/Need precedence and stable
  row-order errors;
- registration/payload/mapping edit and A/B/A restoration, warm reuse and no
  evaluator state in retained memory; and
- a parent-context DICE cutoff A/B/A in which selected labels and Bazel-visible
  occurrence equality stay equal while a nested configured-target provider
  payload or depset alias partition changes; B must republish and the final A
  must restore the original context equality without stale payload; and
- unchanged zero-requirement platform-only behavior plus action/query views
  over ordered rows instead of marker text.

Existing rules_rust loading content may inform field shapes but must not be
copied into this packet. A provider field containing an evaluator function is
still the declared category-5 unsupported terminal and belongs to the next
explicit retained-callable decision, not an implicit widening here.

## Allowlist and caps

Rust baselines are exact blobs at `be1a6e581`.

| Path | Baseline blob / lines | Maximum physical growth |
|---|---:|---:|
| `app/slug_build_api_v2/src/analysis_value.rs` | `36d09b931985a0726a4d16660de322b10868231b` / 1,206 | +30 |
| `app/slug_build_api_v2/tests/analysis_value.rs` | `892c95624521c21d50a6ad820876b63518bec15f` / 438 | +60 |
| `app/slug_loading_v2/src/package.rs` | `c54217443ab3991dfb514340392f4ca1076b94cc` / 6,898 | +90 |
| `app/slug_loading_v2/tests/build_file_loading.rs` | `2cf23f2c97e3c2ed46e3ff2f492fb373cf1d8f0a` / 3,581 | +110 |
| `app/slug_analysis_v2/src/result.rs` | `3ad9df5c09303cd83aaa7acfc06d0e96483ba308` / 800 | +280 |
| `app/slug_analysis_v2/src/dice.rs` | `0cfc18bc28e36eaecf025f77b438390eb1218e88` / 4,757 | +300 |
| `app/slug_analysis_v2/src/starlark_rule.rs` | `756e4ebca5802fb1f384546c1b42a54c93773866` / 729 | +220 |
| `app/slug_analysis_v2/src/lib.rs` | `f1144f085c47babc9d848d5aca662d496c500e2b` / 89 | +15 |
| `app/slug_analysis_v2/tests/configured_target.rs` | `675ba67e2f114f310aedb176ebdc91cfc1bd471a` / 831 | +260 |
| `app/slug_analysis_v2/tests/root_analysis.rs` | `91b263ae992b8367b902094e220799aae7f58455` / 1,360 | +140 |
| `app/slug_analysis_v2/tests/starlark_rule.rs` | `a9bf98c55b80ee98d53f14bff7582161370050b4` / 7,469 | +650 |
| `app/slug_core_v2/src/runtime/tests/build_command_tests.rs` | `ad2255da58671c20eb4447ffce3a8bbdcddee05c` / 4,008 | +50 |

Net production growth is capped at +680 lines and proof growth at +1,160
lines, +1,840 total. Moving/deleting marker code counts normally; caps do not
authorize another owner or behavior.

The 6,898-line loading file remains the cohesive rule-definition/package
producer and changes only one shared definition-context field/resolver. The
4,757-line DICE file remains the sole configured-analysis orchestration owner;
the general preparation replaces rather than coexists with marker helpers. The
7,469/4,008-line files are integration proof only. No new central module is
justified by this bounded replacement, and no demonstrated hot-path regression
requires a benchmark; selected rows reuse already retained Arc/provider data.

No other file may change, especially parser/compiler syntax, category-5 build
API representation beyond the narrow stateless occurrence-pair publication
helper, loading registration/global semantics, configuration, query/reapi
production, server/CLI, BCR content or Zabel.

## Validation and stops

Run Cargo serially:

- `cargo fmt --all -- --check`;
- focused loading rule-definition/context tests and full
  `cargo test -p slug_loading_v2 --no-fail-fast`;
- focused build-api occurrence publication-pair tests and full
  `cargo test -p slug_build_api_v2 --no-fail-fast`;
- focused retained-context constructors and exact category-6 Starlark tests,
  then full `cargo test -p slug_analysis_v2 --no-fail-fast`;
- `cargo test -p slug_query_v2 --no-fail-fast`,
  `cargo test -p slug_reapi_v2 --no-fail-fast`, and
  `cargo test -p slug_core_v2 --no-fail-fast` as public direct dependents;
- `scripts/v2_archive_status.sh`, exact baseline-blob/allowlist/physical and
  net-cap audits, forbidden-marker searches and `git diff --check`; and
- independent terminal DICE/identity/evaluator review.

The base full-core failures for stale external-query event replay and missing
injected `PathObservationEpochKey` are independently documented; reproduce any
remaining occurrence exactly and never weaken their assertions. No CLI binary
smoke is required unless a server-observable result changes; rebuild
`slug_cli_v2` before any such smoke.

Return `REPLAN` before more Rust for a new DICE key, target-configured selected
implementation, inferred repository mapping, retained evaluator value/heap,
process store/cache/interner, lock across DICE compute, missing optional-row or
alias semantics, per-builtin arbitrary field struct, lossy provider copy,
marker compatibility shim, parser/BCR/ruleset control flow, configuration/
display/digest substitution, unguarded recursive selection, file outside the
allowlist, a required cap increase, or one material correction after
implementation begins. One focused implementation correction is permitted;
a second material miss is `REPLAN`.

Residual risk is recursive selected-rule fanout, unsupported callable-valued
provider fields and later exec-group breadth. DICE deduplicates configured
children; callable retention and exec groups remain explicit later decisions.
