# Current Slug V2 Packet

Packet: `WP-4-5-7A-build-setting-config-declaration-loading`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `b949ce8da`.

Result: implement the accepted loading half of the typed build-setting and
configured-condition architecture. Loading must publish all five Bazel 9.2
Starlark build-setting definitions and type-correct target defaults, expose the
magic scope observation without a second store, retain every native
`config_setting` predicate field once, derive its RuleClass/query projection,
and make flag/constraint labels ordinary query dependencies. This packet does
not create configuration overrides, condition matching, selector resolution,
command flags or provider values.

## Accepted predecessor and boundaries

Commit `b949ce8da` accepts
`WP-4-5-7A-typed-build-setting-condition-architecture`. It freezes one
rule-level definition plus target declaration view, one later scoped-option
map, one configured-condition DICE owner and one selector resolver for
ordinary attributes and `toolchain.target_settings`.

Buck2's Rust Starlark parser remains the sole syntax owner. BCR Starlark owns
all rule control flow, including `cc_internal`; `cc_common` is only a future
generic evaluator/provider/host-ABI client. Pinned Bazel 9.2 at
`8220c6198837d5c13d53fea211cf3282aa12408a` is behavior authority. Clean Zabel
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer architecture and
optimization guidance only; copy no Zig representation or claim.

## Learned facts and live preflight

- `StarlarkConfig` exposes exactly `config.int`, `config.bool`,
  `config.string`, `config.string_list` and `config.string_set`.
  `allow_multiple` belongs only to string; `repeatable` belongs to list/set and
  requires `flag = True`.
- Bazel `Type.INTEGER` restricts the invocation's
  `build_setting_default` to signed 32-bit Starlark attribute values. The
  later command/transition option domain is arbitrary-precision; this packet
  must not widen the default or preclude the later BigInt override.
- Bazel `Types.STRING_SET` accepts a Starlark set, not a list, validates every
  member as a string and deduplicates by set semantics. Slug may store the
  immutable membership in deterministic Slug-native order while retaining the
  distinct set kind.
- `StarlarkOptionsParser` and `BuildOptionsScopeFunction` interpret an absent
  or non-explicit string attribute literally named `scope` as internal
  `DEFAULT`. An explicit string value is matched case-insensitively against
  `universal`, `target` and `project`. A rule schema's ordinary default does
  not replace the magic `DEFAULT` sentinel.
- `ConfigSettingRule` owns nonconfigurable `values`, `define_values`,
  `flag_values` and `constraint_values`. It permits an all-empty declaration
  at BUILD loading and reports that error only during configured target
  creation. `flag_values` is a label-keyed string dictionary with canonical
  collision detection.
- `ConfigSetting` combines `define_values` into native `define` predicates and
  validates constraint target kinds/settings during configured matching, not
  during loading. Loading therefore retains source fields without performing
  the future match.

Current Slug already captures the first four definition shapes, but keeps the
enum private, omits `string_set`, rejects invoked string-list settings, and
offers only a root-string default accessor. Its generic attribute row already
owns default/explicit provenance and signed-32-bit integers. Native
`config_setting` semantically retains only sorted `values`; the other fields
can appear in its derived query row through kwargs but do not affect target
equality or dependency edges.

## Implementation contract

### 1. Public derived build-setting declaration

Replace private `BuildSettingKind` with one public, evaluator-independent
`BuildSettingDefinition` covering all five kinds and their legal
flag/multiple/repeatable shape. Add a public typed `BuildSettingDefault` and
`BuildSettingScope` plus an owned compact `BuildSettingDeclaration` returned by
`StarlarkRuleImplementation`.

The rule definition remains the single owner of kind/parameter shape. The
target's existing semantic attribute row remains the single owner of
`build_setting_default`, `scope` and provenance. The declaration accessor
derives and validates its result from those owners; it stores no parallel
default/scope field and retains no evaluator value. It returns internal
`DEFAULT` for absent/non-explicit scope, accepts case-insensitive explicit
`universal`/`target`/`project`, and fails closed on explicit configurable or
wrong-typed scope.

Integer defaults remain `i32`. String-list defaults remain ordered and
duplicate-preserving. String-set defaults must originate from a Starlark set,
contain only strings, and lower to a unique immutable membership slice in one
deterministic Slug-native order. The typed default distinguishes list from set
even if the native query projection uses the existing string-collection row.
An allow-multiple string keeps its scalar declaration default; the accepted
later effective-value owner will wrap it as a singleton list.

### 2. Complete definition and invocation surface

Add `config.string_set(flag = ..., repeatable = ...)` beside the other four
constructors and enforce the same repeatable/flag invariant as Bazel. Preserve
definition export/import/freezing through root and external `.bzl` routes.

Remove the string-list invocation stop and admit type-correct target
invocations for every definition kind. `build_setting_default` stays mandatory
and nonconfigurable. Reject wrong scalar/collection/member types, list-for-set,
set-for-list, out-of-range integer defaults and missing defaults during BUILD
evaluation. Do not execute the implementation or create a configured value.

### 3. One semantic native config-setting declaration

Replace `PackageTargetKind::ConfigSetting { values }` with one public compact
`ConfigSettingTarget` containing:

- canonically normalized `values` and `define_values` string pairs;
- canonically normalized canonical-label-to-string `flag_values` pairs with
  duplicate-canonical-label rejection;
- ordered `constraint_values` canonical labels; and
- explicit/default provenance for all four source attributes.

Both global `config_setting` and `native.config_setting` lower through one
recorder helper. Keep an all-empty declaration valid. Preserve constraint
duplicates/order for the configured validator rather than inventing a loading
decision. Add flag and constraint labels to the semantic reference projection.
`NativeRuleAttributes` must be derived from this object, including empty
defaults and provenance; kwargs may no longer be a second semantic owner for
the four modeled fields.

### 4. Loading-query dependency projection

Root and canonical external package query graphs add ordinary edges for the
config-setting semantic flag and constraint references, deduplicated only for
edge traversal while retaining source declaration order in the target. Query
attributes continue to come from the derived native row. Do not add configured
matching, alias resolution or platform validation.

## Compatibility classification

- **Exact:** five constructor names/parameters and repeatability validation;
  signed-32-bit integer defaults; type-correct scalar/list/set target defaults;
  case-insensitive magic scope observation and default provenance; four-field
  config-setting loading declarations, canonical label-key collision failure,
  all-empty loading success, and normal flag/constraint dependency labels.
- **Slug-native:** Rust enum/container names, deterministic canonical ordering
  of order-insensitive map/set identity, compact allocation layout, and error
  wording not pinned by an accepted oracle.
- **Unsupported/deferred:** arbitrary-precision configured integer overrides,
  effective values, configuration/scope maps, transitions, command flag
  parsing, config-setting matching/errors, constraint/provider validation,
  aliases/feature flags/groups/label settings, selector resolution, provider
  payloads and toolchain selection.

## Proof obligations

1. All five definition kinds survive root/external export/import with exact
   flag/multiple/repeatable shape; invalid repeatable non-flags fail.
2. All five invocation defaults publish typed heap-independent declarations;
   wrong kinds/members, missing values, list/set swaps and integer overflow
   fail without running implementations.
3. Omitted/default/explicit mixed-case scopes derive `DEFAULT`, `universal`,
   `target` and `project` correctly; configurable/wrong-typed explicit scope
   fails only when the declaration is demanded.
4. Config-setting equality owns changes to each of four fields and provenance;
   map source order is a semantic no-op, constraint order remains visible, and
   canonical flag-label collisions fail.
5. Native RuleClass/query rows are derived, and root/external query deps expose
   each distinct flag/constraint label exactly once.
6. Loading DICE A/B/A restores after changing each definition/default/scope or
   predicate field; formatting/map-order no-ops do not invalidate equality.

Reuse pinned `StarlarkOptionsParsingTest`, `StarlarkRuleContextTest`,
`ConfigStringSetTest`, `ConfigSettingTest`, `BuildOptionsScopeFunctionTest` and
`BuildType.LabelKeyedDictType` as source regressions. Extend existing Slug
definition, build-file loading, Bzl invalidation and loading-query tests. Add no
oracle unless implementation exposes a discriminator these sources do not
settle.

## Ownership and memory

Definitions, typed defaults, scope source attributes and config-setting
predicates live only in immutable loaded-package DICE results. Declaration
views are derived owned values; conversion/dedup buffers are evaluation
scratch. Use the existing `Arc` slices, `CompactString`, canonical labels,
`SmallMap`/`SmallSet`, `Dupe` and `Allocative`. Add no interner, cache, global
store, evaluator heap, text hash, lock or second query/predicate row. No DICE
key or lock changes are authorized.

## Allowlist and caps

Production Rust:

1. `app/slug_loading_v2/src/package.rs`;
2. `app/slug_query_v2/src/graph.rs`.

Proof Rust:

3. `app/slug_loading_v2/src/host_package_load_tests.rs`;
4. `app/slug_loading_v2/tests/build_file_loading.rs`;
5. `app/slug_loading_v2/tests/bzl_invalidation.rs`;
6. `app/slug_query_v2/tests/loading_query.rs`.

Completion docs:

7. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
8. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
9. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`.

Caps: 700 production Rust lines, 900 proof Rust lines, 1,600 total Rust lines
and 220 completion-ledger lines. `package.rs` is already above the complexity
trigger; this packet is cohesive because rule definition/invocation,
declaration derivation and native publication share its existing package owner.
Do not perform unrelated cleanup. No Cargo/lockfile/BUILD, fixture, oracle,
configuration, analysis, CLI or Zabel file is admitted.

## Validation

Run serially:

1. focused loading definition/invocation/config-setting unit and integration
   tests;
2. `cargo test -p slug_loading_v2`;
3. focused root/external `slug_query_v2` loading-query tests, then
   `cargo test -p slug_query_v2`;
4. direct compile checks for `slug_analysis_v2` and `slug_bzlmod_v2` after the
   public loading shape changes;
5. `cargo fmt --all -- --check`, `git diff --check`, exact allowlist/caps and
   the named archive baseline; and
6. independent retained-representation/loading review before acceptance.

## Stops

STOP and `REPLAN` for a required file outside the allowlist; a second retained
definition/default/scope/predicate store; `AttributeKind`-wide set churn when a
typed build-setting projection suffices; evaluator heap retention; default
integer widening or future override narrowing; list/set conflation in the
public declaration; configured matching or selector evaluation; a new DICE
key/lock; query-owned semantics; command/configuration/provider/toolchain work;
Rust BCR rule control flow, `cc_internal` or `cc_common` parsing; Zabel
authority; Cargo/oracle changes; cap overflow; or a second material contract
correction.
