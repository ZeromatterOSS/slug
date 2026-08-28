# Current Slug V2 Packet

Packet: `WP-4-5-7A-native-toolchain-declaration-semantics`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `1f0b396cd`.

Result: make the loading-owned native `toolchain()` declaration retain its
complete configured-semantic input, including configurable `target_settings`,
and make the existing marker-only configured consumer reject every newly
represented non-default case. Add no matching, selection, command-option or
provider breadth.

## Immediate predecessor

Commit `1f0b396cd` accepts the expanded-registration consumer cutover. Analysis
now demands execution-platform expansion before toolchain expansion whenever
the exact zero-requirement/zero-local-declaration bypass does not apply, carries
canonical labels through the full-identity package closure, and no longer
parses raw root MODULE registrations. Root/nonroot same-path packages remain
distinct and the accepted marker-leaf selection behavior is unchanged.

The independently reviewed successor architecture freezes this order:

1. complete loading-owned native toolchain declaration semantics;
2. general typed build-setting/config-condition identity and matching;
3. contextual command build-setting and extra-registration overlays;
4. provider-independent configured alias/constraint/settings eligibility and
   selection;
5. one V2-owned recursive analysis-value/provider representation serving
   arbitrary `ToolchainInfo`, user providers and later host-builtin providers;
   and
6. selected implementation analysis under the exec configuration plus the
   exact `ctx.toolchains` payload cutover.

Only step 1 is active.

## Learned facts and research basis

Pinned Bazel 9.2 at commit
`8220c6198837d5c13d53fea211cf3282aa12408a` remains authority:

- `ToolchainRule` declares mandatory `toolchain_type` and `toolchain`, plus
  `exec_compatible_with`, `target_compatible_with`,
  `use_target_platform_constraints` and configurable `target_settings`;
- `RegisteredToolchainsFunction` consumes the configured native declaration
  and validates `target_settings` before candidate selection;
- `SingleToolchainResolutionFunction` consumes the declaration's target and
  execution compatibility only after those configured facts exist; and
- `RegisteredToolchainsFunctionTest#testRegisteredToolchains_targetSetting`
  proves target-setting filtering, while the existing query RuleClass oracle
  proves the complete attribute set's loading-visible defaults and explicit
  values.

Reuse accepted evidence:

- `query-attr-observable-candidates` already discriminates the Bazel 9.2
  loading projection for every native toolchain attribute;
- `rules-rust-073-toolchain-owner` contains the live BCR declaration with
  nonempty target constraints and `target_settings`;
- `toolchain-resolution-first-platform` preserves the existing supported
  marker-only selection behavior; and
- loading/analysis lifecycle tests already own warm equality, cancellation and
  A/B/A scaffolding. Add no oracle unless a concrete loading distinction is
  absent from those owners.

The live Slug audit found that `LoadedPackage.native_attributes` already
retains native RuleClass values in stable slot order and package equality, but
`NativeToolchainTarget::Toolchain` retains only the mandatory labels and
execution constraints. The generic native-override path therefore makes the
remaining attributes query-visible while configured analysis silently ignores
them. Configurable `target_settings` also needs its selector-condition labels
to remain explicit prerequisites rather than a flattened label list.

Clean Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept/test guidance
only. Its package publication retains target constraints, target settings and
the target-platform policy before configured selection, including configurable
target-setting terms. Copy no Zig row layout, arena/store ID, diagnostics,
scheduler or compatibility claim. Bazel 9.2 alone decides behavior.

## Decision and ownership

### One semantic native declaration

Make the loading-owned native target the source of configured toolchain facts,
not a query-only override side channel. The retained declaration must contain:

- canonical toolchain-type and implementation labels;
- ordered execution- and target-constraint label slices;
- the Boolean `use_target_platform_constraints` policy; and
- the original coerced configurable label-list expression for
  `target_settings`, including concatenation, selector branches/default and
  canonical condition labels.

The existing `NativeRuleAttributes` remains the derived RuleClass/query view.
Do not create a second retained settings map. Direct arguments and keyword
attributes must lower once into the semantic declaration, then project stable
native slots and provenance from that value. Selector condition labels also
populate the existing `$config_dependencies` slot so later configured analysis
can demand them without reparsing a Starlark value or scanning source.

Defaults are empty constraint slices, `false`, and an empty target-settings
expression. Preserve explicit/default provenance in the native attribute view,
even when an explicit value equals its default. Reject wrong types and duplicate
arguments during BUILD evaluation with the existing native-call error owner.

The retained declaration, its compact strings/labels and the derived native
attribute row are DICE-retained package semantic memory. Coercion buffers are
evaluation scratch. Retain no `Value`, evaluator heap, mapping, source route,
command flag or configured result in the declaration.

### Existing configured consumer fails closed

The marker-only configured path may continue only when target constraints and
target settings are empty and `use_target_platform_constraints` is false. It
must reject any other represented case before selecting or analyzing the
implementation. Execution constraints keep their accepted behavior.

This guard is deliberately not Bazel matching semantics. It prevents the
newly truthful loading representation from widening the existing supported
surface while later packets add target platforms, settings and exec
configuration. Do not flatten selectors, evaluate config settings, follow an
alias, construct a build-setting value, or admit the live rules_rust toolchain
in this packet.

### Shared category boundary

This packet changes native declaration facts only. It must not extend the
one-field `ToolchainInfo`, add another builtin-specific retained field struct,
or store arbitrary Starlark values. The frozen successor architecture requires
one later V2-owned recursive analysis-value representation for builtin and user
provider fields. `ToolchainInfo`, `BuildSettingInfo`-shaped user providers and
future `cc_common` provider families will use that shared boundary.

The Buck2-derived parser continues to own syntax. BCR Starlark owns every rule
and rule control flow, including `cc_internal`; Rust supplies only generic
evaluator/host ABI capabilities. No Rust C++ parser or rule engine is allowed.

## Proof obligations

Prove all of the following:

- default and explicit native declarations retain every field and exact
  canonical package/repository identity;
- `target_settings = literal + select(...)` preserves term/branch/default
  shape and exposes each condition through `$config_dependencies` without
  flattening selected values;
- changing only target constraints, target-platform policy, a target-setting
  condition or branch changes `LoadedPackage` equality, and A/B/A restores it;
- query-visible RuleClass slots and explicit/default provenance remain equal to
  the accepted Bazel 9.2 oracle;
- the existing empty/default marker declaration still selects identically;
- each nonempty target-constraint/settings case and true target-platform policy
  fails before implementation analysis in legacy and observed paths; and
- cancellation publishes neither a package nor configured result, with no
  lock held across DICE computation.

## Compatibility classification

- **Exact:** native `toolchain()` attribute types/defaults, configurable
  target-settings expression retention, package-context label conversion,
  RuleClass/query projection and configuration-dependency discovery under the
  cited Bazel 9.2 evidence.
- **Slug-native:** Rust enum/expression layout, `Arc` ownership, compact
  containers, equality/hash implementation and fail-closed diagnostic wording.
- **Unsupported/deferred:** configured target/settings matching,
  `use_target_platform_constraints` selection, aliases/custom providers,
  arbitrary `ToolchainInfo`, Starlark build-setting flags, extra registration
  options, exec-configuration implementation analysis and live rules_rust
  configured/action behavior.

## Request, revision and memory behavior

No command or request input changes. Workspace source observation, canonical
route/mapping ownership and final epoch validation remain with the existing
loading keys. Overlapping requests retain independent DICE transactions.

Package declaration and native attribute rows are DICE-retained semantic
memory with complete structural equality. Temporary coercion, selector walks
and proof buffers drop at evaluation completion or cancellation. Reuse existing
`Arc` slices, `CompactString`, `SmallMap`, `Dupe` and `Allocative`; add no
interner, global cache, `HashMap`, service store or new dependency. No lock may
span `ctx.compute`.

## Allowlist, complexity and caps

Production:

1. `app/slug_loading_v2/src/package.rs`
2. `app/slug_analysis_v2/src/dice.rs`

Proof:

3. `app/slug_loading_v2/tests/build_file_loading.rs`
4. `app/slug_loading_v2/tests/bzl_invalidation.rs`
5. `app/slug_loading_v2/src/host_package_inventory_tests.rs`
6. `app/slug_analysis_v2/tests/starlark_rule.rs`

The existing canonical-package proof exhaustively destructures the native
toolchain declaration and is the natural discriminator for full repository
identity in every newly retained label category. Its admission is proof-only;
no other loading source module or behavior is widened.

No configuration, command, core, Bzlmod, query, identity, build-api provider,
fixture, oracle, Cargo, BUILD, Zabel or plan file is admitted after this
scheduling commit. Caps: 450 net production lines, 650 net proof lines, 1,100
total; no new or materially rewritten function over 120 lines.

`package.rs` and `dice.rs` exceed the 2,000-line review trigger but remain the
current package-lowering and configured-analysis owners. Keep coercion in a
bounded helper and selection rejection in the existing validator. STOP if the
change requires a second value representation or another responsibility.

## Validation

Run serially:

1. focused declaration/default/selector/provenance and fail-closed tests;
2. complete `slug_loading_v2` and `slug_analysis_v2` suites;
3. direct `slug_query_v2` and `slug_bzlmod_v2` dependent suites;
4. existing `query-attr-observable-candidates` only if code-local proof cannot
   establish unchanged accepted output;
5. `cargo fmt --all --check`, allowlist/cap/function checks,
   `git diff --check`, packet/canonical ID agreement and archive status against
   its recorded three-file baseline; and
6. independent terminal review before acceptance.

## Stops

STOP and `REPLAN` for configured condition evaluation or selection; command
flag/extra-registration parsing; alias/custom-provider behavior; arbitrary
provider-value retention; evaluator-heap retention; flattening a configurable
target-settings expression; analysis-side label repair/source discovery; a
second native attribute store; a new parser; Rust ownership of BCR rules or
`cc_internal`; a C++ rule engine; Zabel treated as authority; a lock across
DICE; files outside the allowlist; or cap overflow.
