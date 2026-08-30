# Current Slug V2 Packet

Packet: `WP-4-5-7A-selected-toolchain-context-cutover-implementation-r3`

Milestone: M7A category 6 prerequisite for configured hidden Exec dependencies.

Base: `ad274cc1a`. The prior configured-hidden-dependency R4 implementation is
not part of this packet. Its index-only gate proved that an Exec-configured
Starlark dependency reaches the selected-toolchain/action-context boundary,
which remains Target-only at this base. All R4 and unrelated dirty repository,
loading, analysis, core and REAPI hunks remain excluded.

R2 terminal implementation review returned `REPLAN` only because its mandatory
one-shot REAPI proof did not reach the selected-context assertions. The R2
candidate advanced beyond the base selected-BCR terminal to the unrelated
`rules_shell` `attr.label_list(flags=...)` loading boundary; a bounded local
rules_shell experiment then advanced to the independently deferred recursive
`glob(["**"])` in embedded `@bazel_tools//tools/res`. R3 does not widen the
parser, attribute, glob, BCR or ruleset surface to make that command succeed.
It replaces only the proof route with a hermetic semantic-consumer regression.

## Observable result

Delete the marker-only selected-toolchain bridge. For every selected
toolchain row, analyze the declared implementation under the chosen platform's
exact structural Exec configuration, retain its authenticated builtin
`ToolchainInfo` occurrence, publish ordered requested/actual/optional topology,
and expose the same occurrence through immutable alias-aware multi-type
`ctx.toolchains`. A nested Exec-configured rule with zero or nonzero toolchain
requirements uses the ordinary guarded configured-analysis pipeline without a
Target-owner fallback or deadlock.

No parser, BCR rule logic, ruleset-specific payload, new DICE key, cache,
registry or evaluator-retained value is added.

## Authority and learned facts

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is
the sole semantic authority:

- `ConfiguredTargetFunction.java:349-369` loads configured toolchain
  dependencies before the requesting rule;
- `DependencyProducer.java:160-176` associates those dependencies with the
  selected execution platform;
- `ResolvedToolchainContext.load:48-105` extracts builtin `ToolchainInfo`,
  preserves mandatory/optional rows, permits additional providers, rejects a
  missing required provider and represents unresolved optional selection as
  null;
- `StarlarkRuleContext.toolchains:878-897` and
  `StarlarkToolchainContext:69-151` define immutable Label/String lookup,
  membership, missing-optional `None` and unrequested-key failure; and
- `ResolvedToolchainContextTest.load_optional_missing`,
  `load_aliasedToolchain`, `load_withTemplateVariables`, and the applicable
  `RuleContextTest` cases supply the discriminating semantics.

The accepted provider-independent resolution already owns the selected
`ConfiguredPlatform` and ordered requested/actual mandatory/optional rows. Its
selected declaration package is tracked, but the retained row currently drops
the implementation label. Retain that label at the producer; do not reload or
rediscover it later.

The accepted recursive analysis-value graph supplies cheap-clone provider
occurrences, deep publication equality and fresh evaluator materialization.
The marker bridge instead supports only one mandatory row, uses Target
configuration for the selected implementation, extracts one string and
reconstructs a provider. Delete that bridge.

The existing `ConfiguredNodeResult::configured_file_write_actions` is the
validation boundary for the public `ConfiguredActionView`: it rejects absent
selected platforms, non-FileWrite actions, invalid output shape and unsupported
execution fields before yielding an otherwise unconstructible borrowed view.
R3 may expose one doc-hidden
`ResolvedFileWriteSemanticView::from_configured_action` composition seam that
only wraps this already-validated view. It adds no state, alternate validation,
fallback, retained value or executor policy and cannot outlive its source
configured result.

The failed R4 index-only run is a new discriminating regression: broadly
admitting Exec resolution without this cutover either rejects an ordinary
Exec child at `ConfiguredActionOwnerContext` or follows an injected semantic
platform terminal into recursive analysis. The prerequisite must preserve the
accepted `selected_platform_terminals_suppress_implementation_and_rule_evaluation`
test and make genuine selected children use the full retained context.

`docs/developers/dice.md` applies. `ConfiguredNodeAnalysisKey` remains the sole
semantic producer; the existing configured-analysis cycle detector guards the
complete selected-child computation and no lock crosses a DICE compute.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is peer concept/optimization guidance only. Requested/post-alias row
separation, unresolved optional rows and caller-owned evaluator materialization
are useful. Copy no Zig code, layout, names, diagnostics or compatibility
claims.

## Decision and compatibility

Replace singular marker selection with one ordered retained context:

```text
ConfiguredToolchainSelection {
  declaration: CanonicalLabel,
  implementation: ConfiguredTargetKey,        // requested, Exec
  actual_implementation: ConfiguredTargetKey, // alias-resolved, Exec
  info: ProviderOccurrence,                    // builtin ToolchainInfo
}

ConfiguredToolchainContextRow {
  requested: ConfiguredTargetKey, // requester's structural Target or Exec configuration
  actual: ConfiguredTargetKey,    // post-alias type, same configuration
  mandatory: bool,
  selected: Option<ConfiguredToolchainSelection>,
}

ConfiguredActionToolchainContext {
  execution_platform: ConfiguredTargetKey, // selected structural Exec
  rows: Arc<[ConfiguredToolchainContextRow]>,
}
```

Construction validates that requested and actual type rows use the requester's
identical structural Target-or-Exec configuration, selected platform and
implementation identities use structural Exec, and requested rows are unique.
It also validates alias convergence, declaration/implementation pairing,
mandatory selection, builtin `ToolchainInfo` identity and row order. Several
requested aliases may share one actual type and one evaluator-scratch value.

`ToolchainTopology` retains candidate order plus an optional shared
`Arc<ConfiguredActionToolchainContext>`. The requester topology, default action
context and evaluator preparation use the same Arc. An all-optional unresolved
context is `SelectedPlatformOnly`; no requirements retain no context.

- **Exact:** structural Target-or-Exec requester configuration on requested and
  actual type rows, selected implementation configuration, ordered
  mandatory/optional rows, requested and post-alias lookup, missing optional `None`, provider
  authentication, additional child providers/actions, nested selected
  requirements, dependency/error order and immutable `ctx.toolchains` behavior
  for admitted recursive analysis values.
- **Slug-native:** complete structural configuration identity, Rust Arc/layout,
  retained publication equality, cycle wording and unproved diagnostics.
- **Unsupported/deferred:** callable-valued retained provider fields,
  `ToolchainTypeInfo` index objects, `ctx.var`, nondefault/automatic exec groups,
  aspects, broader actions, exact Bazel configuration/output bytes, and the
  configured-hidden subrule invocation/publication work that resumes in R4.

BCR Starlark owns all rules and control flow including `cc_internal`.
`cc_common` and future builtins are generic host/provider-ABI clients. The
starlark-rust `set` builtin remains evaluator-global and is not reimplemented.

## Producer, equality and evaluator handoff

Extend provider-free resolution rows with the already-loaded selected
implementation label. In declared requirement order, prepare unresolved
optional rows or guarded Exec-configured selected-child keys. Aggregate every
Need; otherwise choose the first outer error and then first semantic error in
row order. Accept ordinary Rule or alias-to-Rule results, require the builtin
`ToolchainInfo` occurrence and retain requested plus authenticated actual child
identity.

Parent edges remain deterministic: ordinary/transitioned/visibility edges,
one requirement edge per row, one selected-implementation edge per selected
row, then candidate execution platforms. Selected child actions and providers
remain child-owned.

`ProviderOccurrence::eq` remains Bazel-visible equality, not publication
equality. One build-api helper compares the ordered occurrence pairs with a
single shared `PublicationEqState`; context equality must not compare each row
with a fresh state. This preserves cross-row depset/configured-target alias
partitions and DICE cutoff behavior.

Loading retains the rule definition's shared `Arc<BzlModuleIdentity>` and
repository mapping. A read-only loading resolver interprets String indices;
analysis does not infer mappings. Before evaluator entry,
`AnalysisValueMaterializer` materializes each selected occurrence once into
the module `FrozenHeap`; only evaluator-scratch `FrozenValue`s are retained.
Label/String requested and post-alias lookup return the same object.

The REAPI proof constructs genuine structural Target and Exec configurations,
a selected platform, an authenticated selected `ToolchainInfo` occurrence, one
`ConfiguredActionOwnerContext`, and one `ConfiguredNodeResult` with a real
FileWrite action. It obtains the action only through
`configured_file_write_actions`, wraps that borrowed validated view through the
doc-hidden semantic-view seam, and calls the unchanged
`FileWriteReapiPlan::from_resolved`. This is direct cross-crate proof that REAPI
consumes the generic semantic view; it is not an end-to-end BCR/loading or
command-closure claim.

## Ownership, revision and memory

`ConfiguredNodeAnalysisKey` owns resolution, selected children, edges and the
retained context. Existing registration, constraint, command-overlay,
implementation source, mapping and configuration dependencies drive
invalidation in Legacy and Observed modes. A/B/A must restore complete result
equality without stale payloads or evaluator replay.

Resolution/context rows, configured identities, definition identity and
provider occurrences are DICE/loading-retained semantic memory. They remain
`Allocative`, use compact shared Arc slices and cheap-clone values, and add no
parallel retained descriptor. Join scratch, cycle futures, materializer memo
tables, heaps and lookup views die with the compute/evaluation. There is no
new cache, interner, task, watcher, request carrier or async transfer.

The Buck2 utility review classifies compact collections, `Dupe`/Arc sharing,
`Allocative` and the shared publication-equality state as retained patterns to
preserve. No V1/Buck2 code import or new hash identity is needed.

## Required proof

Reuse accepted fixtures and add no Java helper or Bazel fixture. Prove:

- selected implementation keys exactly equal the chosen platform Exec
  configuration, including target-scoped suppression and exec propagation;
- two mandatory rows plus an unresolved optional row preserve order;
- requested aliases and one actual type share one evaluator object for both
  Label and String lookup, including an external definition mapping;
- arbitrary admitted nested ToolchainInfo payloads, additional providers,
  outputs/actions and child edges remain authentic and child-owned;
- user-provider collisions, missing builtin providers, mandatory absence,
  child failure, Need/outer precedence and configured cycles fail closed;
- a selected implementation with its own toolchain requirement completes
  without deadlock;
- registration, provider payload and mapping A/B/A restore exactly;
- one parent DICE cutoff A/B/A distinguishes a cross-row payload alias
  partition even when Bazel-visible occurrence equality is unchanged;
- zero-requirement platform-only behavior and injected selected-platform
  terminals remain unchanged;
- complete configured identity serialization remains asserted; and
- the real REAPI planner consumes the hermetically constructed validated
  configured-action semantic view, observes the requested and implementation
  identities plus the retained `ToolchainInfo` payload, shares the same action
  context Arc, and emits the selected platform's ordered properties rather
  than remote defaults.

## Frozen allowlist and caps

Only these paths may change from `ad274cc1a`:

- `Cargo.lock`, blob
  `a4305123359154ae4e78e40b609c83e9515cebcd`, only the proof-only
  `slug_reapi_v2` dependency rows;
- `app/slug_build_api_v2/src/analysis_value.rs`, blob
  `36d09b931985a0726a4d16660de322b10868231b`;
- `app/slug_build_api_v2/tests/analysis_value.rs`, blob
  `892c95624521c21d50a6ad820876b63518bec15f`;
- `app/slug_loading_v2/src/package.rs`, blob
  `191b2082de14e5f057d8183c1c156671bd4cbd2a`;
- `app/slug_loading_v2/tests/build_file_loading.rs`, blob
  `2ad939ed512890c27c3959876db4af20dff7a2d3`;
- `app/slug_analysis_v2/src/result.rs`, blob
  `3ad9df5c09303cd83aaa7acfc06d0e96483ba308`;
- `app/slug_analysis_v2/src/dice.rs`, blob
  `70d59f60b3f4b06702eb347e0b615c6961e912d1`;
- `app/slug_analysis_v2/src/starlark_rule.rs`, blob
  `756e4ebca5802fb1f384546c1b42a54c93773866`;
- `app/slug_analysis_v2/src/lib.rs`, blob
  `f1144f085c47babc9d848d5aca662d496c500e2b`;
- `app/slug_analysis_v2/tests/configured_target.rs`, blob
  `675ba67e2f114f310aedb176ebdc91cfc1bd471a`;
- `app/slug_analysis_v2/tests/starlark_rule.rs`, blob
  `cbdfa181596d2011dbc48ba2dc05064d53123daa`;
- `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`, blob
  `ad2255da58671c20eb4447ffce3a8bbdcddee05c`;
- `app/slug_core_v2/src/runtime/dice.rs`, blob
  `005c20b9e0b0c605654402e9db43935db3e9d4f5`, only the doc-hidden
  semantic-view composition seam and its existing internal caller;
- `app/slug_reapi_v2/Cargo.toml`, blob
  `32d96de03a6093348404c84117166cdcbddc0c21`, only proof-only workspace
  dev-dependencies; and
- `app/slug_reapi_v2/tests/reapi.rs`, blob
  `1c466d40e4233c198d80592a48dafbb4a50ee608`.

Caps are 680 production additions, 1,160 proof additions and 1,840 aggregate
additions. Current candidate measurements are 626/962/1,588, classifying the
Cargo manifest and lockfile dev-dependency rows as proof-only. Physical net
growth remains within the prior R1 per-file caps; the REAPI proof may grow by at
most 140 lines. `dice.rs` and `package.rs` remain large natural orchestration/
definition owners and replace narrow bridge/accessor code; cohesive retained
types remain in `result.rs` and evaluator projection in `starlark_rule.rs`.
The added `dice.rs` seam is a five-addition wrapper at the existing FileWrite
semantic-view owner, not a new responsibility or retained representation.

No benchmark is required: retained rows reuse existing Arc/provider data and
no demonstrated hot-path regression exists. Retained size, publication
equality and full dependent tests are mandatory.

## Validation and stops

Run Rust formatting and `git diff --check`; focused and full serial tests for
`slug_build_api_v2`, `slug_loading_v2` and `slug_analysis_v2`; full serial
direct-dependent tests for `slug_query_v2`, `slug_reapi_v2` and
`slug_core_v2`; archive, allowlist, cap, forbidden-marker, retained-size and
base/index isolation checks; and an index-only repeat. Rebuild `slug_cli_v2`
before any binary replay and clean stale `slugd` around daemon tests.

Independent reserved-architecture plan review and terminal DICE/identity/
evaluator review are mandatory.

`REPLAN` for a new DICE key, Target-configured selected implementation,
inferred mapping, retained evaluator heap/value, cache/interner/registry,
lock across compute, lossy/per-builtin provider copy, per-row publication
state, marker shim, parser/BCR/ruleset logic, unguarded recursive selection,
semantic-view construction from anything other than an already-validated
`ConfiguredActionView`, new validation/fallback/state in that seam, file outside
the allowlist, cap increase, or another material correction.

## Immediate successor

After terminal acceptance and commit, reactivate
`WP-4-5-7A-subrule-configured-hidden-dependencies-and-query-r4` on this new
base. Its implementation remains parked and must be re-staged/revalidated;
then Exec hidden dependencies may use the accepted selected-child/action-
context architecture before the direct-call invocation successor.
