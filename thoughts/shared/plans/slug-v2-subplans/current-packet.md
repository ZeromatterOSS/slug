# Current Slug V2 Packet

Packet: `WP-4-5-7A-typed-build-setting-condition-architecture`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `d9df71392`.

Result: freeze the zero-Rust architecture and bounded implementation sequence
for the complete Bazel 9.2 Starlark build-setting category, native
`config_setting` facts, typed configuration identity, configured condition
matching and shared `select()` resolution. This packet changes no Rust and
admits no command flag, platform/toolchain selection or provider payload.

## Immediate predecessor

Commit `d9df71392` accepts complete loading-owned native `toolchain()`
declaration semantics. Canonical mandatory labels, execution/target
constraints, target-platform policy and the original configurable
`target_settings` expression now participate in package equality; selector
conditions populate `$config_dependencies`. The current marker consumer keeps
its accepted default behavior and rejects every nondefault newly retained case
in legacy and observed analysis before implementation selection.

The next category cannot extend the old singleton root-string setting. It must
own every Bazel 9.2 Starlark build-setting kind and provide one condition path
used by ordinary configurable attributes and `toolchain.target_settings`.

## Learned facts and source basis

Pinned Bazel 9.2 at commit
`8220c6198837d5c13d53fea211cf3282aa12408a` remains authority.

- `StarlarkConfig` exposes five build-setting constructors: `config.int`,
  `config.bool`, `config.string`, `config.string_list` and
  `config.string_set`. String settings may be `allow_multiple`; list/set
  settings may be `repeatable`, but repeatability requires `flag = True`.
- `BuildSetting` owns type, flag, multiplicity and repeatability. A rule
  invocation supplies a type-correct `build_setting_default`; the configured
  value is a typed override when present and otherwise that declaration's
  effective default. For `allow_multiple` string settings only, the scalar
  declaration default becomes a singleton configured list.
- A build-setting target may also expose Bazel's magic string attribute named
  `scope`. An omitted or absent string-typed attribute is the internal
  `DEFAULT` scope,
  even when the rule schema has an ordinary default such as `"universal"`;
  explicit `universal`, `target` and `project` are the admitted names. Scope is
  retained only beside a nondefault Starlark option, participates in Bazel
  `BuildOptions` identity, and controls target-to-exec propagation. Project
  scope additionally requires `PROJECT.scl` ownership.
- Bazel's final Starlark-options map contains effective nondefault overrides.
  Setting a flag to its declaration default removes the row. Defaults remain
  declaration facts, not copied configuration entries.
- `CoreOptionConverters.BUILD_SETTING_CONVERTERS` converts config-setting
  `flag_values` text to the referenced declaration's type. Integer
  command/transition overrides are arbitrary-precision Starlark integers,
  while a rule invocation's integer `build_setting_default` remains in Bazel's
  signed 32-bit attribute range. String lists are comma-separated ordered
  lists; string sets are deduplicated sets; multi-valued actuals match a single
  converted expected element.
- `ConfigSetting` combines `values`, `define_values`, `flag_values` and
  `constraint_values`, diagnoses an all-empty predicate when its configured
  match target is created, resolves
  referenced build settings as configured prerequisites, compares a typed
  override or the declaration default, and publishes a configured match fact.
- Configurable attributes consume configured condition facts. Selector keys
  are prerequisites rather than branch-value dependencies; default branches,
  equal-value multiple matches, specialization and ambiguity remain distinct
  cases.
- `StarlarkRuleContext` exposes `ctx.build_setting_value` independently of the
  ordinary providers returned by the build-setting implementation.

Primary pinned tests are `ConfigSettingTest`'s scalar, repeated, list and set
flag-value cases; `ConfigStringSetTest`'s default/command/transition
normalization cases; `ConfigurableAttributesTest`'s condition selection cases;
and existing transition tests for typed Starlark options. Pinned scope anchors
are `Scope`, `BuildOptions`, `StarlarkOptionsParser`,
`FunctionTransitionUtil`, `BuildOptionsScopeFunction` and their scope tests.
Reuse the accepted Slug `query-attr-observable-candidates`, root string-setting
lifecycle and transition proofs, `rules-rust-073-toolchain-owner`, and native
configuration metadata/converter tests. Add an oracle only for a concrete
discriminator not covered by these sources or accepted fixtures.

The live Slug audit found:

- loading retains `int`, `bool`, `string` and `string_list` definition metadata
  but omits `config.string_set` and rejects every invoked setting except a
  flag, non-`allow_multiple` string;
- `PackageTargetKind::ConfigSetting` semantically owns only the `values`
  dictionary while `flag_values`, `define_values` and `constraint_values` live
  only in the derived native RuleClass row;
- `SlugConfiguration` carries one `RootStringSettingValue`, including a copied
  default, has no Starlark-option scope carrier, and its transition path
  accepts exactly one string output;
- configured analysis has no native config-setting node and does not resolve
  `CoercedAttributeValue::Selector`; and
- the configuration crate already owns a complete typed native-option table,
  structural canonical bytes, exec projection and collision-safe Slug-native
  identity.

Clean Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only. Its
separate loading declarations, canonical final Starlark-option rows, typed
condition-input projection, configured matcher and final-options ownership
support keeping declarations, overrides and match results separate. Copy no
Zig row layout, arena/store registration, stringified value format, scheduler,
diagnostic or compatibility claim. Bazel 9.2 alone decides behavior.

## Frozen category architecture

### 1. Complete loading declarations

Loading owns one public, evaluator-independent rule-level
`BuildSettingDefinition`. It records exactly one of:

- integer, with a signed 32-bit invocation default and arbitrary-precision
  configured overrides;
- Boolean;
- scalar string, including `allow_multiple`;
- ordered string list, including `repeatable`; or
- canonical string set, including `repeatable`.

The definition also records `flag`, `allow_multiple` or `repeatable` as
applicable. A target-level `BuildSettingDeclaration` view combines that shape
with the invocation's type-correct immutable `build_setting_default` and magic
scope observation. `config.string_set` joins the existing four constructors
through the same path. Defaults lower from Starlark values at package
publication; no `Value`, `FrozenValue`, evaluator heap or text-only surrogate
enters either surface. The existing `i32` attribute representation is exact for
an integer declaration default but must never narrow a command/transition
override in the configuration map. String-set defaults retain set semantics
and expose a deterministic canonical iteration order at the Rust/Starlark
boundary.

The target's ordinary attribute row remains the single owner of `scope` and
its explicit/default provenance. A derived declaration accessor returns
`DEFAULT` unless a string attribute literally named `scope` was explicitly
set, validates an explicit value as `universal`, `target` or `project`, and
rejects configurable or wrong-typed forms when the setting is prepared. Do
not retain a second scope/default store: the typed declaration and native
RuleClass/query values are derived from the rule definition plus the target's
single semantic attribute row.

Replace the narrow `PackageTargetKind::ConfigSetting` value with one semantic
native declaration containing:

- canonically normalized native `values` string pairs;
- canonically normalized `define_values` string pairs;
- canonically normalized canonical-label-to-string `flag_values` pairs; and
- ordered canonical `constraint_values` labels;

plus explicit/default provenance where Bazel query distinguishes it.
`NativeRuleAttributes` remains a derived RuleClass/query projection. Every
flag and constraint label is a normal loading dependency. Wrong types and
canonical duplicate keys fail during BUILD evaluation; an all-empty
declaration remains loadable and becomes a configured-condition error before
matching, as in Bazel. Do not retain a second predicate store.

### 2. One typed Starlark-option configuration map

Replace `RootStringSettingValue` with one immutable, canonical-label-keyed map
owned by `slug_configuration_v2`. Each sorted entry is one
`StarlarkOptionOverride` containing a kind tag, resolved scope and one
heap-independent typed value:

- arbitrary-precision signed integer using the workspace's existing
  `num-bigint`/Allocative support;
- Boolean;
- `CompactString`;
- ordered duplicate-preserving `Arc<[CompactString]>` for string-list or
  allow-multiple string values; or
- order-insensitive unique `Arc<[CompactString]>` for string-set values, with
  one deterministic Slug-native canonical order.

The map contains only effective overrides that differ from the referenced
declaration default. Absence means “use the loaded declaration default”; it
never means empty string/list/set or zero. Declaration kind, default, flag,
multiplicity and repeatability remain source facts demanded when an override
is prepared or a build-setting target is analyzed. Default comparison uses the
effective typed value: an `allow_multiple` string declaration's scalar default
is compared as the singleton sequence containing that string.

Complete canonical labels, kind tags, scopes and typed values participate
structurally in equality, hash, Slug-native canonical bytes, display projection
and DICE invalidation. There is no parallel scope map. Bump the configuration
grammar version rather than interpreting old singleton bytes. Target-to-exec
projection filters `target` scope and carries `universal` scope; `DEFAULT`
follows the already-owned native
`incompatible_exclude_starlark_flags_from_exec_config` and
`experimental_propagate_custom_flag` inputs, failing closed on a nondefault
propagation option until its converter is admitted. `project` scope fails
closed until the later `PROJECT.scl` owner can reset it at project boundaries
while preserving its exec propagation. Keep semantic configuration, display
token, Bazel checksum, ActionKey and REAPI/CAS digest domains distinct.

The prototype has no migration obligation: remove the singleton type and
rewrite its direct callers in the owning implementation packet. Add no
compatibility shim, parallel map, joined-text digest or label string repair.

### 3. Typed effective build-setting values

A configured build-setting target loads its definition through the existing
Root/Canonical package owner and computes:

1. the map override when the canonical label and kind agree; otherwise
2. the declaration's effective default, including the singleton-list wrapper
   for an `allow_multiple` string setting.

That typed value supplies `ctx.build_setting_value`. It is separate from the
ordinary user providers returned by the rule implementation, including common
`BuildSettingInfo`-shaped providers. The implementation still runs normally
when its configured target is requested; consumers may depend on its returned
providers only through ordinary configured edges.

Generalize the existing test/transition input API to the typed map. Transition
outputs must be canonicalized against loaded declarations before child
configured keys are built. A newly introduced nondefault output also resolves
the target declaration's magic scope before the child key is published. A
value equal to the default removes the map row and its scope; unrelated rows
survive. Wrong kinds, missing declarations, non-setting targets, duplicate
canonical outputs, unsupported scope and unsupported split shapes fail closed.
Command text conversion and flag occurrence precedence remain category 3 and
are not admitted here.

### 4. Configured condition ownership

Add one analysis-owned DICE condition key identified by workspace plus the
fully configured canonical `config_setting` target. It loads the semantic
declaration and all referenced setting/native/constraint prerequisites, then
returns a compact typed result: match, no-match with structural discriminator,
or semantic error. It emits no ordinary provider and stores no source route,
query row or evaluator value.

Matching is the conjunction of independently typed criteria:

- `flag_values` converts each expected string with the referenced build-setting
  definition and compares against the override-or-default effective value;
- `values` and `define_values` use the existing native-option metadata and
  converters rather than a second option parser;
- `constraint_values` consumes a separately supplied configured target-platform
  fact once category 4 owns target-platform selection.

The first implementation packets must admit scalar/list/set `flag_values` and
native/define criteria or explicitly fail closed per nonempty unsupported
category. Constraint-only or mixed constraint conditions remain deferred until
the configured target-platform owner exists. Alias keys, feature flags,
flag aliases, config-setting groups and label-typed build settings remain
unsupported unless a later packet names their exact owner and evidence.

Condition equality includes the complete declaration, referenced definition
shape, effective typed values and target-platform input when present. No
condition reparses source or scans configuration display bytes. Legacy and
observed keys share the pure matcher and differ only in their established
input carrier/event frontier.

### 5. Shared configurable-value resolution

Add one pure recursive resolver over `CoercedAttributeValue`, parameterized by
the configured condition results for its selector keys. It preserves literal,
selector, concatenation, branch/default and collection kind until conditions
are known, then produces one typed resolved value or a structural ambiguity/
missing-condition error.

The resolver implements Bazel's direct-condition rules for the admitted
surface: default only when no explicit branch matches, equal resolved values
may collapse, a strictly more specialized direct config setting wins, and
otherwise multiple matches are ambiguous. It never treats selector keys as
ordinary branch-value dependencies, stringifies values, or chooses first by
map order.

One configured-target preparation owner batches the distinct
`$config_dependencies`, demands each condition through DICE, and reuses the
result map for every configurable attribute. Ordinary dependency edges are
built from resolved branch values only. Native `toolchain.target_settings`
first uses that resolver for its own configurable label-list expression, then
demands every resolved condition label through the same condition owner and
requires each to match. It receives no special parser or matcher.
Config-setting groups, aliases, direct constraint-value selector keys and
constraint/platform specialization join only through later
provider-independent eligibility packets.

## Bounded implementation sequence

Do not combine this architecture into one unreviewable writer. Run these
packets in order, preserving the shared types above:

1. complete loading build-setting/config-setting declarations, including
   `config.string_set`, typed defaults, canonical dependencies and derived
   query projection;
2. replace the singleton configuration slot with the typed scoped-override map
   and migrate default/explicit/scope/exec/equality/hash/canonical-byte
   consumers;
3. generalize configured build-setting leaf evaluation and typed transition
   output application;
4. add direct native/define/flag configured condition matching for all five
   build-setting kinds, with unsupported constraint/alias/group boundaries;
5. add batched configured-condition preparation and shared selector/
   concatenation resolution, then use it for ordinary attributes and
   `toolchain.target_settings`; and
6. only after all five are accepted, schedule contextual command Starlark
   flags and extra-registration overlays.

Each implementation manifest must name exact files, caps, evidence and stop
conditions. A packet may split a step further after preflight, but may not add
another retained setting value, condition matcher or selector resolver.

## Proof obligations for the implementation sequence

Across the five implementation packets, prove:

- all five definition kinds, type-correct defaults including signed-32-bit
  integer boundaries, flag/multiple/repeatable constraints, canonical labels,
  repository identity, explicit/default scope observation and wrong-shape
  failures;
- complete config-setting field order/provenance/dependencies, canonical
  duplicate and non-setting-key rejection, and configured all-empty failure;
- singleton removal and multi-label/multi-kind/multi-scope map equality, hash,
  canonical bytes, default-row-plus-scope elision, exact admitted
  target-to-exec filtering and A/B/A restoration;
- arbitrary-precision integer override plus signed-32-bit integer default,
  Boolean, scalar string, allow-multiple string, ordered string-list and
  canonical string-set effective values;
- default/explicit/transition/remove/restore lifecycle, `allow_multiple`
  singleton-default/list behavior, unrelated-row preservation, wrong-kind and
  missing-declaration failures;
- config-setting match/no-match/error for default and override values, list/set
  single-element semantics, native/define conjunction and mixed unsupported
  boundaries in legacy and observed paths;
- direct select, default, concatenation, equal-value multiple match,
  specialization, ambiguity, missing condition and branch-only dependency
  behavior; and
- cancellation publishes neither partial configuration, condition map nor
  configured result, with no lock held across DICE computation.

## Compatibility classification

- **Exact:** named Bazel 9.2 build-setting constructors and parameter
  validation; typed defaults/effective values; `DEFAULT`, `universal` and
  `target` scope identity and exec propagation under the admitted native-option
  values; admitted native `config_setting` declaration and matching behavior;
  default row/scope
  elision; selector/default/equal-value/specialization/ambiguity semantics
  under the cited sources and accepted evidence.
- **Slug-native:** Rust enum/container layout, deterministic canonical order
  for otherwise order-insensitive string-set identity, canonical configuration
  byte grammar and projection, error wording not pinned by an oracle, DICE key
  decomposition and memory accounting.
- **Unsupported/deferred:** command Starlark flag parsing, repeat/precedence
  from real CLI occurrences, `project`-scope `PROJECT.scl` boundary resets,
  flag aliases, feature flags, label/label-list build settings,
  config-setting groups, configured aliases, target-platform constraint
  matching, exact Bazel configuration checksum/output bytes and provider
  payload generalization.

## Ownership, lifetime and memory

Loading definitions and native predicates remain package DICE memory. Typed
nondefault overrides remain configuration memory. Condition results belong to
configured analysis DICE keys. Resolved attribute collections belong to the
configured target result only when otherwise needed there; match batching and
conversion buffers are request/compute scratch.

Use `Arc` slices, `CompactString`, canonical labels, `SmallMap`/`SmallSet`,
`Dupe`, `Allocative` and the existing `num-bigint` support. Do not add a global
interner, mutable service store, query cache, evaluator heap, text hash, second
native option registry or per-builtin provider struct. No lock may span
`ctx.compute`.

## Allowlist and caps

This is a zero-Rust architecture packet. Writable files:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
2. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`.

Cap: 1,100 net documentation lines. No source, test, fixture, oracle, Cargo,
lockfile, BUILD, Zabel or routing-log file is admitted.

## Validation

1. canonical/current-packet ID and predecessor agreement;
2. targeted pinned Bazel 9.2 source/test anchors for every admitted type and
   matcher rule;
3. clean Zabel commit and guidance-only claim check;
4. allowlist/cap, structure, archive-baseline and `git diff --check` gates; and
5. independent architecture/DICE/retained-representation review before
   scheduling implementation step 1.

## Stops

STOP and `REPLAN` for a copied default inside configuration identity; a second
setting map, parallel scope map, matcher, selector evaluator or native-option
converter; flattened selectors; text-only typed values; i32-narrowed
command/transition integers or widened integer defaults; omitted scope
identity; orderful set equality; evaluator-heap
retention; query-owned configuration semantics;
condition matching without configured prerequisites; command parsing or
platform/toolchain selection in this packet; provider payload work; Rust rule
control flow for BCR rules or `cc_internal`; `cc_common`-specific parsing;
Zabel authority; a lock across DICE; files outside the allowlist; or cap
overflow.
