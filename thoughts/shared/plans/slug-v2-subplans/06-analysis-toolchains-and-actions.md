# Stage 6: Analysis, Toolchains, and Actions

## Goal

Analyze configured targets with Bazel-compatible providers, depsets,
configuration transitions, toolchain resolution, and action declarations.

## Scope

- configured target keys and analysis DICE keys;
- user-defined providers and native providers;
- depset semantics and validation;
- `ctx`, `ctx.attr`, `ctx.files`, `ctx.actions`, outputs, runfiles, and
  default providers;
- configuration fragments, build settings, and transitions;
- platform and toolchain resolution;
- aspects and aspect propagation after base analysis is stable.

## V1 Extraction Candidates

- depset/provider tests from
  `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/depset.rs`
  and
  `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/provider/collection.rs`;
- `rule(implementation=...)` tests from
  `slug-v1-archive:app/slug_interpreter_for_build_tests/src/tests.rs`, with
  implementation orientation from
  `slug-v1-archive:app/slug_interpreter_for_build/src/rule.rs`;
- selected `cc_common` and provider surfaces from
  `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/cc_common.rs`,
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/provider.rs`,
  and
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/provider/collection.rs`;
- action declaration plumbing from
  `slug-v1-archive:app/slug_build_api/src/actions/registry.rs` and
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/context.rs`,
  only after Stage 3 path semantics are clean;
- shared-DAG design and traversal from
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/nested_set.rs`,
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/transitive_set/traversal.rs`,
  and
  `slug-v1-archive:thoughts/shared/plans/slug-bazel-subplans/54-depset-transitive-set-shared-core.md`.

These paths are absent from the active clean root. Inspect them with
`git show slug-v1-archive:<path>` or an external archive worktree; do not search
for or import them from the active root. Use the matching
[Stage 9 extraction-ledger](./09-v1-extraction-ledger.md) row to choose the
import mode, oracle, and validation.

## Bazel Oracle Anchors

- `ConfiguredTargetFunction.java` for configured-target evaluation.
- `StarlarkRuleConfiguredTargetUtil.java` and
  `StarlarkRuleClassFunctions.java` for rule implementation and rule
  definition behavior.
- `RuleContext.java`, `ConfiguredTargetFactory.java`, and provider classes for
  `ctx` and provider semantics.
- `ToolchainResolutionFunction.java`, `SingleToolchainResolutionFunction.java`,
  `PlatformFunction.java`, and `PlatformKeys.java` for platform/toolchain
  selection.
- `AspectFunction.java` and `ToplevelStarlarkAspectFunction.java` for aspect
  propagation after base analysis is stable.

Migrate focused themes from Bazel 9.2.0 tests including
`RuleConfiguredTargetTest`, `StarlarkRuleContextTest`,
`StarlarkRuleClassFunctionsTest`,
`StarlarkRuleImplementationFunctionsTest`, `DepsetTest`, and the owning
platform/toolchain tests. Every fixture records its exact class/method in the
Stage 1 provenance manifest.

## Current Priority: Analysis Graph Integration Gate

Stage 6 is the current implementation owner. Existing provider/depset/action
helpers and the `analyze_loaded_rule` first-rule slice are useful scaffolding,
but do not constitute configured-target analysis until this gate passes.

1. `ConfiguredTargetKey` is a real DICE key over a Stage 3 label,
   configuration, transition inputs, repository mapping, toolchain/platform
   policy, and the loaded target revision.
2. Its computation obtains the loaded package from the shared Stage 2 graph,
   resolves configured attributes, and recursively computes configured
   dependencies. Use DICE parallelism such as `try_compute_join` for
   independent edges while preserving deterministic Bazel ordering.
3. The rule implementation runs through the real analysis registry/context.
   `ctx.attr`, files, executables, configuration, toolchains, and dependency
   provider collections are prepared inputs; Starlark-visible getters perform
   no filesystem or graph discovery.
4. The returned provider collection is authoritative. Do not synthesize
   `DefaultInfo` from declared outputs or infer providers from action side
   effects. Validate Bazel's required-provider and duplicate-provider errors.
5. Action declarations are registered during that rule evaluation and retained
   in `AnalysisResult`. Stage 6 computes deterministic action ownership and
   conflicts but performs no execution.
6. `cquery` reads these configured-target results and `aquery` reads these exact
   action objects. Separate command-only mock graphs are forbidden.
7. Same-daemon dependency, `.bzl`, configuration, toolchain, and repository
   mapping edits invalidate through named DICE dependencies, including create
   and delete transitions.

The current single-rule path may be rewritten to satisfy this gate. Preserve
focused behavior tests, not accidental scaffold interfaces.

### Buck2 and V1 reuse anchors

Before implementing this gate, inspect Buck2 commit
`088c75c7e36805df99c3de29062baa95db700b8b` at:

- `../buck2/app/buck2_analysis/src/analysis/calculation.rs` for the analysis
  key and recursive dependency-compute pattern;
- `../buck2/app/buck2_analysis/src/analysis/env.rs` and
  `../buck2/app/buck2_build_api/src/analysis/registry.rs` for analysis context
  and action/provider registry ownership; and
- `../buck2/app/buck2_interpreter_for_build/` for attribute coercion,
  interning, and prepared Starlark values.

Inspect the equivalent V1 archive paths as behavior/test sources where Stage 9
records them. Port the DICE/registry/compact-data patterns behind V2 Bazel
labels, configurations, providers, and output paths; reject cells, Buck labels,
Buck configurations, and Buck output semantics. Hot graph structures require
the repo utility audit: prefer retained `SmallMap`/`SmallSet`, Fx hashing,
`Hashed`, `ArcStr`/`ThinArcStr`, `Dupe`, and `Allocative` where their measured
shape fits instead of default owned `String`, `Vec`, or std hash collections.

## Implementation Slices

### 6.1 Configured Target Key and Configuration

- Define Bazel-shaped configured-target keys using Stage 3 labels plus
  configuration hash.
- Implement target, exec, and host-like transition policy only where Bazel 9
  still exposes it.
- Build setting values, command-line flags, and transition outputs are DICE
  inputs, not global process state.
- Initial modules: `app/slug_analysis_v2/src/{key.rs,result.rs,dice.rs,configured_target.rs}`
  and `app/slug_build_api_v2/src/providers/`.
- Use archived V1 analysis code such as
  `slug-v1-archive:app/slug_analysis/src/analysis/calculation.rs` and
  `slug-v1-archive:app/slug_analysis/src/analysis/toolchain_resolution.rs` only
  as pattern sources; do not port Buck labels or V1 configuration identity.
- Implement this as the DICE computation defined by the current-priority gate;
  a key-shaped serializable struct without `Key::compute` integration is only
  substrate.

### 6.2 Providers and Depsets

- Implement user providers, native providers, `DefaultInfo`, `OutputGroupInfo`,
  `RunEnvironmentInfo`, `FilesToRunProvider`, `PlatformInfo`, and the provider
  collection API needed by the first rulesets.
- Implement Bazel `depset` order, validation, flattening, equality constraints,
  and transitive nesting without implicit `transitive_set` coercion.
- Store the transitive structure as an immutable shared nested DAG: composition
  must not recursively copy children, and flattening is an explicit consuming
  operation. Selectively extract the archived shared traversal/depth lessons
  named above while keeping the Bazel depset facade V2-owned.
- Initial modules: `app/slug_build_api_v2/src/{ctx.rs,attrs.rs,providers.rs,runfiles.rs,depset.rs}`.
- Extraction candidates are the archived V1 depset/provider tests and
  implementation paths named above, but public types must be Bazel-shaped and
  Stage-3 label based.

### 6.3 Rule Implementation Context

- Implement `ctx.attr`, `ctx.file`, `ctx.files`, `ctx.executable`,
  `ctx.label`, `ctx.outputs`, `ctx.actions`, `ctx.fragments`, `ctx.toolchains`,
  `ctx.exec_groups`, `ctx.var`, `ctx.expand_location`, and
  `ctx.resolve_command` in priority order driven by fixtures.
- Starlark-visible methods that need prepared values must receive them from
  analysis inputs rather than doing filesystem or graph discovery.

### 6.4 Action Declaration IR

- Define an action IR that is independent of executor choice.
- Actions include mnemonic, argv, env, execution requirements, input digests,
  tools, paramfiles, output declarations, progress message, and exec
  properties.
- Stage 7 consumes this IR to build REAPI commands.
- Initial modules:
  `app/slug_build_api_v2/src/actions/{registry.rs,spec.rs,ctx_actions.rs,reapi_projection.rs}`.
- Implement `declare_file`, `declare_directory`, `declare_symlink`, `write`,
  `write_json`, `expand_template`, `run`, `run_shell`, `args`, output conflict
  checks, exec groups, toolchain action contexts, and action exec properties.
- Stage 6 emits deterministic action descriptions only; it does not execute
  actions or decide CAS/AC policy.

### 6.5 Toolchains and Platforms

- Implement constraint values/settings, platform target analysis, registered
  toolchains, toolchain type resolution, execution platform filtering, exec
  groups, and per-action exec properties.
- Registration order must come from Stage 5 bzlmod outputs and command-line
  flags in Bazel order.
- Initial modules:
  `app/slug_analysis_v2/src/toolchains/{registered.rs,resolution.rs,context.rs,exec_groups.rs,platform_constraints.rs}`.
- Replace V1 process-global toolchain state with DICE keys for registered
  toolchains, platform aliases, host fallback, optional and mandatory
  toolchains, target settings, default constraints, and per-exec-group contexts.

### 6.6 Aspects After Base Analysis

- Defer aspects until custom rules, providers, depsets, and toolchains are
  stable.
- First aspect fixture should cover attr propagation and provider requirements;
  advanced incrementality can be a later Stage 8/9 extraction.
- Initial modules: `app/slug_analysis_v2/src/{aspect_key.rs,aspect_analysis.rs}`.
- Model aspect dependency edges explicitly; do not smuggle aspect state through
  the configured-target cache.

## Exact Test Criteria

- `custom-rule-analysis-basic` fixture returns providers matching Bazel for:
  user provider, `DefaultInfo.files`, runfiles, and an output group.
- `ctx-attrs-files-executable` fixture compares `ctx.attr`, `ctx.file`,
  `ctx.files`, `ctx.executable`, `ctx.label`, and `ctx.toolchains`.
- `default-info-runfiles-executable` and `provider-output-group-basic` compare
  normalized provider keys, files, runfiles, and output groups.
- `depset-orders-and-rejections` fixture covers all Bazel orders, incompatible order
  failures, nested depsets, duplicate handling, and flattening order.
- `depset-orders-and-rejections` fixture compares `to_list()` order and
  rejection diagnostics.
- A focused structural test proves that combining nested depsets preserves
  shared child-node identity and does not flatten or recursively clone the DAG.
- `actions-api-basic` fixture declares write, run, run_shell, symlink, and
  expand_template actions and compares action IR to Bazel's normalized
  `ActionGraphContainer`.
- `action-declare-file-package-boundary`, `action-run-shell-basic`,
  `action-run-tool-exec-cfg`, and `action-conflicting-output` compare output
  paths, mnemonic, argv, env, tools, inputs, outputs, and diagnostics.
- `toolchain-resolution-first-platform`, `toolchain-resolution-host-platform`,
  `toolchain-resolution-platform-alias`, and `toolchain-mandatory-missing`
  compare selected execution platform, resolved toolchain labels, events, and
  missing-toolchain diagnostics.
- `exec-groups-action-platform` proves per-action exec-group platform selection.
- `transition-basic` fixture executes a user transition and proves outgoing
  configuration affects a dependency.
- `aspect-provider-propagation` runs only after base analysis is stable and
  compares aspect-produced providers and actions.
- Registered toolchain and platform edits invalidate through named DICE keys.
- `rg -n "std::fs|process-global|CellResolver|buck-out" <v2-analysis-crates>`
  returns no semantic production shortcuts.

## Acceptance Criteria

- Custom Starlark rule fixtures produce the same providers/actions as Bazel.
- Depset construction is cheap shared-DAG composition; flattening/copying is
  confined to explicit Bazel-visible operations.
- Toolchain and platform fixtures match Bazel for focused public examples.
- Action declarations produce REAPI-ready command/input/output structures.
- No analysis shortcut depends on Buck cells or direct filesystem scans outside
  DICE-tracked inputs.
- A multi-target fixture proves recursive dependency analysis, provider flow,
  shared subdependency reuse, and deterministic action ownership in one
  same-daemon DICE graph.
- `AnalysisResult` contains the providers actually returned by each rule and
  the actions actually registered during its implementation; no output-derived
  provider synthesis or command-specific mock graph remains.
- Stage 8 `cquery` and `aquery` consume this graph without re-evaluating rules
  in separate command-owned state.

## Validation

```bash
cargo test -p slug_analysis_v2
cargo test -p slug_build_api_v2 depset
cargo test -p slug_analysis_v2 toolchain
slug-v2-oracle run --fixture custom-rule-analysis-basic --compare providers,actions,outputs,diagnostics
slug-v2-oracle run --fixture depset-orders-and-rejections --compare stdout,stderr
slug-v2-oracle run --fixture actions-api-basic
slug-v2-oracle run --fixture toolchain-resolution-first-platform --compare providers,events
slug-v2-oracle run --fixture exec-groups-action-platform --compare actions,providers
slug-v2-oracle run --fixture transition-basic
slug-v2-oracle run --fixture aspect-provider-propagation --compare providers,actions
```

## Checkpoint Evidence

- 2026-06-27 Stage 6.2 build API substrate: added `slug_build_api_v2` with
  Bazel-shaped depset order/compatibility/depth checks plus `DefaultInfo`,
  `OutputGroupInfo`, `RunEnvironmentInfo`, `FilesToRunProvider`, `PlatformInfo`,
  user providers, and duplicate-provider collection validation. Added
  `custom-rule-analysis-basic` and `depset-orders-and-rejections` oracle
  fixtures; expected Bazel outputs were placeholders at this checkpoint because
  the local `bazel.exe` could not fetch Bazel under the restricted
  network/proxy environment. The depset fixture was refreshed with Bazel 9.1.1
  in the 2026-07-01 checkpoint below.
  Validation: `cargo test -p slug_build_api_v2`; `cargo test -p
  slug_build_api_v2 depset`; `py -3 -B tools/v2_oracle list`; `rg -n
  "std::fs|process-global|CellResolver|buck-out" app/slug_build_api_v2`
  returned no matches. Stage 6 commands requiring `slug_analysis_v2` or real
  configured-target/action/toolchain execution are not yet meaningful until the
  analysis crate and evaluator slices land.
- 2026-06-27 Stage 6.1 configured-target key substrate: added
  `slug_analysis_v2` with configuration checksums/kinds, configured target
  keys based on Stage 3 canonical labels, transition edges, DICE key-shaped
  semantic input digests for command-line/build-setting/repository/toolchain
  policy, and an `AnalysisResult` shell over the V2 provider collection. The
  DICE scaffold introduces no locks or process-global semantic state.
  Validation: `cargo test -p slug_analysis_v2`; `cargo test -p
  slug_analysis_v2 toolchain` currently has zero matching tests; `py -3 -B
  tools/v2_oracle list`; `rg -n
  "std::fs|process-global|CellResolver|buck-out" app/slug_analysis_v2
  app/slug_build_api_v2` returned no matches. Real configured-target
  evaluation, action IR, and toolchain resolution remain open Stage 6 slices.
- 2026-06-27 Stage 6.4 action IR substrate: added
  `slug_build_api_v2::actions` with action specs, deterministic env/exec
  property maps, declared output kinds, paramfile records, registry output
  conflict checks, ctx-style declaration helpers for write/write_json/run/
  run_shell/symlink/expand_template, and a REAPI command projection that
  separates output files/directories without executing actions. Wired
  `AnalysisResult` to carry action specs and added the `actions-api-basic`
  oracle fixture scaffold. Validation: `cargo test -p slug_build_api_v2`;
  `cargo test -p slug_analysis_v2`; `py -3 -B tools/v2_oracle list`; `rg -n
  "std::fs|process-global|CellResolver|buck-out" app/slug_analysis_v2
  app/slug_build_api_v2` returned no matches. The subplan oracle command
  `slug-v2-oracle run --fixture actions-api-basic` is still skipped because
  Stage 2 `build` is a placeholder and no configured-target evaluator/aquery
  projection exists yet.
- 2026-06-27 Stage 6.5 toolchain/platform substrate: added explicit
  platform constraint sets, execution platforms, registered toolchain inputs,
  registration DICE key digests, first-compatible-platform resolution events,
  mandatory-missing diagnostics, resolved toolchain contexts, and exec-group
  property/context containers under `slug_analysis_v2::toolchains`. Added
  `toolchain-resolution-first-platform` and `exec-groups-action-platform`
  oracle fixture scaffolds. Validation: `cargo test -p slug_analysis_v2
  toolchain`; `cargo test -p slug_analysis_v2`; `py -3 -B tools/v2_oracle
  list`; `rg -n "std::fs|process-global|CellResolver|buck-out"
  app/slug_analysis_v2 app/slug_build_api_v2` returned no matches. Oracle
  fixture execution remains skipped until Stage 2 `build` grows real analysis
  and toolchain evaluation.
- 2026-06-27 Stage 6.3 rule context substrate: added `slug_build_api_v2`
  attr maps, prepared `RuleContext` accessors for `ctx.attr`, `ctx.file`,
  `ctx.files`, `ctx.executable`, `ctx.label`, `ctx.outputs`, `ctx.actions`,
  `ctx.fragments`, `ctx.toolchains`, `ctx.exec_groups`, `ctx.var`,
  `expand_location`, and `resolve_command`, plus a `RunfilesBuilder` for
  DefaultInfo runfiles. Added `ctx-attrs-files-executable`,
  `default-info-runfiles-executable`, and `provider-output-group-basic` oracle
  fixture scaffolds. Validation: `cargo test -p slug_build_api_v2` (rerun with
  `CARGO_BUILD_JOBS=1` after a Windows object-file lock); `cargo test -p
  slug_analysis_v2`; `py -3 -B tools/v2_oracle list`; `rg -n
  "std::fs|process-global|CellResolver|buck-out" app/slug_analysis_v2
  app/slug_build_api_v2` returned no matches. Fixture execution remains
  skipped until Stage 2 `build` has real configured-target analysis.
- 2026-07-01 Stage 6 depset oracle refresh: regenerated
  `depset-orders-and-rejections` expected output with Bazel 9.1.1 now that the
  local oracle is available. Strengthened the fixture to manifest-compare
  `bazel-bin/probe.txt`, pinning the `depset.to_list()` output bytes in
  addition to the incompatible `preorder`/`postorder` diagnostic.
  Validation: `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run
  --fixture depset-orders-and-rejections --tool bazel --bazel
  <Bazel-9.1.1-binary> --timeout 120 --update-expected`;
  same command without `--update-expected`; `CARGO_TARGET_DIR=.codex-cargo-target
  CARGO_BUILD_JOBS=1 cargo test -p slug_build_api_v2 depset`; `py -3 -B -m
  tools.v2_oracle list`; bundled `pytest -q -p no:cacheprovider
  tests/v2_oracle/test_v2_oracle.py`. Toolchain and transition/aspect fixtures
  still need either Bazel expected-output refreshes or full V2 configured-target
  analysis before Slug-side oracle comparison is meaningful.
- 2026-07-02 Stage 6 custom/context/action oracle refresh: regenerated Bazel
  9.1.1 expected output for `custom-rule-analysis-basic`,
  `ctx-attrs-files-executable`, `default-info-runfiles-executable`, and
  `provider-output-group-basic`. Adjusted `actions-api-basic` to use Bazel
  aquery summary rather than build execution, matching the Stage 6 criterion to
  compare action declaration IR without depending on host shell execution; the
  fixture still declares write, run, run_shell, symlink, and expand_template
  actions. Bazel 9 also proved `ctx.actions.symlink(target_file = ...)` requires
  a file/directory output, so the fixture now declares the symlink output as a
  file while preserving the symlink action.
  Validation: Bazel 9.1.1 `--update-expected` and no-update oracle runs for all
  five fixtures; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo
  test -p slug_build_api_v2`; `py -3 -B -m tools.v2_oracle list`; bundled
  `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`.
- 2026-07-14 Stage 6 shared-depset correction: replaced the recursive
  by-value `Depset<T>` storage with immutable `Arc` nodes and immutable child
  slices. Composition now retains child identity and flattening remains the
  only operation that walks the DAG. This follows the archived V1 nested-set
  traversal lesson while retaining a V2 Bazel-shaped facade, and uses the
  retained Buck2 `FxHashSet` traversal pattern rather than a new default
  hasher. The focused structural regression proves two parents retain the
  same child node; Bazel order, deduplication, compatibility, and depth tests
  remain the contract.
- 2026-07-14 Stage 6 first-rule-analysis packet: `rule(implementation=...)`
  now retains its frozen Starlark closure through package loading. A V2-owned
  evaluator invokes that closure with prepared `ctx.label`,
  `ctx.actions.declare_file`, and `ctx.actions.write` values, turns declared
  outputs into a `DefaultInfo.files` shared depset, and carries the action IR
  into `slug build`. The context state lock is scoped only around synchronous
  action-registry mutations and never crosses DICE or evaluator re-entry.
  The rebuilt `simple-rule-action` smoke reports
  `dice_starlark_rule_analysis`, `analyzed_target_count=1`, and
  `declared_action_count=1`. This advances gate clause 2 but does not execute,
  upload, materialize, or claim oracle parity for the output; those remain
  Stage 7/integration work.

### Reviewed next packet — `WP-6-m2-recursive-custom-analysis` (2026-07-22)

Work packet ID: `WP-6-m2-recursive-custom-analysis`

Owner stage and plan: Stage 6,
`thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`;
consumes the retained workspace transaction and the Stage 4 package-loading
graph through `de835cdc`.

Goal and gate link: replace the direct one-rule analysis shortcut with one real
recursive configured-target DICE key for root-repository custom rules. This is
the first bounded M2 vertical. It must make returned providers and target-local
declared actions authoritative before Stage 8 reads the graph.

Prerequisites and current state:

- `WorkspaceRuntime` owns the retained DICE instance and committed
  file/directory observations;
- `PackageLoadKey` owns BUILD and `.bzl` loading but Starlark rule definitions
  currently discard attribute schemas and invocations discard attribute
  values;
- `analyze_loaded_rule` is called synchronously after package loading,
  synthesizes `DefaultInfo.files` from action outputs, and is not a DICE key;
- `AnalysisDiceInputs` and `ConfiguredTargetDiceKey` are digest-only
  scaffolds, not computations; and
- query/cquery/aquery remain consumers, not permission to create a second
  command-owned graph.

Oracle-first artifact:
`tests/v2_oracle/fixtures/recursive-custom-rule-providers-actions`, generated
and independently rerun with Bazel 9.2.0 at
`8220c6198837d5c13d53fea211cf3282aa12408a`; landed as oracle commit
`9e6a4450`. A shared
`//rules:defs.bzl` defines a string-field provider, a leaf rule, and a parent
rule whose sole schema is `attrs = {"deps": attr.label_list()}`. Separate
`//leaf` and `//parent` packages declare two leaves and a parent with dependency
order `[second, first]`.

The fixture uses Bazel `cquery --output=starlark --starlark:file=...` to report
the configured label, `DefaultInfo.files`, and the qualified custom-provider
field for parent and leaves. It uses `aquery deps(//parent:parent)` to prove
distinct deterministic leaf and parent write actions. Build output or
materialization is not provider evidence. Invoke Bazel normally so the user's
external `~/.bazelrc` may accelerate it; never read, copy, log, or commit that
file or its credentials.

Reuse audit:

- selectively translate Buck2 commit
  `088c75c7e36805df99c3de29062baa95db700b8b`
  `app/buck2_analysis/src/analysis/calculation.rs`: real recursive `Key`
  computation plus ordered `try_compute_join`;
- selectively translate
  `app/buck2_analysis/src/analysis/env.rs`: prepare an immutable dependency
  provider environment before Starlark evaluation, with no graph discovery in
  Starlark getters;
- adopt the ownership boundary, not Buck types, from
  `app/buck2_build_api/src/analysis/registry.rs`: one target-local action
  registry finalized after the rule returns;
- reuse retained `SmallMap`/`SmallSet` or other compact ordered
  `starlark_map` values, immutable shared slices, `Dupe`, `Allocative`, and
  precomputed hashes when their semantics match; do not introduce default
  `HashMap`/owned-string churn in graph values; and
- keep V1
  `slug-v1-archive:app/slug_analysis/src/analysis/calculation.rs` at
  `e218054d4c796655939b968d90208b185decb352` reference-only. Reject Buck cells,
  labels, configurations, output paths, global registries, and command-owned
  analysis graphs.

Reviewed architecture:

1. Delete `AnalysisDiceInputs` and `ConfiguredTargetDiceKey`. Define the one
   production identity
   `ConfiguredTargetAnalysisKey { workspace, configured_target }`. Its compute
   requests `PackageLoadKey`, resolves the target, recursively computes direct
   dependency analysis keys in the same explicitly named root target
   configuration, and evaluates the rule. Loaded revisions flow through DICE
   dependencies, never digest strings.
2. Loading supports exactly
   `rule(implementation=..., attrs={"deps": attr.label_list()})`. Omitted
   `deps` becomes `[]`. Invocation values accept only lists of root
   `//pkg:name` or package-relative `:name` strings and normalize them to
   canonical root labels. Unknown attributes, non-lists, non-string labels,
   missing targets, and external repositories fail deterministically.
   Immutable rule schemas live with frozen rule definitions; immutable
   invocation values live with each package target.
3. Implement one real `DefaultInfo(files=depset(...))` constructor and one
   user-provider constructor with declared string fields. Provider identity is
   structural—`.bzl` label plus exported name—and survives freezing. Prepared
   `ctx.attr.deps` target/provider views support lookup by that constructor.
   Decode the returned rule list into an owned `ProviderCollection` before
   returning from analysis; no DICE result borrows evaluator heaps. Enforce
   duplicate-provider and missing-`DefaultInfo` failures.
4. The rule's returned provider list is authoritative. Declared files come
   from returned `DefaultInfo`; action outputs remain registry facts. Do not
   synthesize one from the other.
5. `AnalysisResult` records ordered direct configured-dependency keys and owns
   only the actions declared by its rule. It never aggregates dependency
   actions; future Stage 8 traversal follows the named dependency keys.
6. `WorkspaceRuntime` computes `ConfiguredTargetAnalysisKey` inside its
   existing transaction. Remove the production direct-helper path. No lock may
   cross DICE computation or Starlark evaluation; the action-state lock is
   limited to synchronous registry mutation.

Exact scope:

- `app/slug_loading_v2` rule/attr/provider loading and focused tests;
- `app/slug_analysis_v2` real key, prepared evaluator, owned result, and tests;
- `app/slug_build_api_v2` only the minimal Starlark depset/provider values
  required by this fixture;
- `app/slug_core_v2/src/runtime/dice.rs` transaction wiring and retained-runtime
  tests;
- the oracle fixture, this owner plan, Stage 1 evidence, Stage 9 ledger, and
  orchestration routing log.

Exclude query implementation, command formatting, execution, external
repositories and mapping, transitions, toolchains, selects, aspects, native
rules, files/executables, general attribute/provider breadth, and new action
kinds.

Implementation and test order:

1. Generate and independently rerun the Bazel 9.2.0 fixture; verify exact
   cquery provider keys/fields and aquery ownership before Rust edits.
2. Add loading schema/invocation regressions and the structural frozen-provider
   identity test.
3. Add the real analysis key and recursive ordered join, prepared dependency
   views, authoritative provider decoding, and target-local registry
   finalization.
4. Replace the core direct call and add an `ActivationTracker` regression by
   concrete analysis-key identity: initial leaf/parent evaluation; no
   activation on identical observations; leaf provider edit reevaluating leaf
   and parent; unrelated-package mutation validating/reusing without rule
   evaluation; declaration-order dependencies; per-key providers and actions.
5. Run the affected crates serially through one Cargo target, then obtain
   Sol-low post-review before commit.

Focused validation:

- Bazel 9.2.0 generation plus no-update rerun of
  `recursive-custom-rule-providers-actions`;
- `CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 -p slug_build_api_v2
  -p slug_analysis_v2 -p slug_core_v2 -p slug_server_v2 -p slug_cli_v2`;
- `cargo fmt --all -- --check`;
- ownership greps for filesystem reads, direct production analysis helpers,
  duplicate configured-target identities, default std hash collections,
  runtime creation, blocking bridges, and locks across DICE/evaluator work;
- `scripts/v2_archive_status.sh` and `git diff --check`.

Evidence and completion boundary: Sol-low accepted this revised architecture
after requiring removal of the parallel digest identity, an exact loading
schema, structural provider identity and owned decoding, target-local action
ownership, ordered dependency keys, and key-specific activation evidence.
Record the generated oracle commit first; after implementation acceptance,
record the exact implementation commit, activation events, utility reuse,
validation, and residuals here and in Stage 9.

Stop on a non-generated/non-9.2 oracle, unsafe provider identity or frozen
ownership, display-string provider keys, a need for general attr coercion or
transitions, graph/filesystem discovery from Starlark, a lock across
`ctx.compute`/ordered join/evaluation, provider/action synthesis, external
repository identity, or any query/command-owned graph. Stage 5 does not block
this root-only packet; non-root labels remain explicit errors until repository
mapping is DICE-owned.

### Recursive custom-rule analysis implementation evidence (2026-07-22)

Implementation commit `4f4599e0` completes
`WP-6-m2-recursive-custom-analysis` against oracle commit `9e6a4450`.
`ConfiguredTargetAnalysisKey { workspace, configured_target }` is now the
single production analysis identity. It consumes `PackageLoadKey`, computes
unique direct dependencies with DICE's parallel join, restores declaration
order, prepares immutable provider views, then synchronously evaluates the
frozen rule. `WorkspaceRuntime` requests that key inside its retained
transaction; the digest-only analysis scaffolds and production direct helper
were removed.

The bounded Starlark surface now preserves exactly the reviewed
`attr.label_list` dependency schema, root/package-relative label normalization,
frozen structural provider identity (`.bzl` label plus exported name),
`DefaultInfo(files=depset(...))`, declared string-field providers, and
target-local actions. Returned providers and `DefaultInfo.files` are
authoritative. `AnalysisResult` owns the ordered direct configured-target keys,
decoded providers, local actions, declared outputs, and diagnostics; it does
not aggregate dependency actions or borrow evaluator heaps. Hot graph values
use `CompactString`, `SmallMap`, `SmallSet`, immutable `Arc` slices, `Dupe`,
and `Allocative` instead of new default hash collections or string-heavy
parallel identities.

Exact `ActivationTracker` multisets establish:

- initial request: both leaves and the parent `Evaluated` exactly once;
- identical observations: no analysis-key activation;
- unrelated-package file creation: both leaves and the parent `Reused`
  exactly once, with no rule evaluation;
- shared provider/rule implementation edit: both leaves and the parent
  `Evaluated` exactly once;
- deleting one leaf declaration: both leaf keys and the dependent parent
  `Evaluated`, producing the missing-target error; and
- recreating the declaration: both leaves `Evaluated`, while the restored
  equal parent result is `Reused`.

Validation passed:
`CARGO_TARGET_DIR=/tmp/slug-m2-analysis-target CARGO_BUILD_JOBS=1 cargo test
-p slug_identity_v2 -p slug_loading_v2 -p slug_build_api_v2
-p slug_analysis_v2 -p slug_core_v2 -p slug_server_v2 -p slug_cli_v2`;
focused exact activation and external-label rejection regressions;
`cargo fmt --all -- --check`; removed-identity/helper, filesystem/runtime,
lock, and default-hash-collection ownership greps; and `git diff --check`.
Sol-low initially requested exact event counts, explicit `@repo`/`@@repo`
rejection tests, and corrected runtime documentation, then returned `ACCEPT`
after all three changes. `scripts/v2_archive_status.sh` still reports its
known environmental `v1-archive` branch absence and broad path-classification
false positives; its immutable `slug-v1-archive` ref and physical-archive
checks pass.

Residual scope remains deliberate: external repositories/mapping,
transitions/configuration selection, general attributes/providers, native
rules, toolchains, query formatting/traversal, execution, and materialization
remain later packets. The migration observer still scans the workspace before
injecting immutable inputs; analysis itself performs no filesystem discovery.

### WP-6-m2 root configured-target command boundary design (2026-08-04)

**Status: REPLAN; run only
`WP-6-m2-root-cquery-label-output-evidence`.** Decide whether the first root-only literal
`cquery` result can consume the existing sole
`ConfiguredTargetAnalysisKey { workspace, configured_target }` and its ordered
dependency results without a second command graph, new configuration
representation, or analysis re-evaluation. Reuse the accepted
`recursive-custom-rule-providers-actions` Bazel 9.2 cquery evidence unless one
literal-label discriminator is genuinely absent. No Rust, test, fixture, or
oracle edit is authorized until reserved Sol review accepts a complete
identity/ownership/error/output/lifecycle boundary.

Live ownership is bounded without a new key: the later command may drive the
existing `RootConfiguredTargetAnalysisKey` directly through
`NativeCommandRoot`. That key already owns root loading, recursive configured
analysis, complete-only equality/validity, typed Needs/errors, and captured
events. Formatting can project the accepted `AnalysisResult` after terminal
selection; no command-owned graph or evaluator call is required.

The accepted Bazel 9.2 fixture nevertheless contains only Starlark-formatted
cquery rows. It does not pin default or explicit label output, configuration
suffix, missing-target failure, or same-server recovery; old literal label rows
are Bazel 9.1.1 and are not authority. Two Terra audits independently found
this evidence gap. Reserved Sol review accepted exactly one isolated retained
Bazel 9.2 sequence: default literal, explicit label literal, missing literal,
then explicit label recovery. Record raw exit/stdout/stderr separately and do
not normalize away configuration identifiers. No analysis-error row is needed
unless later implementation reveals distinct translation.

### Root cquery label-output evidence and configuration-identity REPLAN (2026-08-04)

`WP-6-m2-root-cquery-label-output-evidence` is accepted from an isolated copy
of `recursive-custom-rule-providers-actions` under `/usr/bin/bazel` 9.2.0 and
one retained output base. The exact serial observations were:

1. `cquery //parent:parent` exited 0 and wrote exactly
   `//parent:parent (a7a71fd)\n` to stdout.
2. `cquery //parent:parent --output=label` exited 0 with byte-identical stdout.
3. `cquery //parent:missing` exited 1 with empty stdout and the stable pair
   `ERROR: Skipping '//parent:missing': no such target
   '//parent:missing': target 'missing' not declared in package 'parent'
   defined by <workspace>/parent/BUILD.bazel` and `ERROR: no such target
   '//parent:missing': target 'missing' not declared in package 'parent'
   defined by <workspace>/parent/BUILD.bazel`, followed by the unsuccessful
   completion line.
4. Repeating the explicit-label command in the same server exited 0, restored
   the exact successful stdout, and reported zero newly loaded/configured
   targets.

The successful stderr reported one analyzed target on the cold request and the
ordinary successful completion summary. Default and explicit `label` output
are therefore the same contract; neither offers a configuration-free slice.
The temporary workspace/output base was removed after server shutdown, and no
fixture or generated oracle record changed.

Pinned Bazel 9.2 source explains the suffix.
`LabelAndConfigurationOutputFormatterCallback` formats the label plus
`BuildConfigurationValue.shortId()`; `BuildConfigurationValue` delegates to
`BuildOptions`, whose checksum is the SHA-256 fingerprint of every native
fragment option cache key plus canonical Starlark options and option scopes.
`shortId()` is the first seven hexadecimal characters. Thus `a7a71fd` is
authoritative configured-target identity, not a cquery mnemonic or
formatter-local token.

**Status: REPLAN before Rust.** Slug's `ConfigurationKey` currently validates
and stores an opaque caller-supplied checksum, while both production root
analysis entry points supply the placeholder `first-build`. Slug has no
BuildOptions inventory, native option cache-key serialization, Starlark build
setting/scope input, or authoritative checksum producer. Truncating the current
placeholder or hard-coding `a7a71fd` would be a fixture shim, not Bazel parity.
The existing `RootConfiguredTargetAnalysisKey` remains the accepted later
cquery command root, with no second graph, new cquery DICE key, or evaluator
call, but public formatting and daemon-wire work stay deferred.

Design next only `WP-6-m2-root-configuration-identity-design`. It must decide
whether a bounded exact root target configuration input/checksum owner exists
for the accepted fixture, identify every semantic input and invalidation edge,
and return `REPLAN` rather than approximate or embed Bazel's observed digest.
No Rust, tests, wire schema, fixture, or new oracle command is authorized by
this evidence checkpoint.

### Root configuration identity design result (2026-08-04)

**Status: REPLAN; no implementation exists for this packet.** Pinned Bazel
9.2 source and two live Slug audits prove that even the no-extra-flags target
configuration is not a bounded constant. This historical design originally
counted fourteen native `FragmentOptions` classes. A complete follow-up audit
corrected the Bazel 9.2 registry to seventeen classes; the omitted classes were
coverage, test, and config-feature-flag options. Bazel sorts the registry by
fully qualified class name and hashes
every option cache key—including defaults—in alphabetical option-name order,
then hashes canonical Starlark options and option scopes. The native set spans
platform, shell, core, strict-action-environment, Python, Android, Apple, C++,
Java, J2ObjC, ObjC, and proto options. CPU/host CPU, Apple defaults, target and
host platforms, platform mappings, and platform-selected flags introduce host
and graph inputs before the final configuration key.

Live Slug has no matching producer. `ConfigurationKey` is only an opaque
validated checksum carrier. The retained build root and the legacy one-shot
analysis entry each construct `target:first-build`; recursive analysis then
correctly clones that identity to every dependency. Command normalization and
the daemon wire own only bzlmod policy/environment, lockfile mode, and registry
URLs. `--config` is parse-only, and Slug has no native option inventory,
Starlark build-setting/scope model, platform mapping, transition/toolchain
selection input, or Bazel `OptionsBase.cacheKey()` serialization. Existing
SHA-256 values belong to unrelated bzlmod, repository, or REAPI domains.

A future exact prerequisite is therefore the general Bazel-9 target-option and
configuration-identity substrate, not a line-bounded root checksum key. Once
that substrate exists, a DICE-owned producer may compute a full checksum inside
the command transaction before either root key is constructed. It must reject
every unmodeled configuration-affecting input before analysis and prove native
option, Starlark option/scope, and same-daemon `C0 -> C1 -> C0` identity and
recursive reuse transitions. This packet does not authorize that broad work.

Reserved review accepted the REPLAN and preserved a smaller cquery consumer
that does not display configuration identity: one root literal with
`--output=starlark --starlark:expr=str(target.label)`, driven directly through
the existing `RootConfiguredTargetAnalysisKey`. The old standalone
`cquery-provider-starlark` record is Bazel 9.1.1 and is not authority for this
packet, while the accepted recursive Bazel 9.2 fixture uses a Starlark file and
does not pin the exact expression/lifecycle contract. Run only
`WP-6-m2-root-cquery-starlark-label-evidence` next. Default/explicit `label`
output remains unsupported until the full configuration substrate exists.

### Root cquery Starlark-label evidence (2026-08-04)

`WP-6-m2-root-cquery-starlark-label-evidence` is accepted from an isolated copy
of `recursive-custom-rule-providers-actions` under `/usr/bin/bazel` 9.2.0 and
one retained output base. The exact serial sequence was:

1. `cquery //parent:parent --output=starlark
   --starlark:expr=str(target.label)` exited 0 and wrote exactly the 18 bytes
   `@@//parent:parent\n` (`40402f2f706172656e743a706172656e740a`).
2. The same formatter for `//parent:missing` exited 1 with empty stdout and the
   stable skipping/no-such-target pair naming
   `<workspace>/parent/BUILD.bazel`, followed by
   `ERROR: Build did NOT complete successfully`.
3. Repeating the successful command in the same server exited 0 and returned
   byte-identical stdout. Bazel reported warm analysis progress through four
   packages/six configured targets and then five packages/nine configured
   targets before successful completion.

The output exposes neither a configuration checksum nor a mnemonic and agrees
with the canonical-label component of the frozen Bazel 9.2 Starlark-file row.
The retained server shut down successfully, the temporary workspace/output
base was removed, and no fixture or generated record changed.

Design next only `WP-6-m2-root-cquery-starlark-label-boundary-design`. Preserve
the exact single-root-literal, explicit formatter/expression boundary and the
existing `RootConfiguredTargetAnalysisKey`; default/explicit `label`, arbitrary
Starlark, files, patterns, external labels, and configuration identity remain
outside the packet.

### Root cquery Starlark-label boundary design (2026-08-04)

**Status: ACCEPT; implement only
`WP-6-m2-root-cquery-starlark-label-implementation`.** The bounded public command is exactly one
root `TargetPattern::Single` plus explicit `--output=starlark` and exactly one
`--starlark:expr=str(target.label)`. The parser admits only those formatter
flags, one optional output base, and the already-normalized bzlmod policy,
environment, lockfile, and registry inputs. It rejects missing/duplicate
formatter values, passthrough, multiple positionals, patterns, external labels,
query functions/order/graph flags, Starlark files, and every alternate output
or expression before analysis as a structured exit-2 unsupported/parse error.

Core defines a private non-DICE `CqueryCommandRoot { requested,
analysis_key }` implementing `NativeCommandRoot`. Its compute calls exactly the
existing `RootConfiguredTargetAnalysisKey` in the accepted command transaction,
so the native-demand driver continues to own Needs, retries, terminal
selection, events, publication, and repository/path acceptance. It adds no
configured graph/key, package preflight, evaluator call, or second identity.
The internal `first-build` configuration remains unobservable and unchanged.

Success retains the returned `AnalysisResult` through acceptance and projects
only `result.key().label()` plus one newline. Pinned Bazel source confirms the
general formatter evaluates per configured target, but the accepted expression
reads only the configured node's canonical label and appends the line
terminator. The exact output is `@@//parent:parent\n`; progress/event UI is not
formatter stdout. Every other expression remains unsupported, including the
configuration/provider/build-options surfaces of Bazel's general formatter.

For an exact missing terminal, change `AnalysisError` from a string-only value
to an `AnalysisErrorKind::{TargetNotFound { label, build_file }, Message}`
owner. `starlark_rule_implementation` already has the loaded package, build
path, and configured label and constructs the typed variant there. Existing
display text remains stable; all other errors remain `Message`. The cquery root
maps only a matching requested-root miss to its own typed terminal. A dependency
miss or other analysis failure remains a generic analysis error. There is no
new load and no display-text parsing.

Supported root missing exits 1 with empty stdout and the three accepted Bazel
9.2 stderr lines: the `Skipping ... no such target` line, the repeated
`ERROR: no such target ...` line, and unsuccessful completion. Success has no
cquery JSON envelope. Pre-terminal parse/unsupported/transport failures retain
Slug's structured exit-2 JSON convention; cquery has no prior non-placeholder
runtime contract, so this does not change an accepted semantic terminal.

One-shot and daemon normalize the same request. The additive daemon wire is a
dedicated `CqueryRequest { target, bzlmod }` and
`DaemonRequest::Cquery`; it does not reuse loading-query expression, ordering,
graph, or strict-suite fields. The daemon response retains the existing
out-of-band `invalidated_files` metric, while CLI stdout/stderr remain the
semantic terminal bytes. Missing followed by recovery without a filesystem
edit reports zero invalidated files.

Exact production allowlist:

- `app/slug_analysis_v2/src/dice.rs`;
- `app/slug_analysis_v2/src/lib.rs`;
- `app/slug_commands_v2/src/cquery.rs`;
- `app/slug_core_v2/src/runtime/dice.rs`;
- `app/slug_core_v2/src/runtime/mod.rs`;
- `app/slug_cli_v2/src/commands/cquery.rs`;
- `app/slug_server_v2/src/lib.rs`; and
- `app/slug_server_v2/src/server.rs`.

Exact test allowlist:

- `app/slug_analysis_v2/tests/root_analysis.rs` for the typed direct-missing
  variant and preserved display;
- `app/slug_commands_v2/tests/commands.rs` for the exact positive request and
  rejection matrix;
- the existing test module in `app/slug_core_v2/src/runtime/dice.rs` for direct
  native-root Needs/terminal ownership, cold/warm activation, BUILD and `.bzl`
  invalidation, and zero action/REAPI execution;
- `app/slug_cli_v2/tests/cli.rs` for exact one-shot and daemon success/missing
  bytes and unsupported arguments; and
- `app/slug_server_v2/src/tests.rs` for additive schema round trips, malformed
  wire, retained missing/recovery, and invalidated-file counts.

Caps are 650 formatted net production lines, 600 formatted net test lines, and
1,250 total. The retained-error representation keeps existing owned values and
`Allocative`; it adds no default hash collection, interner, duplicate identity,
or new utility import. Validate serially after cleaning stale `slugd`: focused
analysis/commands/core/server/CLI tests; full affected crates; `cargo build -p
slug_cli_v2` before binary daemon tests; GNU-Windows no-run checks for affected
library crates; formatting, archive, scope/cap, forbidden graph/evaluator/error
parse/REAPI greps, and `git diff --check`.

Stop and `REPLAN` on any ninth production file, new DICE key, analysis-crate
filesystem read, display-text parsing, second package/analysis evaluation,
general Starlark execution, observable configuration token, altered fixture,
or cap breach.

Reserved review accepted the exact eight-production/five-test allowlists and
650/600/1,250 caps. It additionally requires exact
`RootConfiguredTargetAnalysisKey` activation identities and counts across cold,
warm, missing, recovery, BUILD edit, and `.bzl` edit; explicit zero action and
zero REAPI counters; dependency misses that cannot masquerade as the requested
root-missing terminal; and forbidden evaluator/key/error-parse greps. No new
retained utility, hashing, interning, compact collection, DICE ownership, or
lock-across-compute work is authorized.

### Root cquery Starlark-label implementation acceptance (2026-08-04)

**Status: ACCEPTED at `135b0567`.** The exact twelve-file implementation
lands at 649 formatted net production lines, 293 test lines, and 942 total.
It retains typed direct-target misses separately from dependency failures,
drives the existing `RootConfiguredTargetAnalysisKey` through a private
non-DICE command root, retains the returned `AnalysisResult`, and publishes
only its canonical label plus newline. The dedicated additive daemon request
contains only the target and bzlmod inputs. No configuration checksum, second
graph/evaluator, action execution, or REAPI handoff entered the path.

Focused analysis, parser, core activation, server, and one-shot/daemon CLI
tests pass. The root activation vector is exactly `13/1/1/1/1/1` for cold,
warm, missing, recovery, BUILD edit, and `.bzl` edit, with every identity the
expected `@@//pkg:{probe|missing} [target:first-build]`. Full analysis and
commands suites pass. Full core, CLI, and server suites retain only the known
clean-baseline visibility wording, broken-Bzl parser wording, and two root-only
fixture failures. GNU-Windows no-run passes for analysis, commands, and core;
the server retains its pre-existing Unix-socket compile boundary. Formatting,
diff, archive, scope, cap, and forbidden-boundary checks pass. Independent
final review returned `ACCEPT`.

### Aquery configuration-boundary replan (2026-08-04)

**Status: REPLAN before implementation.** Parallel live-owner and pinned
Bazel 9.2 formatter audits found that the existing recursive fixture already
proves three target-owned `FileWrite` actions, and Slug can traverse the same
`AnalysisResult::{actions,direct_dependencies}` graph without execution.
However, every Bazel action-query formatter exposes configuration-dependent
facts that Slug does not own. Text output includes configuration, execution
platform, action key, and configured `bazel-out` paths; summary still includes
configurations and execution platforms. Older action-shape fixtures are Bazel
9.1.1 and are not authority for a Bazel 9 implementation.

Do not add an `aquery` command root, renderer, or wire until target
configuration identity, configured artifact paths, execution platforms, and
action identity are truthful. A reduced mnemonic/count/content renderer would
be a Slug-specific format, while a hard-coded `first-build`, checksum, path, or
platform would be observably false.

Run next only `WP-6-m2-string-build-setting-transition-oracle`. It must pin the
first semantic configuration input without exposing a configuration ID: one
root string build setting, explicit command override, and a user transition
that rewrites the setting on a dependency edge, observed through exact
provider values with retained edit/error recovery. No Rust or aquery change is
authorized by the evidence packet.

### String build-setting transition diagnostic replan (2026-08-04)

**Status: REPLAN; the stopped fixture was discarded.** A six-file, eight-row
Bazel 9.2 fixture generated and replayed successfully for direct default and
command values, two distinct transitioned edges to the same child, warm reuse,
transition edit, build-setting default edit, invalid output, and recovery. The
mandated invalid-transition diagnostic names the configured edge as
`//:parent (a7a71fd) -|left|-> //:consumer`; both raw and normalized stderr
therefore expose the unavailable configuration checksum. Hiding it would
require forbidden normalization or an inexact diagnostic.

The worker removed all six untracked fixture files and restored a clean
worktree. Reserved review accepts a positive-only successor because every
successful row exposes only semantic provider values and stable labels; invalid
transition programs remain gated on general public configuration identity.

Run next only `WP-6-m2-positive-string-build-setting-transition-oracle`.
Retain the same fixture-only scope and caps, replace the invalid/recovery pair
with transition-value restoration and build-setting-default restoration, and
stop if any successful output exposes configuration identity. No Rust,
normalization, or failure-diagnostic claim is authorized.

### Positive string build-setting transition oracle acceptance (2026-08-04)

**Status: ACCEPTED at `b12774b9`.** The isolated Bazel 9.2 fixture has six
regular files, zero links, eight commands, 169 authored non-generated lines,
and 451 total generated lines. Exact Starlark cquery output proves the direct
default `default`, command input `command`, and two dependency edges from one
parent to the same `//:consumer` under distinct `left` and `right` string
values. The retained server then proves unchanged warm output, transition edit
to `changed,right`, transition restoration, build-setting default edit, and
default restoration.

Generation and no-update replay pass with `/usr/bin/bazel` 9.2.0. Fixture
list, JSON, inventory/caps, provenance, credential-pattern, archive, and diff
checks pass; focused pytest is unavailable in the environment. Independent
review required and accepted the pinned
`StarlarkConfig.java#stringSetting` source anchor. No successful formatter
output contains a configuration ID, configured path, platform, action key, or
mnemonic. Invalid transition programs remain explicitly gated on the general
configuration identity required by their Bazel diagnostic.

Design next only `WP-6-m2-positive-string-build-setting-transition-design`.
It must decide whether a bounded successful internal configuration-input and
transition vertical can reuse the existing configured-analysis graph without
claiming general cquery Starlark-file support or failure diagnostics. No Rust
is authorized until the semantic value/equality, DICE input, loading/evaluator,
dependency-transition, command-observability, utility, allowlist, and cap
boundaries are independently accepted.

### Positive string build-setting transition design (2026-08-04)

**Status: ACCEPTED; implement the bounded internal vertical.** Parallel
pinned-source/utility and live-owner audits, root synthesis, and independent
correction review selected one semantic overlay on the existing target
configuration. `RootStringSettingValue(CompactString)` owns the effective
value's equality, ordinary hash, and `Allocative` behavior. The existing
opaque `first-build` token remains only a private legacy base discriminator;
it is not parsed, displayed, exposed, or claimed as Bazel's checksum. The
packet-fixed `@@//:setting` label is not duplicated in every configuration.

The graph remains exclusively `RootConfiguredTargetAnalysisKey`. An internal
request mode resolves an optional explicit value or the loaded
`build_setting_default` through `RootPackageLoadKey`, constructs the semantic
configured key, and computes the existing resolved root mode. Resolved mode
continues to own root loading Needs and events, recursive dependency analysis,
full-key `SmallSet` deduplication, and result assembly. The legacy
`ConfiguredTargetAnalysisKey` route receives no change and must have zero new
activations. This correction avoids a parallel package-loading/event route.

Loading and evaluation are limited to the accepted fixture: one flagged
`config.string`, one string default, `ctx.build_setting_value`, one inherited
implicit setting dependency, and two declared-order user-transition label
attributes with empty inputs and exactly one string `//:setting` output.
`_setting` remains a direct dependency/provider lookup; `left` and `right` are
singleton transitioned sequences. Each transition overlays the parent value
before its recursively resolved child key is built, so two edges to the same
label remain distinct without a second graph. Frozen callables and semantic
metadata stay under the existing loaded-module lifetime and source
fingerprint. Public build/cquery flags, wire, Starlark-file/provider rendering,
outputs, diagnostics, actions, and REAPI remain unchanged.

The retained utility boundary uses `CompactString`, immutable `Arc` slices,
and the existing ordered `SmallMap`/`SmallSet`; it adds no duplicated label,
global interner/cache, default hash collection, `Hashed`, SHA, or new hasher.
Independent review accepted exact seven-production/three-test allowlists and
raised feasibility-reviewed caps to 850 production, 450 test, and 1,300 total
formatted net lines. Acceptance must freeze observed request/resolved
activation identities and counts and prove semantic A-to-B-to-A restoration,
distinct/restored transitioned keys, exact provider values, zero legacy-key
activation, zero action execution, and zero REAPI reach.

Implement next only
`WP-6-m2-positive-string-build-setting-transition-implementation`. Return
`REPLAN` for general options/checksums, broader build settings or transitions,
public cquery or exact failure diagnostics, another graph, direct filesystem
or global state, lock-across-compute, an outside file, or a cap breach.

### Positive string build-setting transition implementation replan (2026-08-04)

**Status: REPLAN; partial Rust was discarded and the worktree is clean at
`7d39c759`.** A Terra worker implemented the bounded representation, loading,
and root-key path far enough for the focused fixture-shaped analysis test to
reach provider decoding. `evaluate_loaded_rule` then passed the custom-only
return to strict `ProviderCollection::new`, which failed exactly with
`collection did not receive a DefaultInfo provider`. All three accepted
fixture rules return only custom providers. The worker removed every partial
edit with `apply_patch`; status, diff, and diff-check are clean.

Live ownership shows an existing permissive `ProviderCollection::from_values`
constructor and a two-file decoder-only route, but pinned Bazel 9.2 source
proves that permissive configured-target absence would be inexact. Bazel
accepts omitted `DefaultInfo`, creates an implicit empty default, and exposes
it through target indexing, membership, and cquery `providers(target)`. The
accepted transition fixture proves custom-only returns succeed but does not
observe that implicit default, so it cannot decide between absence and
synthesis. Changing the provider representation or fabricating a result inside
the stopped packet would exceed its allowlist and evidence.

Run next only `WP-6-m2-implicit-default-info-provider-oracle`. Add one isolated
six-file, four-command positive Bazel 9.2 fixture comparing custom-only,
explicit-empty, dependency indexing, and warm replay through named semantic
fields. Do not enumerate providers or add failures, Rust, public cquery, or
execution behavior. After acceptance, design the exact decoder normalization
owner before resuming string-setting transitions.

### Implicit DefaultInfo provider oracle acceptance (2026-08-04)

**Status: ACCEPTED at `d4e7e47e`.** The isolated Bazel 9.2 fixture has six
regular files, zero links, four successful retained-server commands, 123
authored lines, and 255 total generated-plus-authored lines. A custom-only
target and an explicit-empty target both expose named `DefaultInfo` with zero
files. A consumer successfully indexes both dependencies through the exported
custom and `DefaultInfo` constructors, retains both custom values, and observes
zero default files. Its unchanged warm replay is byte-identical.

Generation and no-update replay pass with `/usr/bin/bazel` 9.2.0. Fixture
list, JSON, inventory/caps, provenance, credential-pattern, whitespace,
archive, and diff checks pass; focused pytest is unavailable. Independent
latest-diff review returned `ACCEPT`. The formatter reads only exact named
provider keys and exposes no configuration ID, path, platform, action key, or
mnemonic. The evidence does not authorize provider enumeration,
outputs/runfiles/executable semantics, failures, or general cquery support.

### Implicit DefaultInfo decoder design (2026-08-04)

**Status: ACCEPTED; synthesize only at successful Starlark return decoding.**
Keep strict `ProviderCollection::new` and its always-default invariant. After
`evaluate_loaded_rule` successfully decodes the returned provider list, append
one existing empty `DefaultInfo` only when the list has no explicit default,
then call the unchanged strict constructor. Explicit defaults, declared files,
actions, duplicate rejection, dependencies, events, and evaluator ownership
remain unchanged. Do not use the permissive collection constructor or edit the
build API.

Independent review accepted one production and one test owner at caps of 20
production, 120 test, and 140 total formatted net lines. Required evidence is
custom-provider retrieval plus present empty default and empty outputs/actions,
the unchanged explicit-default/write-action regression, and the unchanged
strict generic collection rejection. No retained utility changes.

Implement next only `WP-6-m2-implicit-default-info-decoder-implementation`.
After acceptance, resume the already accepted positive string build-setting
transition implementation from a clean baseline.

### Implicit DefaultInfo decoder implementation (2026-08-04)

**Status: ACCEPTED at `7c6eeae5`.** The Starlark rule decoder now appends the
existing empty `DefaultInfo` only when successful return decoding found no
explicit default, then calls unchanged strict `ProviderCollection::new`.
Explicit defaults, declared files, actions, duplicate rejection, dependencies,
events, and build-API collection semantics are unchanged.

The two-file patch is 8 formatted production and 37 test lines, 45 total. A
custom-only rule regression proves exported-provider retrieval, present
structurally empty default, and empty declared outputs/actions. The existing
explicit-default/write-action regression and strict build-API rejection pass.
All five Starlark-rule tests, four configured-target tests, the full analysis
suite, formatting, diff/scope/cap checks, and GNU-Windows analysis no-run pass
with existing warnings only. Independent latest-diff review returned
`ACCEPT`; no retained utility changed.

Resume `WP-6-m2-positive-string-build-setting-transition-implementation` from
this clean baseline under its already accepted root-key graph, seven
production/three test allowlists, 850/450/1,300 caps, internal-only
observability, and all general-configuration/public-command/execution stops.

### Positive string build-setting transition implementation acceptance (2026-08-04)

**Status: ACCEPTED at `dfc1705e`.** The eight-file implementation adds one
compact semantic string-setting overlay and request/resolved modes to the
existing root configured-analysis key family. Request mode validates the exact
root setting declaration before resolving explicit/default value. Resolved
semantic roots retain named direct versus singleton-sequence dependencies,
apply and validate the one-output user transition before child-key creation,
deduplicate full configured keys, and recurse only through root keys. Ordinary
root configurations retain their legacy dependency extractor, and the legacy
configured-analysis key has zero new activations.

The final patch is 514 formatted production and exactly 450 test lines, 964
total. A retained-DICE lifecycle proves direct default, command, explicit
default-equivalent restoration, unchanged warm wrapper reuse, distinct
left/right children of the same label, transition edit/restoration, BUILD
default edit/restoration, exact custom provider values, empty actions, and a
sorted exact 33-event request/resolved activation multiset with zero legacy
keys. Ten consecutive lifecycle runs pass after correcting sibling activation
order from an invalid ordered assertion.

All 111 loading tests and 16 analysis tests pass. Full core passes 125/126;
the sole failure is the documented clean-baseline external visibility wording
mismatch. GNU-Windows no-run passes for loading, analysis, and core.
Formatting, archive, diff, scope, cap, forbidden-boundary, and independent
latest-diff reviews pass. No public build/cquery flag, wire, formatter,
diagnostic, action, or REAPI behavior changed. The retained utility audit
accepts `CompactString`, immutable `Arc` slices, and existing ordered
`SmallMap`/`SmallSet`, with no interner or new hashing.

### First-compatible toolchain evidence replan (2026-08-04)

**Status: REPLAN before production integration.** Parallel live and pinned
Bazel audits found that the existing toolchain resolver is only hand-built
pure test data: its registration key is not a DICE key, root module evaluation
does not retain registrations, loading has none of the native toolchain or
platform target kinds, `rule()` cannot declare toolchains, and analysis lacks
`ctx.toolchains`. The two named toolchain fixtures are ungenerated scaffolds
without Bazel provenance and observe build actions; the other planned fixtures
do not exist. Wiring the dormant resolver now would create an input with no
semantic consumer.

Run next only `WP-6-m2-positive-first-compatible-toolchain-oracle`. Rewrite
the dormant first-platform scaffold as a six-command Bazel 9.2 provider-only
cquery oracle proving initial/warm/reordered/restored platform selection plus
registered toolchain marker edit/restoration. It must expose no configuration,
platform label, action, output, or diagnostic identity. After acceptance,
design root registration, native declarations, DICE resolution, and prepared
toolchain context ownership before Rust.

### First-compatible toolchain oracle acceptance (2026-08-04)

**Status: ACCEPTED at `ed4baf08`.** The rewritten Bazel 9.2 fixture has six
regular files, zero links, six successful retained-server commands, 161
authored lines, and 381 generated-plus-authored lines. Provider-only cquery
observes `first`, unchanged warm `first`, registration-order `second`, restored
`first`, BUILD-marker `edited-first`, and restored `first`. All rows exit zero,
retain empty manifests and zero actions, and expose no configuration, platform,
toolchain label, path, output, action key, or mnemonic.

Generation and two no-update replays pass with Bazel 9.2.0 at pinned commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. Inventory, caps, provenance,
credential-pattern, whitespace, archive, and diff checks pass; independent
latest-diff review returned `ACCEPT`. Pinned source establishes ordered root
registration, earliest candidate selection, constraint filtering, selected
implementation loading, and `ctx.toolchains` exposure. Failure diagnostics,
patterns, external registrations, host fallback, aliases, optional or multiple
types, target constraints, exec groups, actions, and public Slug cquery remain
outside the evidence.

### Root toolchain registration-retention design (2026-08-04)

**Status: ACCEPTED as a serial semantic prerequisite.** Add one compact
`RootModuleRegistrations` value to `EvaluatedRootModule`: separate ordered
`Arc<[ApparentLabel]>` slices for execution platforms and toolchains, frozen
from evaluator-local vectors. Root `register_execution_platforms` and
`register_toolchains` retain argument and call order without sorting or
deduplication, use the existing root command policy to retain a dev registration
iff `!dev_dependency || !ignore_dev_dependency`, and set the existing
`non_module_called` ordering flag.

The retention boundary is direct-label-only and fail-closed. A minimal local
guard rejects recursive, package-wide, and wildcard target patterns before
`ApparentLabel::parse`; pattern expansion is deferred. Derived structural
equality carries registration order through root evaluation, files, graph, the
Host carrier, and the existing Need-aware `RootModuleLoadingAnchorKey`. The
anchor gains only a slice-value accessor. There is no registration digest, new
DICE key, global state, mapping rewrite, target loading, resolver, or public
surface. A→B→A order restoration, unchanged warm equality, exact multi-call
order, direct apparent labels, fail-closed invalid inputs, the default→ignore→
restore dev policy, sole Host event ownership, and the anchor's existing single
dependency are the required evidence.

Implement next only
`WP-6-m2-root-toolchain-registration-retention-implementation`. Production may
edit only `module_eval.rs`, `host_module.rs`, and `lib.rs`; tests may additionally
edit `tests/root_module_dice.rs` and inline `host_module.rs` tests. Caps are 220
formatted production net lines, 300 test lines, and 520 total. Return `REPLAN`
for pattern expansion, external materialization/mapping, command-line
registrations, native target loading/declarations, constraint selection,
`ToolchainInfo`, `ctx.toolchains`, configuration identity, public formatting,
actions, REAPI, a new DICE key/digest, or process-global state. Direct-local
MODULE dependency cycles remain the user-approved unsupported boundary for a
later packet.

### Root toolchain registration-retention implementation acceptance (2026-08-04)

**Status: ACCEPTED at `4a3af8df`.** The four-file implementation adds the
compact ordered `RootModuleRegistrations` value, records root
`register_execution_platforms` and `register_toolchains` calls under the
existing dev-dependency command policy, and exposes the result through the
existing Need-aware loading anchor. It adds no DICE key, digest, cache,
resolver, target loader, mapping rewrite, configuration identity, or public
command surface.

The final formatted net change is 83 production and 166 test lines, 249 total.
Tests prove exact multi-call order, root/apparent direct labels, retained
literal-ellipsis target names, fail-closed relative/non-string/recursive/
wildcard patterns, default→ignore→restore dev policy, order A→B→A restoration,
unchanged warm pointer reuse, sole Host event ownership, and the anchor's
unchanged single dependency. Two focused registration tests and the focused
anchor lifecycle pass. The full bzlmod crate passes 463 tests with only its
known clean-baseline nonroot source-span assertion failure. GNU-Windows no-run,
downstream core checking, formatting, archive, diff, scope, and cap gates pass.

Independent review found and corrected an overbroad `contains("...")` guard:
only no-target recursive spellings and exact package wildcard names are now
rejected, while a direct label with literal ellipses is retained. Corrected
latest-diff review returned `ACCEPT`; no retained utility or representation
outside the accepted compact slices changed.

Design next only `WP-6-m2-native-toolchain-declaration-loading-design`. Decide
the smallest serial owners for fixture-bounded native constraint setting/value,
platform, toolchain type, toolchain declaration, Starlark `rule(toolchains=)`
requirements, and `platform_common.ToolchainInfo`. Keep DICE selection,
implementation analysis, prepared `ctx.toolchains`, failure diagnostics,
patterns, externals, host fallback, public commands, actions, and REAPI out of
scope until separately accepted.

### Native toolchain declaration-loading design (2026-08-04)

**Status: ACCEPTED as two serial loading packets before one integrated
resolution/context vertical.** Native constraint, platform, toolchain-type, and
toolchain declarations are real BUILD targets, not a side list. The first
packet adds one compact `NativeToolchainTarget` enum behind
`PackageTargetKind::NativeToolchain`, preserving the ordinary target namespace,
BUILD order, duplicate-name behavior, canonical package-context labels,
structural package equality, fixed native rule capabilities, and future target
lookup. The fixture-bounded variants are `ConstraintSetting`,
`ConstraintValue { constraint_setting }`,
`Platform { constraint_values }`, `ToolchainType`, and
`Toolchain { toolchain_type, implementation, exec_compatible_with }`.

All label lists use immutable ordered slices. The native `toolchain`
implementation label is retained as NODEP semantics and never enters ordinary
dependency edges. Root registration order remains solely owned by the accepted
MODULE anchor; declarations are not joined to registrations or resolved here.
The existing `RootPackageLoadKey` remains the only Need/error/event/invalidation
owner. Root query must fail closed before projecting any package graph when a
native toolchain declaration is present; silently omitting or partially
projecting these targets is forbidden. External loading classifies the new
variant through its existing unsupported-kind boundary. No Bazel diagnostic
wording is claimed by these private/deferred stops.

The second serial packet adds ordered required toolchain-type labels to frozen
rule definitions and `StarlarkRuleImplementation`, resolving them relative to
the defining `.bzl` package and keeping them separate from ordinary
dependencies. It also binds only a freeze-capable load symbol for
`platform_common.ToolchainInfo`; invocation stays explicitly unsupported. It
must not add a ToolchainInfo provider/value, user-provider masquerade, decoder,
`ProviderCollection` change, selection, or `ctx.toolchains`. After both loading
values are accepted, one integrated DICE packet must consume the root anchor,
root package declarations, selected implementation analysis with a dedicated
builtin ToolchainInfo value, and the prepared requesting context. The dormant
digest-string `RegisteredToolchainsKey` is not an owner and remains unused.

Implement next only `WP-6-m2-native-toolchain-target-loading-implementation`.
Production may edit `app/slug_loading_v2/src/package.rs`,
`app/slug_loading_v2/src/bzl_module.rs`, and
`app/slug_query_v2/src/graph.rs`. Tests may additionally edit
`app/slug_loading_v2/tests/build_file_loading.rs`, inline
`host_package_load_tests.rs`, inline query graph tests, and
`app/slug_query_v2/tests/loading_query.rs`. Caps are 360 formatted production
net lines, 520 test lines, and 880 total. Required evidence covers exact five-
class order/capabilities/canonical labels/list order/NODEP separation,
duplicate-name behavior, wrong types/patterns/externals/unmodeled attributes,
RootPackageLoad cold/warm/edit/A→B→A/delete/recreate ownership, explicit root
query deferral, and existing external-kind deferral.

Return `REPLAN` for rule requirements, `platform_common`, ToolchainInfo,
registered-target lookup, target-kind/provider validation, constraint
normalization or resolution, command-line registration, external mapping,
aliases, host fallback, optional/multiple types, target constraints/settings,
exec groups, public query projection, cquery, diagnostics, configuration
identity, actions, REAPI, a new key/digest/cache/interner, or process-global
state. No new oracle is needed: `ed4baf08` pins the successful syntax and
pinned Bazel 9.2 rule definitions pin attribute kinds and NODEP ownership.

### Native toolchain target-loading implementation (2026-08-04)

**Status: ACCEPTED in `6a457406`.** The six-file implementation is 193
production lines, 404 test lines, and 597 total formatted net lines, within
the accepted 360/520/880 caps. `NativeToolchainTarget` retains the five exact
fixture classes as real ordered package targets with canonical labels,
immutable ordered label slices, fixed native capabilities, ordinary duplicate
name behavior, and a NODEP implementation label. It adds no declaration side
table, dependency edge, registration join, resolver, provider, or DICE key.

Root package graph construction now fails closed before any projection when a
native declaration is present. External package loading classifies the same
targets through its existing unsupported-kind error before its dependency-free
early return. Loading lifecycle coverage proves cold/warm semantic equality,
declaration edit and A-to-B-to-A restoration, deletion/recreation, sole Host
event ownership, and the unchanged first root-anchor dependency.

All 114 `slug_loading_v2` tests passed. The 53 loading-query integrations and
six query parser tests passed; the query library result was 27/28 only because
the untouched
`external_restricted_visible_uses_canonical_fake_caller_without_a_second_route`
test returns the same `PreparationRestart` failure at clean `f9f3c3d8`.
Loading, query, and core checks, both GNU-Windows no-run builds, formatting,
archive, diff, scope, and cap gates passed. Independent Terra latest-diff
review returned `ACCEPT`.

Implement next only
`WP-6-m2-toolchain-rule-provider-loading-implementation`. Retain ordered
fixture-bounded `rule(toolchains=)` requirements relative to the defining
`.bzl` package, separate from ordinary dependencies, and add only a
freeze-capable `platform_common.ToolchainInfo` load symbol whose invocation is
explicitly unsupported. Do not add a ToolchainInfo provider/value, decoder,
selected implementation analysis, resolver key, or `ctx.toolchains`.

### Toolchain rule-requirement/provider-symbol loading (2026-08-04)

**Status: ACCEPTED in `1d6106bd`.** The exact three-file implementation is 67
production lines, 269 test lines, and 336 total formatted net lines, within the
accepted 340/310/650 caps. Frozen rule definitions and instantiated
`StarlarkRuleImplementation` values retain an ordered immutable slice of zero
or one definition-package-relative canonical toolchain-type requirement.
Structural equality includes the slice, while ordinary dependencies and query
edges remain unchanged.

The existing loading globals now bind a frozen `platform_common` namespace
whose `ToolchainInfo` attribute uses the existing analysis-builtin callable
shape. The accepted oracle implementation body freezes unchanged, while a
loading-time invocation fails explicitly as unsupported. No ToolchainInfo
provider/value, user-provider surrogate, decoder, ProviderCollection change,
selection, resolver key, or `ctx.toolchains` was added.

Review corrections replaced a close substitute with the verbatim accepted
`toolchain-resolution-first-platform` definitions and BUILD declaration
shape, added cross-package definition-relative and explicit-empty coverage,
and asserted zero ordinary dependencies. Full loading first exposed an
unnecessary external-source parse for omitted requirements; returning the
empty slice before source-label parsing restored the existing external rule
path. All 117 loading tests, 53 query-loading integrations, and 16 analysis
tests then passed. Four direct checks, three GNU-Windows no-run builds,
formatting, archive, diff, scope, and cap gates passed. Independent Terra final
rereview returned `ACCEPT`.

Design next only `WP-6-m2-integrated-toolchain-resolution-context-design`.
The design must consume ordered root registrations, native declarations, and
frozen rule requirements in one real DICE-owned selection and prepared-context
vertical. It must decide canonical lookup/mapping, first-compatible ordering,
selected implementation analysis, a dedicated builtin ToolchainInfo value,
and `ctx.toolchains` access together. Do not create a dormant resolver-only key
or infer authority for Rust implementation from this acceptance.

### Integrated toolchain resolution/context design (2026-08-04)

**Status: ACCEPTED; implement only
`WP-6-m2-integrated-toolchain-resolution-context-implementation`.** Selection
stays inline in the existing `RootConfiguredTargetAnalysisKey`; no context,
resolver, digest, or cache key is added. The root analysis owner consumes the
ordered registration anchor and existing root package values, validates the
exact native declaration/reference graph, preserves platform-outer and
toolchain-inner MODULE order, then analyzes the selected NODEP implementation
through the same root key and existing configuration.

The provider boundary adds a dedicated builtin `ProviderValue::ToolchainInfo`
with exactly one compact string marker and builtin-specific collection lookup.
The existing `platform_common.ToolchainInfo` callable is phase-gated through
the analysis evaluator: loading invocation remains unsupported, while analysis
accepts exactly one named string marker. The requesting context adds only
string `ctx.attr.marker` and a one-entry `ctx.toolchains` index for the exact
root-apparent requested type. User providers remain distinct.

Selected implementations are fixture-bounded leaves: root Starlark rules with
no ordinary dependencies, toolchain requirements, transition/build-setting
role, actions, outputs, or nonempty providers beyond builtin ToolchainInfo plus
implicit empty DefaultInfo. Guards run before recursive analysis where
possible. Package Needs are unioned before semantic errors; existing anchor,
package, child-analysis, and requester event owners remain unchanged. No lock
crosses a DICE computation.

Parallel Terra live-owner, pinned Bazel, and adversarial audits found the six
accepted rows sufficient. Reserved review rejected a separate
`RootToolchainContextKey` and a private marker outside ProviderCollection,
selected the inline real-consumer path, and fixed the five-production/two-test
allowlist at 540 production, 700 test, and 1,240 total formatted net lines.
Optional/multiple types, externals, aliases, host/target fallback, exec groups,
general attributes/providers, public diagnostics/query expansion, actions,
execution, REAPI, and configuration expansion remain stops.

#### Integrated implementation cap correction (2026-08-04)

The first uncommitted sizing prototype proved that the accepted vertical is
atomic but the original production cap was not realistic. Reserved Terra
review accepted correction in place at **740 production, 760 test, and 1,500
total formatted net lines**, retaining the exact five-production/two-test
allowlist. Splitting would create dormant resolver or provider/context
substrate; reconstruction would reproduce the same owner boundary.

Before final review, every discovery round must inspect all sibling root
package outcomes, union Needs, and return Need before recorded semantic or DICE
errors. An external registration records an error without loading/projecting
the external label, while discoverable root registrations and the required
type still participate in that round's Need union. Toolchain execution
constraints reject duplicate settings. Selected leaves require exactly one
user string `marker` schema/value plus only the loader-invariant defaulted
empty `tags` entry, no executable/test capability, and the existing dependency/
requirement/transition/build-setting/action/output/diagnostic guards.
Post-analysis provider validation uses exact builtin keys and exact two-entry
cardinality for empty DefaultInfo plus ToolchainInfo, never names.
Zero-requirement requesters bypass anchor/resolution and retain the existing
activation/result path; selected children still use the same existing root key.

The positive requester must match the accepted oracle and observe only
`ctx.toolchains["//:demo_type"].marker`. Evidence also covers full lifecycle
and A-to-B-to-A result equality, root/anchor/package/child ownership, zero
legacy activation, all native/reference/selection/leaf failures, external Need
precedence, builtin/user identity, callable/index errors, explicit
zero-requirement no-anchor behavior, and zero actions, outputs, diagnostics,
and oracle manifests.

### Integrated toolchain resolution/context implementation (2026-08-04)

**Status: ACCEPTED in `1533569f`.** The exact seven-file implementation is
724 production lines, 630 test lines, and 1,354 total formatted net lines,
within the corrected 740/760/1,500 caps. Root analysis now consumes ordered
root registrations, validates the complete root native declaration/reference
graph, selects the first compatible platform/toolchain pair in MODULE order,
and analyzes the selected NODEP implementation through the ordinary existing
root configured-target key.

The implementation adds a dedicated builtin ToolchainInfo value and exact
builtin-key collection lookups, phase-gates its callable to analysis, and
prepares only the accepted string marker plus one-entry `ctx.toolchains`
surface. Selected implementations are exact non-executable, non-test marker
leaves with loader-defaulted empty tags, zero dependencies, requirements,
actions, outputs, and diagnostics, and exactly builtin empty DefaultInfo plus
builtin ToolchainInfo. Zero-requirement requesters bypass resolution. Every
root package discovery round unions Needs before recorded semantic or DICE
errors; external registrations and native references are never projected as
root labels.

The accepted matrix covers first-compatible selection and restoration,
marker/BUILD lifecycle and full result equality, exact root/anchor/package/
selected-child activation and event ownership, native/reference/constraint/
selection failures, external and later-round Need precedence, leaf/capability/
provider/callable/context failures, builtin/user collisions, and the
zero-requirement no-anchor edge. Root serial validation passed 24 analysis,
22 build-API, and 117 loading tests, `slug_core_v2` check, formatting, diff,
scope, cap, archive, and three GNU-Windows no-run gates with existing warnings
only. Independent Terra latest-diff review returned `ACCEPT` after corrections
for external-reference projection, defaulted-tag provenance, exact builtin
DefaultInfo identity, and the omitted-marker guard.

Design next only `WP-6-m2-root-action-closure-boundary-design`. Live
`BuildCommandEvaluation` retains requested roots only, so its declared-action
count and REAPI iteration omit actions owned by recursively analyzed
dependencies. Reuse the accepted recursive target-owned-write evidence and
design a deterministic, duplicate-safe closure over existing configured
dependency identities. Do not retry public configuration identity or aquery:
both remain `REPLAN` until the general configuration substrate exists.

### Root command action-closure boundary design (2026-08-04)

**Status: ACCEPTED; implement only
`WP-6-m2-root-action-closure-implementation`.** The existing root analysis key
already owns recursive configured-target evaluation and each `AnalysisResult`
owns its local actions, but `BuildCommandEvaluation` retains only requested
roots. Its action count and existing REAPI iterator therefore omit recursively
owned dependency actions even though the accepted Bazel 9.2 recursive record
proves distinct parent and two-leaf FileWrite ownership.

The accepted design changes successful root-analysis payloads to shared
`Arc<AnalysisResult>` handles inside the existing outer DICE result. The build
command retains requested targets plus an immutable command-local action
closure of those handles. It traverses existing configured dependency keys in
breadth-first frontiers: requested roots first, then declaration-ordered
layers, with first-seen deduplication by the complete opaque
`ConfiguredTargetKey`. This matches the proven parent/second/first case without
claiming generic Bazel traversal order. Same-label configuration-distinct
nodes remain distinct without formatting a configuration.

Every frontier reuses the existing root analysis key and joins all unique
children, so Needs union before the first BFS-order terminal error. Those
direct command-to-child DICE edges make child-only action edits invalidate the
command even when a parent provider result compares equal. Reuse may record
the same child node as reused, but no evaluator or event batch is duplicated.
`BuildCommandRootKey` remains the sole command owner; no key, graph, cache,
lock, global, interner, scheduler, or second action owner is added.

Parallel Terra live-owner, accepted-evidence, and Buck2 utility audits plus
reserved review selected two production owners, four test owners, and caps of
360 production, 650 test, 180 documentation, and 1,190 total net lines. The
implementation must cover multi-root/diamond/configuration-distinct ordering,
Need precedence, target-local events, child edit/delete/recreate/orphan
pruning and A-to-B-to-A equality, public action count, and the existing
consumer iterator. Configuration identity, aquery, scheduling/execution,
cycles, external mapping, and toolchain action breadth remain explicit stops.

Pre-edit compile review added `app/slug_analysis_v2/tests/starlark_rule.rs` as
a mechanical fourth test owner under the unchanged caps. Its existing root
request helpers must return shared `Arc<AnalysisResult>` handles after the
root-key payload change; cloning an owned result there would violate the
accepted no-deep-clone boundary. No fixture or assertion semantics may change.

### Root command action-closure implementation accepted (2026-08-04)

Commit `afd2a606` accepts `WP-6-m2-root-action-closure-implementation` at 162
formatted production, 458 test, and 620 total net lines. Successful root
analysis values now share `Arc<AnalysisResult>` handles, and the existing build
command retains one immutable breadth-first action closure over full
`ConfiguredTargetKey` identities. Requested-root analysis counts remain
unchanged while declared-action counting and the existing CLI/REAPI iterator
include recursively owned actions.

The implementation preserves root and declaration order, duplicate-root and
diamond deduplication, configuration-distinct same-label nodes, Need-before-
error frontier reduction, direct child invalidation edges, target-local event
ownership, child edit/delete/recreate and orphan pruning, and full A-to-B-to-A
command equality. A public CLI regression reports three actions and separately
borrows the REAPI-style iterator to observe the exact parent/second/first
outputs without executing them. Cyclic configured-target behavior remains
deferred by explicit user approval.

Root validation passed all 24 `slug_analysis_v2` tests, four focused action-
closure tests, the focused recursive CLI test, formatting, and diff checks.
The full core result was 129/130 library tests plus 13/13 integrations; the sole
external-visibility diagnostic wording failure is the established clean-
baseline mismatch. Independent Terra correction review returned `ACCEPT` and
confirmed the exact five-file allowlist and caps.

Design next only `WP-6-m2-action-query-identity-boundary-design`. The accepted
closure solves recursive reachability, not Bazel identity. Reconcile the
existing Bazel 9.2 evidence and live owners for authoritative `BuildOptions`
configuration identity, configured artifact paths, selected execution-
platform identity, and Bazel ActionKey. Treat those four facts and the aquery
consumer as one handoff for adjudication: either freeze one future atomic
vertical or return `REPLAN` with exact serial prerequisites.

This successor is documentation-only at a 340-line cap. It authorizes no Rust,
tests, fixtures, oracle runs, dependencies, DICE owners, command wire or
formatter, paths, platform/action-key representation, REAPI/execution work, or
partial/hard-coded checksum. `first-build` and REAPI digests are explicitly not
Bazel configuration or ActionKey identities.

### Action-query identity boundary design result (2026-08-04)

**Status: REPLAN; no atomic implementation vertical is authorized.** The raw
Bazel 9.2 recursive action record visibly includes `k8-fastbuild`
configuration, `bazel-out/k8-fastbuild/bin/...` outputs, the host execution
platform, and a distinct 64-hex ActionKey for each parent/second/first
FileWrite. Its accepted normalization deliberately ignores configuration and
action-key noise, so it proves recursive ownership and visibility only—not
identity equality, invalidation, or algorithms.

The live gaps reside in serial owners. Build and cquery requests carry bzlmod
inputs and still construct `target:first-build`; no shared transactional
target-options producer exists. Action declarations retain package-relative
strings, not configured artifacts or output roots. Integrated toolchain
selection drops the selected platform before `PreparedToolchain` and action
registration; `ActionSpec` retains exec properties/group but no selected
platform. No Bazel ActionKey type or computation exists. The later REAPI digest
hashes protocol command/action bytes, input-root digest, timeout, and remote
properties and is not a Bazel ActionKey. Aquery remains a query-like parser and
CLI placeholder with no core root, daemon request, action container, or
formatter.

The required order is: a complete DICE-owned target configuration and command
input owner; typed configured artifact/output-root identity; retained
per-action execution-platform and exec-group identity; action-kind-specific
Bazel ActionKey ownership; then an aquery root/container/formatter consuming
the accepted action closure without re-analysis. A partial checksum, string
path shim, platform inferred from remote properties, or REAPI digest alias
would make every downstream layer observably false.

Run next only `WP-6-m2-action-query-identity-evidence`. Add an isolated Bazel
9.2 source/oracle fixture—never modify the protected recursive fixture—to pin
configuration/output-root `C0 -> C1 -> C0`, a default execution-platform
`P0 -> P1 -> P0` selection lifecycle, and FileWrite content/output-path
ActionKey changes and restorations. Capture paired raw text and
`--output=jsonproto` for exactly nine states: baseline, each changed state,
and each restoration, for 18 total commands. Anchor
`BuildOptions`, `BuildConfigurationValue`, `OutputDirectories`,
`StarlarkActionFactory`, `ActionKeyComputer#getKey`, and
`FileWriteAction#computeKey`. Evidence may name stable inputs and serial owners
only; it must not prescribe a Slug checksum, path, platform, or ActionKey
algorithm.

The evidence packet allows zero Rust and at most 340 formatted authored
fixture/documentation lines. Stop on non-9.2 source, an unavailable isolated
platform discriminator, protected-fixture edits, unstable identity treated as
a constant, any Slug command/wire/DICE/formatter change, REAPI reuse,
execution/cache/materialization work, broader action kinds or exec groups, or
cap breach.

### Action-query identity evidence accepted (2026-08-04)

Commit `f00e99db` accepts `WP-6-m2-action-query-identity-evidence` as one
isolated five-file Bazel 9.2 fixture at 227 authored lines. Its 18 commands are
nine immediately paired text/jsonproto states in one retained server:
configuration `C0 -> C1 -> C0`, default execution platform `P0 -> P1 -> P0`,
FileWrite content A-to-B-to-A, and declared output path A-to-B-to-A. The
protected recursive action-ownership fixture is unchanged.

The evidence separates the four identity domains. Changing compilation mode
changes the full BuildOptions checksum and `bazel-out` configuration root but
leaves the FileWrite ActionKey unchanged. Changing the selected default-
exec-group platform changes that platform and the ActionKey while preserving
configuration and output. Changing content changes only the ActionKey.
Changing the declared output name changes the configured artifact path but
leaves the ActionKey unchanged. Every restoration returns exact baseline
text and structured fields, and each text/jsonproto pair agrees.

Pinned Bazel 9.2 source confirms the ownership: `BuildOptions` supplies the
configuration checksum; `BuildConfigurationValue` and `OutputDirectories`
derive configured roots; Starlark FileWrite construction takes the default
exec-group `RuleContext` action owner; `ActionKeyComputer` fingerprints the
selected execution platform after action-specific material; and
`FileWriteAction#computeKey` fingerprints its GUID, executable bit, and
contents. The protocol REAPI digest remains unrelated.

Generation and two fresh no-update Bazel 9.2 replays passed. Independent review
accepted the exact matrix, source anchors, restoration facts, zero credential
matches, protected scope, and 227/340 authored cap. Generated `oracle.json` is
the retained record and is excluded from the authored cap; no Slug Rust, test,
wire, DICE, dependency, or formatter changed.

Design next only `WP-6-m2-general-target-configuration-input-chain-design`.
Enumerate every native `FragmentOptions` default/cache-key input plus canonical
Starlark option/scope and host/platform/mapping input, map each to a shared
one-shot/daemon/transaction owner, and freeze exact Need/error/invalidation/
equality behavior before either build or cquery constructs a root key. Return
`REPLAN` if complete ownership cannot fit one bounded implementation packet.

This successor is documentation/source-only at a 380-line cap. It authorizes
no Rust, tests, fixtures, oracle run, dependency, command wire, DICE owner, or
checksum. Configured artifacts, per-action platform, Bazel ActionKey, public
cquery/aquery formatting, and execution remain downstream stops.

### General target-configuration input-chain design result (2026-08-04)

**Status: REPLAN; no Rust implementation is authorized.** The complete pinned
Bazel 9.2 registry contains seventeen native `FragmentOptions` classes and 341
cache-key options, not the fourteen classes assumed by the predecessor. In
fully qualified class-name checksum order, the classes and option counts are:

1. `PlatformOptions` (7)
2. `ShellConfiguration.Options` (1)
3. `CoreOptions` (71)
4. `CoverageConfiguration.CoverageOptions` (2)
5. `TestConfiguration.TestOptions` (21)
6. `BazelRuleClassProvider.StrictActionEnvOptions` (1)
7. `BazelPythonConfiguration.Options` (3)
8. `AndroidConfiguration.Options` (60)
9. `BazelAndroidConfiguration.Options` (1)
10. `AppleCommandLineOptions` (26)
11. `ConfigFeatureFlagOptions` (2)
12. `CppOptions` (78)
13. `JavaOptions` (36)
14. `J2ObjcCommandLineOptions` (2)
15. `ObjcCommandLineOptions` (13)
16. `ProtoConfiguration.Options` (11)
17. `PythonOptions` (6)

The Bazel rule-class provider supplies rule-set-required fragments plus
`CoreOptions` to `FragmentRegistry.create`. `BuildOptions` sorts native classes
by fully qualified name and fingerprints each normalized fragment cache key,
then canonical-label-sorted Starlark values and scopes. `OptionsBase` orders
option fields alphabetically and encodes empty lists, nulls, and escaped string
forms distinctly. Defaults, converters, repeat/expansion behavior, and
fragment-specific normalization are therefore semantic inputs rather than
parser details.

The final configuration also depends on inputs outside that registry. Bazel's
configuration-key producer applies target-platform flags or platform mappings,
resolves Starlark option scopes including project-baseline reset/removal, and
only then creates the key. Host CPU/default observations, repository-mapped
labels, mapping-file contents, platform targets, BUILD and `.bzl` definitions,
typed build-setting defaults and explicit values, and `PROJECT.scl` can all
participate before the checksum.

Live Slug has none of that complete chain. Build preserves raw configuration
flags but runtime ignores them; cquery rejects them; daemon requests carry no
target options. The per-attempt input bundle injects only bzlmod/registry
inputs. Build and cquery construct `target:first-build` before entering the
command transaction. `ConfigurationKey` accepts an opaque checksum, its stable
serialization omits the existing string-setting side channel, and transitions
overlay that side channel without recomputing identity. A truthful producer
must instead run inside the committed command transaction, return unioned
Needs before semantic errors, and allow roots to be constructed only from a
complete canonical configuration.

The required serial implementation layers are: complete typed native
vocabulary and normalization; shared one-shot/daemon request identity; Host,
platform mapping, and platform-flag DICE inputs; Starlark values/defaults/
scopes; then transactional production, checksum/key/transition replacement,
and build/cquery integration. A single implementation packet cannot own those
surfaces with a complete 341-option contract.

Run next only `WP-6-m2-bazel-9-target-configuration-input-ledger`. It must
inventory all seventeen classes and 341 options with defaults, converters,
normalizers, ordering, and encoding; enumerate every non-native input; and map
each row to exactly one eventual command, wire, Host/loading, DICE, or analysis
owner. Freeze pre-DICE parse errors, Need-before-error, complete-result
validity, structural equality, invalidation, and one-shot/same-daemon
`C0 -> C1 -> C0` restoration before naming the first Rust packet.

The ledger is documentation/source-only at 680 formatted lines. It authorizes
zero Rust, tests, fixtures, oracle, generated data, wire, or DICE changes.
Configured artifacts, per-action execution platforms, Bazel ActionKey,
cquery/aquery formatting, and execution remain downstream. Configured-target
dependency-cycle semantics are deferred with user approval; the retained
closure and the configuration ledger cover acyclic recursion only.

### Bazel 9.2 native target-option ledger (pinned `8220c6198837d5c13d53fea211cf3282aa12408a`)

**Status: complete native inventory — 17 registered `FragmentOptions` classes, 341
unique canonical cache-key options.** This is an input contract, not Slug
implementation authorization. Each source value below was read only through
`git -C ../bazel show 8220c6198837d5c13d53fea211cf3282aa12408a:<path>`; the
sibling checkout's newer `HEAD` was not consulted.

**Row notation.** Rows are canonical-option-name order within the FQCN-order class
heading: `t` field type, `d` exact annotation default literal/source expression,
`c` converter (`-` = annotation's built-in `Converter.class`), `m`
`allowMultiple`, `old` oldName, `x` expansion, `i` implicitRequirements, `N`
normalizer, and the exact pinned annotation line. `-` means the annotation's
empty/default member. `d="null"` has the `Option.java` special default meaning:
null for non-repeatable fields and `[]` for `m=T`; repeatable flags otherwise
default to `[]` (their annotation default is ignored). A default is parsed by its
converter, so the literal/source expression—not a help rendering—is authoritative.

**Registration, ordering, and encoding.** `FragmentRegistry.create` collects fragment
requirements plus additional options into `ImmutableSortedSet` using
`BuildOptions.LEXICAL_FRAGMENT_OPTIONS_COMPARATOR` (`Class::getName`): this heading
order is the required FQCN checksum order. `BuildOptions.Builder.addFragmentOptions`
first calls `getNormalized`, then sorts fragment-map keys by that comparator;
`checksum()` fingerprints each native `cacheKey()` in that order, then
canonical-label-sorted Starlark values and scopes. `OptionsBase.asMap()` uses
`IsolatedOptionsData.getAllOptionDefinitionsForClass`, whose fields are sorted by
canonical option name: the row order is therefore the per-fragment cache-key order.
`mapToCacheKey` emits `name=EMPTY, ` for an empty `List`, `name=NULL, ` for null,
and otherwise quoted `ESCAPER.escape(value.toString())`; the escaper maps `\` to
`\\` and `"` to `\"`. Class cache keys are
`FQCN{<map>}`. Empty native-and-Starlark options checksum to 64 zeroes.

**Normalizer legend.** `I` is inherited identity. `P` clones then makes
`extraToolchains` non-null and deduplicates keeping last, and keeps only the first
`platforms` value. `C` clones then dedup/sorts `allowedCpuValues`; keeps-last then
key-sorts `commandLineBuildVariables` and `commandLineFlagAliases`; canonicalizes
`defaultFeatures` (sorted enables followed by sorted disables; disable wins); and
keeps-last environment entries in `actionEnvironment` and `hostActionEnvironment`.
`T` clones then keeps the last `testEnvironment` entry per variable name. Thus `N`
is the class-level effect applied before every native cache key, including rows not
mutated by that particular normalizer.

**Pinned anchors.** Registry: `src/main/java/com/google/devtools/build/lib/analysis/config/FragmentRegistry.java:L28-L54`; class comparator/checksum/normalizing builder: `src/main/java/com/google/devtools/build/lib/analysis/config/BuildOptions.java:L73-L75,L180-L192,L402-L407,L479-L497`; option-name order: `src/main/java/com/google/devtools/common/options/IsolatedOptionsData.java:L63-L82`; cache encoding: `src/main/java/com/google/devtools/common/options/OptionsBase.java:L75-L109`; annotation default/repeat/expansion/old-name semantics: `src/main/java/com/google/devtools/common/options/Option.java:L51-L58,L138-L184`; normalization base/helpers: `src/main/java/com/google/devtools/build/lib/analysis/config/FragmentOptions.java:L61-L129`; P/C/T overrides: the per-class files at `L189-L201`, `L1186-L1257`, and `L407-L411`, respectively.

#### 01. `com.google.devtools.build.lib.analysis.PlatformOptions` — 7 options; N=P; pinned `src/main/java/com/google/devtools/build/lib/analysis/PlatformOptions.java`
01|`extra_execution_platforms`|t=`List<String>`|d=`""`|c=`CommaSeparatedOptionListConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=P|@L66
02|`extra_toolchains`|t=`List<String>`|d=`"null"`|c=`CommaSeparatedOptionListConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=P|@L98
03|`host_platform`|t=`Label`|d=`DEFAULT_HOST_PLATFORM`|c=`HostPlatformConverter.class`|m=F|old=`"experimental_host_platform"`|x=`-`|i=`-`|N=P|@L52
04|`incompatible_use_toolchain_resolution_for_java_rules`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=P|@L133
05|`platform_mappings`|t=`PlatformMappingKey`|d=`""`|c=`PlatformMappingKeyConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=P|@L142
06|`platforms`|t=`List<Label>`|d=`""`|c=`LabelListConverter.class`|m=F|old=`"experimental_platforms"`|x=`-`|i=`-`|N=P|@L82
07|`toolchain_resolution_debug`|t=`RegexFilter`|d=`"-.*"`|c=`RegexFilter.RegexFilterConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=P|@L118
#### 02. `com.google.devtools.build.lib.analysis.ShellConfiguration.Options` — 1 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/analysis/ShellConfiguration.java`
01|`shell_executable`|t=`PathFragment`|d=`"null"`|c=`PathFragmentConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L72
#### 03. `com.google.devtools.build.lib.analysis.config.CoreOptions` — 71 options; N=C; pinned `src/main/java/com/google/devtools/build/lib/analysis/config/CoreOptions.java`
01|`action_env`|t=`List<Converters.EnvVar>`|d=`"null"`|c=`Converters.EnvVarsConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=C|@L525
02|`affected by starlark transition`|t=`List<String>`|d=`""`|c=`EmptyListConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L456
03|`allow_analysis_failures`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L790
04|`allow_unresolved_symlinks`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`"experimental_allow_unresolved_symlinks"`|x=`-`|i=`-`|N=C|@L877
05|`allowed_cpu_values`|t=`ImmutableList<String>`|d=`""`|c=`CommaSeparatedOptionSetConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L270
06|`analysis_testing_deps_limit`|t=`int`|d=`"2000"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L818
07|`archived_tree_artifact_mnemonics_filter`|t=`RegexFilter`|d=`"-.*"`|c=`RegexFilter.RegexFilterConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L1075
08|`build_runfile_links`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L601
09|`build_runfile_manifests`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L591
10|`check_licenses`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L705
11|`check_visibility`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L661
12|`collect_code_coverage`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L566
13|`compilation_mode`|t=`CompilationMode`|d=`"fastbuild"`|c=`CompilationMode.Converter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L396
14|`cpu`|t=`String`|d=`""`|c=`AutoCpuConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L279
15|`define`|t=`List<Map.Entry<String, String>>`|d=`"null"`|c=`Converters.AssignmentConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=C|@L255
16|`enable_runfiles`|t=`TriState`|d=`"auto"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L954
17|`enforce_constraints`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`"experimental_enforce_constraints"`|x=`-`|i=`-`|N=C|@L713
18|`evaluating for analysis test`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L804
19|`exec_aspects`|t=`List<String>`|d=`"null"`|c=`Converters.CommaSeparatedOptionListConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=C|@L1125
20|`experimental_action_listener`|t=`List<Label>`|d=`"null"`|c=`LabelListConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=C|@L724
21|`experimental_allow_map_directory`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L896
22|`experimental_collect_code_coverage_for_generated_files`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L580
23|`experimental_debug_selects_always_succeed`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L1087
24|`experimental_enforce_transitive_visibility`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L670
25|`experimental_exclude_defines_from_exec_config`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L137
26|`experimental_exec_config`|t=`String`|d=`"@_builtins//:common/builtin_exec_platforms.bzl%bazel_exec_transition"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L97
27|`experimental_exec_configuration_distinguisher`|t=`ExecConfigurationDistinguisherScheme`|d=`"off"`|c=`ExecConfigurationDistinguisherSchemeConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L491
28|`experimental_extended_sanity_checks`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L306
29|`experimental_output_directory_naming_scheme`|t=`OutputDirectoryNamingScheme`|d=`"diff_against_dynamic_baseline"`|c=`OutputDirectoryNamingSchemeConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L757
30|`experimental_output_paths`|t=`OutputPathsMode`|d=`"off"`|c=`OutputPathsConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L932
31|`experimental_override_platform_cpu_name`|t=`List<Map.Entry<Label, String>>`|d=`"null"`|c=`LabelToStringEntryConverter.class`|m=T|old=`"experimental_override_name_platform_in_output_dir"`|x=`-`|i=`-`|N=C|@L196
32|`experimental_platform_in_output_dir`|t=`TriState`|d=`"Auto"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L162
33|`experimental_propagate_custom_flag`|t=`List<String>`|d=`"null"`|c=`CoreOptionConverters.CustomFlagConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=C|@L121
34|`experimental_remotable_source_manifests`|t=`boolean`|d=`"false"`|c=`BooleanConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L1050
35|`experimental_strict_fileset_output`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L338
36|`experimental_throttle_action_cache_check`|t=`boolean`|d=`"true"`|c=`BooleanConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L1100
37|`experimental_use_platforms_in_output_dir_legacy_heuristic`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L182
38|`experimental_writable_outputs`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L329
39|`features`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=C|@L831
40|`flag_alias`|t=`List<Map.Entry<String, Label>>`|d=`"null"`|c=`CoreOptionConverters.FlagAliasConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=C|@L1060
41|`host_action_env`|t=`List<Converters.EnvVar>`|d=`"null"`|c=`Converters.EnvVarsConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=C|@L547
42|`host_compilation_mode`|t=`CompilationMode`|d=`"opt"`|c=`CompilationMode.Converter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L406
43|`host_cpu`|t=`String`|d=`""`|c=`AutoCpuConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L387
44|`host_features`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=C|@L845
45|`include_config_fragments_provider`|t=`IncludeConfigFragmentsEnum`|d=`"off"`|c=`IncludeConfigFragmentsEnumConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L1026
46|`incompatible_always_include_files_in_data`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L611
47|`incompatible_auto_exec_groups`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L419
48|`incompatible_bazel_test_exec_run_under`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L1008
49|`incompatible_bep_cpu_from_platform`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L243
50|`incompatible_check_testonly_for_output_files`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L690
51|`incompatible_compact_repo_mapping_manifest`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L626
52|`incompatible_disable_select_on`|t=`ImmutableList<String>`|d=`""`|c=`CommaSeparatedOptionSetConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L111
53|`incompatible_exclude_starlark_flags_from_exec_config`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`"experimental_exclude_starlark_flags_from_exec_config"`|x=`-`|i=`-`|N=C|@L150
54|`incompatible_filegroup_runfiles_for_data`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L66
55|`incompatible_limit_platforms_in_output_dir_to`|t=`List<Label>`|d=`""`|c=`LabelListConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L216
56|`incompatible_merge_genfiles_directory`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L88
57|`incompatible_modify_execution_info_additive`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L991
58|`incompatible_target_cpu_from_platform`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L230
59|`instrument_test_targets`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L374
60|`instrumentation_filter`|t=`RegexFilter`|d=`"-/javatests[/:],-/test/java[/:]"`|c=`RegexFilter.RegexFilterConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L359
61|`is exec configuration`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L781
62|`min_param_file_size`|t=`int`|d=`"32768"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L294
63|`modify_execution_info`|t=`List<ExecutionInfoModifier>`|d=`"null"`|c=`ExecutionInfoModifier.Converter.class`|m=T|old=`-`|x=`-`|i=`-`|N=C|@L963
64|`platform_suffix`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L510
65|`run_under`|t=`RunUnder`|d=`"null"`|c=`RunUnderConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L640
66|`scl_config`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L77
67|`stamp`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L349
68|`strict_filesets`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L317
69|`target_environment`|t=`List<Label>`|d=`"null"`|c=`LabelListConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=C|@L858
70|`use_target_platform_for_tests`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L1113
71|`verbose_visibility_errors`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=C|@L681
#### 04. `com.google.devtools.build.lib.analysis.test.CoverageConfiguration.CoverageOptions` — 2 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/analysis/test/CoverageConfiguration.java`
01|`coverage_output_generator`|t=`Label`|d=`"@bazel_tools//tools/test:lcov_merger"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L40
02|`coverage_report_generator`|t=`Label`|d=`"@bazel_tools//tools/test:coverage_report_generator"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L57
#### 05. `com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions` — 21 options; N=T; pinned `src/main/java/com/google/devtools/build/lib/analysis/test/TestConfiguration.java`
01|`allow_local_tests`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L398
02|`cache_test_results`|t=`TriState`|d=`"auto"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L168
03|`coverage_support`|t=`Label`|d=`"@bazel_tools//tools/test:coverage_support"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L325
04|`default_test_resources`|t=`List<Pair<String, Map<TestSize, Double>>>`|d=`"null"`|c=`TestResourcesConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=T|@L125
05|`experimental_cancel_concurrent_tests`|t=`CancelConcurrentTests`|d=`"never"`|c=`CancelConcurrentTests.Converter.class`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L310
06|`experimental_fetch_all_coverage_outputs`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L342
07|`experimental_retain_test_configuration_across_testonly`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L210
08|`experimental_split_coverage_postprocessing`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L366
09|`incompatible_check_sharding_support`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L383
10|`incompatible_exclusive_test_sandboxed`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L353
11|`runs_per_test`|t=`List<PerLabelOptions>`|d=`"1"`|c=`RunsPerTestConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=T|@L260
12|`runs_per_test_detects_flakes`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L286
13|`test_arg`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=T|@L228
14|`test_env`|t=`List<Converters.EnvVar>`|d=`"null"`|c=`Converters.EnvVarsConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=T|@L90
15|`test_filter`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L147
16|`test_result_expiration`|t=`int`|d=`"-1"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L187
17|`test_runner_fail_fast`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L158
18|`test_sharding_strategy`|t=`TestShardingStrategy`|d=`"explicit"`|c=`ShardingStrategyConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L243
19|`test_timeout`|t=`Map<TestTimeout, Duration>`|d=`"-1"`|c=`TestTimeout.TestTimeoutConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L109
20|`trim_test_configuration`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L195
21|`zip_undeclared_test_outputs`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=T|@L375
#### 06. `com.google.devtools.build.lib.bazel.rules.BazelRuleClassProvider.StrictActionEnvOptions` — 1 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/bazel/rules/BazelRuleClassProvider.java`
01|`incompatible_strict_action_env`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`"experimental_strict_action_env"`|x=`-`|i=`-`|N=I|@L73
#### 07. `com.google.devtools.build.lib.bazel.rules.python.BazelPythonConfiguration.Options` — 3 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/bazel/rules/python/BazelPythonConfiguration.java`
01|`experimental_python_import_all_repositories`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L57
02|`incompatible_remove_ctx_bazel_py_fragment`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L74
03|`python_path`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L45
#### 08. `com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options` — 60 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/rules/android/AndroidConfiguration.java`
01|`Android configuration distinguisher`|t=`ConfigurationDistinguisher`|d=`"MAIN"`|c=`ConfigurationDistinguisherConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L200
02|`android_compiler`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L209
03|`android_databinding_use_androidx`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L622
04|`android_databinding_use_v3_4_args`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L609
05|`android_dynamic_mode`|t=`DynamicMode`|d=`"off"`|c=`DynamicModeConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L221
06|`android_fixed_resource_neverlinking`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L661
07|`android_manifest_merger`|t=`AndroidManifestMerger`|d=`"android"`|c=`AndroidManifestMergerConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L538
08|`android_manifest_merger_order`|t=`ManifestMergerOrder`|d=`"alphabetical"`|c=`ManifestMergerOrderConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L553
09|`android_migration_tag_check`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L673
10|`android_platforms`|t=`List<Label>`|d=`""`|c=`LabelOrderedSetConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L238
11|`android_resource_shrinking`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L489
12|`apk_signing_method`|t=`ApkSigningMethod`|d=`"v1_v2"`|c=`ApkSigningMethodConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L571
13|`break_build_on_parallel_dex2oat_failure`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L382
14|`desugar_for_android`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`"experimental_desugar_for_android"`|x=`-`|i=`-`|N=I|@L267
15|`desugar_java8_libs`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`"experimental_desugar_java8_libs"`|x=`-`|i=`-`|N=I|@L280
16|`dexopts_supported_in_dexmerger`|t=`List<String>`|d=`"--minimal-main-dex,--set-max-idx-number"`|c=`Converters.CommaSeparatedOptionListConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L429
17|`dexopts_supported_in_dexsharder`|t=`List<String>`|d=`"--minimal-main-dex"`|c=`Converters.CommaSeparatedOptionListConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L443
18|`dexopts_supported_in_incremental_dexing`|t=`List<String>`|d=`"--no-optimize,--no-locals"`|c=`Converters.CommaSeparatedOptionListConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L415
19|`experimental_allow_android_library_deps_without_srcs`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L469
20|`experimental_always_filter_duplicate_classes_from_android_test`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L916
21|`experimental_android_assume_minsdkversion`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L356
22|`experimental_android_compress_java_resources`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L584
23|`experimental_android_databinding_v2`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L596
24|`experimental_android_library_exports_manifest_default`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L637
25|`experimental_android_resource_cycle_shrinking`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L500
26|`experimental_android_resource_name_obfuscation`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L526
27|`experimental_android_resource_path_shortening`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L514
28|`experimental_android_resource_shrinking`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L477
29|`experimental_android_rewrite_dexes_with_rex`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L455
30|`experimental_android_use_parallel_dex2oat`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L370
31|`experimental_check_desugar_deps`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L296
32|`experimental_disable_instrumentation_manifest_merge`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L979
33|`experimental_filter_library_jar_with_program_jar`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L930
34|`experimental_filter_r_jars_from_android_test`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L698
35|`experimental_get_android_java_resources_from_optimized_jar`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L990
36|`experimental_incremental_dexing_after_proguard`|t=`int`|d=`"50"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L320
37|`experimental_incremental_dexing_after_proguard_by_default`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L343
38|`experimental_omit_resources_info_provider_from_android_binary`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L650
39|`experimental_one_version_enforcement_use_transitive_jars_for_binary_under_test`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L710
40|`experimental_persistent_aar_extractor`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L724
41|`experimental_remove_r_classes_from_instrumentation_test_jar`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L903
42|`experimental_use_dex_splitter_for_incremental_dexing`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L334
43|`experimental_use_rtxt_from_merged_resources`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L940
44|`fat_apk_hwasan`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L254
45|`incompatible_disable_native_android_rules`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L685
46|`incompatible_remove_ctx_android_fragment`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L1001
47|`incremental_dexing`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L308
48|`internal_persistent_android_dex_desugar`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L881
49|`internal_persistent_busybox_tools`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L851
50|`internal_persistent_multiplex_android_dex_desugar`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L892
51|`internal_persistent_multiplex_busybox_tools`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L862
52|`legacy_main_dex_list_generator`|t=`Label`|d=`"null"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L957
53|`non_incremental_per_target_dexopts`|t=`List<String>`|d=`"--positions"`|c=`Converters.CommaSeparatedOptionListConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L395
54|`optimizing_dexer`|t=`Label`|d=`"null"`|c=`EmptyToNullLabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L970
55|`output_library_merged_assets`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L949
56|`persistent_android_dex_desugar`|t=`Void`|d=`"null"`|c=`-`|m=F|old=`-`|x=`{ "--internal_persistent_android_dex_desugar", "--strategy=Desugar=worker", "--strategy=DexBuilder=worker", }`|i=`-`|N=I|@L794
57|`persistent_android_resource_processor`|t=`Void`|d=`"null"`|c=`-`|m=F|old=`-`|x=`{ "--internal_persistent_busybox_tools", "--strategy=AaptPackage=worker", "--strategy=AndroidResourceParser=worker", "--strategy=AndroidResourceValidator=worker", "--strategy=AndroidResourceCompiler=worker", "--strategy=RClassGenerator=worker", "--strategy=AndroidResourceLink=worker", "--strategy=AndroidAapt2=worker", "--strategy=AndroidAssetMerger=worker", "--strategy=AndroidResourceMerger=worker", "--strategy=AndroidCompiledResourceMerger=worker", "--strategy=ManifestMerger=worker", "--strategy=AndroidManifestMerger=worker", "--strategy=Aapt2Optimize=worker", "--strategy=AARGenerator=worker", "--strategy=ProcessDatabinding=worker", "--strategy=GenerateDataBindingBaseClasses=worker" }`|i=`-`|N=I|@L735
58|`persistent_multiplex_android_dex_desugar`|t=`Void`|d=`"null"`|c=`-`|m=F|old=`-`|x=`{ "--persistent_android_dex_desugar", "--internal_persistent_multiplex_android_dex_desugar", }`|i=`-`|N=I|@L810
59|`persistent_multiplex_android_resource_processor`|t=`Void`|d=`"null"`|c=`-`|m=F|old=`-`|x=`{ "--persistent_android_resource_processor", "--modify_execution_info=AaptPackage=+supports-multiplex-workers", "--modify_execution_info=AndroidResourceParser=+supports-multiplex-workers", "--modify_execution_info=AndroidResourceValidator=+supports-multiplex-workers", "--modify_execution_info=AndroidResourceCompiler=+supports-multiplex-workers", "--modify_execution_info=RClassGenerator=+supports-multiplex-workers", "--modify_execution_info=AndroidResourceLink=+supports-multiplex-workers", "--modify_execution_info=AndroidAapt2=+supports-multiplex-workers", "--modify_execution_info=AndroidAssetMerger=+supports-multiplex-workers", "--modify_execution_info=AndroidResourceMerger=+supports-multiplex-workers", "--modify_execution_info=AndroidCompiledResourceMerger=+supports-multiplex-workers", "--modify_execution_info=ManifestMerger=+supports-multiplex-workers", "--modify_execution_info=AndroidManifestMerger=+supports-multiplex-workers", "--modify_execution_info=Aapt2Optimize=+supports-multiplex-workers", "--modify_execution_info=AARGenerator=+supports-multiplex-workers", }`|i=`-`|N=I|@L766
60|`persistent_multiplex_android_tools`|t=`Void`|d=`"null"`|c=`-`|m=F|old=`-`|x=`{ "--internal_persistent_multiplex_busybox_tools", "--persistent_multiplex_android_resource_processor", "--persistent_multiplex_android_dex_desugar", }`|i=`-`|N=I|@L825
#### 09. `com.google.devtools.build.lib.rules.android.BazelAndroidConfiguration.Options` — 1 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/rules/android/BazelAndroidConfiguration.java`
01|`merge_android_manifest_permissions`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L45
#### 10. `com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions` — 26 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/rules/apple/AppleCommandLineOptions.java`
01|`apple configuration distinguisher`|t=`ConfigurationDistinguisher`|d=`"UNKNOWN"`|c=`ConfigurationDistinguisherConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L222
02|`apple_platform_type`|t=`String`|d=`"macos"`|c=`PlatformTypeConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L194
03|`apple_platforms`|t=`List<Label>`|d=`""`|c=`LabelListConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L338
04|`apple_split_cpu`|t=`String`|d=`""`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L205
05|`catalyst_cpus`|t=`List<String>`|d=`"null"`|c=`CommaSeparatedOptionListConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L283
06|`experimental_include_xcode_execution_requirements`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L304
07|`experimental_objc_provider_from_linked`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L37
08|`experimental_prefer_mutual_xcode`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L158
09|`host_macos_minimum_os`|t=`DottedVersion.Option`|d=`"null"`|c=`DottedVersionConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L147
10|`incompatible_enable_apple_toolchain_resolution`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L328
11|`ios_minimum_os`|t=`DottedVersion.Option`|d=`"null"`|c=`DottedVersionConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L103
12|`ios_multi_cpus`|t=`List<String>`|d=`"null"`|c=`CommaSeparatedOptionListConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L231
13|`ios_sdk_version`|t=`DottedVersion.Option`|d=`"null"`|c=`DottedVersionConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L59
14|`macos_cpus`|t=`List<String>`|d=`"null"`|c=`CommaSeparatedOptionListConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L273
15|`macos_minimum_os`|t=`DottedVersion.Option`|d=`"null"`|c=`DottedVersionConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L136
16|`macos_sdk_version`|t=`DottedVersion.Option`|d=`"null"`|c=`DottedVersionConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L92
17|`tvos_cpus`|t=`List<String>`|d=`"null"`|c=`CommaSeparatedOptionListConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L263
18|`tvos_minimum_os`|t=`DottedVersion.Option`|d=`"null"`|c=`DottedVersionConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L125
19|`tvos_sdk_version`|t=`DottedVersion.Option`|d=`"null"`|c=`DottedVersionConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L81
20|`use_platforms_in_apple_crosstool_transition`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L347
21|`visionos_cpus`|t=`List<String>`|d=`"null"`|c=`CommaSeparatedOptionListConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L243
22|`watchos_cpus`|t=`List<String>`|d=`"null"`|c=`CommaSeparatedOptionListConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L253
23|`watchos_minimum_os`|t=`DottedVersion.Option`|d=`"null"`|c=`DottedVersionConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L114
24|`watchos_sdk_version`|t=`DottedVersion.Option`|d=`"null"`|c=`DottedVersionConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L70
25|`xcode_version`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L49
26|`xcode_version_config`|t=`Label`|d=`"@bazel_tools//tools/cpp:host_xcodes"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L293
#### 11. `com.google.devtools.build.lib.rules.config.ConfigFeatureFlagOptions` — 2 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/rules/config/ConfigFeatureFlagOptions.java`
01|`all feature flag values are present (internal)`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L48
02|`enforce_transitive_configs_for_config_feature_flag`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L29
#### 12. `com.google.devtools.build.lib.rules.cpp.CppOptions` — 78 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/rules/cpp/CppOptions.java`
01|`apple_generate_dsym`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L850
02|`build_test_dwp`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L195
03|`cc_dotd_files`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L948
04|`cc_include_scanning`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L927
05|`cc_output_directory_tag`|t=`String`|d=`""`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L133
06|`compiler`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L113
07|`conlyopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L257
08|`copt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L239
09|`crosstool_top`|t=`Label`|d=`"@bazel_tools//tools/cpp:toolchain"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L102
10|`cs_fdo_absolute_path`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L396
11|`cs_fdo_instrument`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`{"--copt=-Wno-error"}`|N=I|@L384
12|`cs_fdo_profile`|t=`Label`|d=`"null"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L445
13|`custom_malloc`|t=`Label`|d=`"null"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L311
14|`cxxopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L248
15|`dynamic_mode`|t=`DynamicMode`|d=`"default"`|c=`DynamicModeConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L205
16|`enable_propeller_optimize_absolute_paths`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L465
17|`enable_remaining_fdo_absolute_paths`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L457
18|`experimental_cc_implementation_deps`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L970
19|`experimental_cpp_compile_resource_estimation`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L1014
20|`experimental_cpp_modules`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L981
21|`experimental_generate_llvm_lcov`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L877
22|`experimental_inmemory_dotd_files`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L691
23|`experimental_link_static_libraries_once`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L998
24|`experimental_omitfp`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L706
25|`experimental_save_feature_state`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L819
26|`experimental_unsupported_and_brittle_include_scanning`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L906
27|`experimental_use_cpp_compile_action_args_params_file`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L895
28|`experimental_use_llvm_covmap`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L737
29|`fdo_instrument`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`{"--copt=-Wno-error"}`|N=I|@L349
30|`fdo_optimize`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L361
31|`fdo_prefetch_hints`|t=`Label`|d=`"null"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L420
32|`fdo_profile`|t=`Label`|d=`"null"`|c=`EmptyToNullLabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L435
33|`fission`|t=`List<CompilationMode>`|d=`"no"`|c=`FissionOptionConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L179
34|`force_pic`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L218
35|`grte_top`|t=`Label`|d=`"null"`|c=`LibcTopLabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L669
36|`host_compiler`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L121
37|`host_conlyopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L626
38|`host_copt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L604
39|`host_cxxopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L615
40|`host_grte_top`|t=`Label`|d=`"null"`|c=`LibcTopLabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L680
41|`host_linkopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L659
42|`host_per_file_copt`|t=`List<PerLabelOptions>`|d=`"null"`|c=`PerLabelOptions.PerLabelOptionsConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L637
43|`incompatible_disable_legacy_cc_provider`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L801
44|`incompatible_disable_nocopts`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L839
45|`incompatible_dont_enable_host_nonhost_crosstool_features`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L752
46|`incompatible_enable_cc_toolchain_resolution`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L810
47|`incompatible_make_thinlto_command_lines_standalone`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L763
48|`incompatible_remove_legacy_whole_archive`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L790
49|`incompatible_require_ctx_in_configure_features`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L772
50|`incompatible_use_cpp_compile_header_mnemonic`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L886
51|`incompatible_use_specific_tool_files`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L828
52|`incompatible_validate_top_level_header_inclusions`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L781
53|`interface_shared_objects`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L165
54|`legacy_whole_archive`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L322
55|`linkopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L275
56|`ltobackendopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L293
57|`ltoindexopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L284
58|`memprof_profile`|t=`Label`|d=`"null"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L518
59|`minimum_os_version`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L141
60|`objc_enable_binary_stripping`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L866
61|`objc_generate_linkmap`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L858
62|`objc_use_dotd_pruning`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L960
63|`objccopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L266
64|`per_file_copt`|t=`List<PerLabelOptions>`|d=`"null"`|c=`PerLabelOptions.PerLabelOptionsConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L563
65|`per_file_ltobackendopt`|t=`List<PerLabelOptions>`|d=`"null"`|c=`PerLabelOptions.PerLabelOptionsConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L584
66|`process_headers_in_dependencies`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L229
67|`propeller_optimize`|t=`Label`|d=`"null"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L491
68|`propeller_optimize_absolute_cc_profile`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L473
69|`propeller_optimize_absolute_ld_profile`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L482
70|`proto_profile`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L533
71|`proto_profile_path`|t=`Label`|d=`"null"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L541
72|`save_temps`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L552
73|`share_native_deps`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L717
74|`start_end_lib`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L152
75|`strict_system_includes`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L727
76|`strip`|t=`StripMode`|d=`"sometimes"`|c=`StripModeConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L337
77|`stripopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L302
78|`xbinary_fdo`|t=`Label`|d=`"null"`|c=`EmptyToNullLabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L407
#### 13. `com.google.devtools.build.lib.rules.java.JavaOptions` — 36 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/rules/java/JavaOptions.java`
01|`bytecode_optimization_pass_actions`|t=`int`|d=`"1"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L289
02|`bytecode_optimizers`|t=`Map<String, Label>`|d=`"Proguard"`|c=`LabelMapConverter.class`|m=F|old=`"experimental_bytecode_optimizers"`|x=`-`|i=`-`|N=I|@L239
03|`enforce_proguard_file_extension`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L297
04|`experimental_add_test_support_to_compile_time_deps`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L331
05|`experimental_enable_jspecify`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L423
06|`experimental_fix_deps_tool`|t=`String`|d=`"add_dep"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L180
07|`experimental_inmemory_jdeps_files`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L133
08|`experimental_java_classpath`|t=`JavaClasspathMode`|d=`"bazel"`|c=`JavaClasspathModeConverter.class`|m=F|old=`"java_classpath"`|x=`-`|i=`-`|N=I|@L122
09|`experimental_java_test_auto_create_deploy_jar`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L432
10|`experimental_local_java_optimization_configuration`|t=`Label`|d=`"null"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L265
11|`experimental_local_java_optimizations`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L253
12|`experimental_one_version_enforcement`|t=`OneVersionEnforcementLevel`|d=`"OFF"`|c=`OneVersionEnforcementLevelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L307
13|`experimental_run_android_lint_on_java_rules`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L342
14|`experimental_strict_java_deps`|t=`StrictDepsMode`|d=`"default"`|c=`StrictDepsConverter.class`|m=F|old=`"strict_java_deps"`|x=`-`|i=`-`|N=I|@L167
15|`experimental_turbine_annotation_processing`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L364
16|`explicit_java_test_deps`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L191
17|`host_java_launcher`|t=`Label`|d=`"null"`|c=`EmptyToNullLabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L202
18|`host_javacopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L60
19|`host_jvmopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L82
20|`incompatible_disallow_java_import_exports`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L414
21|`incompatible_multi_release_deploy_jars`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L405
22|`java_debug`|t=`Void`|d=`"null"`|c=`-`|m=F|old=`-`|x=`{ "--test_arg=--wrapper_script_flag=--debug", "--test_output=streamed", "--test_strategy=exclusive", "--test_timeout=9999", "--nocache_test_results" }`|i=`-`|N=I|@L149
23|`java_deps`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L114
24|`java_header_compilation`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`"experimental_java_header_compilation"`|x=`-`|i=`-`|N=I|@L105
25|`java_language_version`|t=`String`|d=`""`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L389
26|`java_launcher`|t=`Label`|d=`"null"`|c=`EmptyToNullLabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L211
27|`java_runtime_version`|t=`String`|d=`"local_jdk"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L373
28|`javacopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L51
29|`jvmopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L71
30|`one_version_enforcement_on_java_tests`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L319
31|`plugin`|t=`List<Label>`|d=`"null"`|c=`LabelListConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L354
32|`proguard_top`|t=`Label`|d=`"null"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L223
33|`split_bytecode_optimization_pass`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L276
34|`tool_java_language_version`|t=`String`|d=`""`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L397
35|`tool_java_runtime_version`|t=`String`|d=`"remotejdk_11"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L381
36|`use_ijars`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L94
#### 14. `com.google.devtools.build.lib.rules.objc.J2ObjcCommandLineOptions` — 2 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/rules/objc/J2ObjcCommandLineOptions.java`
01|`j2objc_dead_code_report`|t=`Label`|d=`"null"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L41
02|`j2objc_translation_flags`|t=`List<String>`|d=`"null"`|c=`Converters.CommaSeparatedOptionListConverter.class`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L31
#### 15. `com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions` — 13 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/rules/objc/ObjcCommandLineOptions.java`
01|`device_debug_entitlements`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L93
02|`experimental_objc_fastbuild_options`|t=`List<String>`|d=`"-O0,-DDEBUG=1"`|c=`CommaSeparatedOptionListConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L60
03|`incompatible_avoid_hardcoded_objc_compilation_flags`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L104
04|`incompatible_builtin_objc_strip_action`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L166
05|`incompatible_disable_native_apple_binary_rule`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L144
06|`incompatible_disallow_sdk_frameworks_attributes`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L120
07|`incompatible_objc_alwayslink_by_default`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L131
08|`incompatible_strip_executable_safely`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L155
09|`ios_memleaks`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L51
10|`ios_signing_cert_name`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L70
11|`ios_simulator_device`|t=`String`|d=`"null"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L40
12|`ios_simulator_version`|t=`DottedVersion.Option`|d=`"null"`|c=`DottedVersionConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L29
13|`objc_debug_with_GLIBCXX`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L82
#### 16. `com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options` — 11 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/rules/proto/ProtoConfiguration.java`
01|`cc_proto_library_header_suffixes`|t=`List<String>`|d=`".pb.h"`|c=`Converters.CommaSeparatedOptionSetConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L137
02|`cc_proto_library_source_suffixes`|t=`List<String>`|d=`".pb.cc"`|c=`Converters.CommaSeparatedOptionSetConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L146
03|`experimental_proto_descriptor_sets_include_source_info`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L58
04|`proto_compiler`|t=`Label`|d=`ProtoConstants.DEFAULT_PROTOC_LABEL`|c=`CoreOptionConverters.LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L67
05|`proto_toolchain_for_cc`|t=`Label`|d=`ProtoConstants.DEFAULT_CC_PROTO_LABEL`|c=`CoreOptionConverters.EmptyToNullLabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L104
06|`proto_toolchain_for_j2objc`|t=`Label`|d=`ProtoConstants.DEFAULT_J2OBJC_PROTO_LABEL`|c=`CoreOptionConverters.EmptyToNullLabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L94
07|`proto_toolchain_for_java`|t=`Label`|d=`ProtoConstants.DEFAULT_JAVA_PROTO_LABEL`|c=`CoreOptionConverters.EmptyToNullLabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L85
08|`proto_toolchain_for_javalite`|t=`Label`|d=`ProtoConstants.DEFAULT_JAVA_LITE_PROTO_LABEL`|c=`CoreOptionConverters.LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L76
09|`protocopt`|t=`List<String>`|d=`"null"`|c=`-`|m=T|old=`-`|x=`-`|i=`-`|N=I|@L49
10|`strict_proto_deps`|t=`StrictDepsMode`|d=`"error"`|c=`CoreOptionConverters.StrictDepsConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L113
11|`strict_public_imports`|t=`StrictDepsMode`|d=`"off"`|c=`CoreOptionConverters.StrictDepsConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L125
#### 17. `com.google.devtools.build.lib.rules.python.PythonOptions` — 6 options; N=I; pinned `src/main/java/com/google/devtools/build/lib/rules/python/PythonOptions.java`
01|`build_python_zip`|t=`TriState`|d=`"auto"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L35
02|`experimental_py_binaries_include_label`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L83
03|`incompatible_default_to_explicit_init_py`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L46
04|`incompatible_python_disallow_native_rules`|t=`boolean`|d=`"false"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L71
05|`incompatible_remove_ctx_py_fragment`|t=`boolean`|d=`"true"`|c=`-`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L92
06|`python_native_rules_allowlist`|t=`Label`|d=`"null"`|c=`LabelConverter.class`|m=F|old=`-`|x=`-`|i=`-`|N=I|@L60

### Bazel 9.2 non-native configuration-input and owner ledger

Native fragments are only the first checksum input. Pinned source fixes the
remaining input order and assigns each retained fact one future Slug owner:

| Input or operation | Pinned Bazel owner and ordering | Eventual Slug owner |
|---|---|---|
| RC sections, imports, `--config`, explicit argv, expansions, implicit requirements, old names, repeats | `BlazeOptionHandler#parseArgsAndConfigs` and `ConfigExpander`: flatten in command/RC order before typed native/Starlark parsing; recursive configs cycle-error and missing definitions error | `slug_commands_v2`; it alone creates the ordered normalized request and pre-DICE diagnostics |
| Platform-specific config selection | `ConfigExpander#getPlatformName`: an enabled host OS selects an existing command-specific config | `slug_workspace_v2` owns the OS observation; `slug_commands_v2` owns expansion |
| Host CPU defaults | `CoreOptions` plus `AutoCpuConverter`: empty CPU/host CPU becomes an OS/architecture token; explicit values bypass it | `slug_workspace_v2` observation consumed by the future context-aware configuration converter |
| Target/host platform options | `PlatformOptions#getNormalized` and `BuildConfigurationKeyProducer`: default target is host, keep first target platform, keep-last extra toolchains | typed value in `slug_configuration_v2`; context-free P normalization there after its metadata packet |
| Repository-mapped labels and flag aliases | `PlatformFunction`, `ParsedFlagsFunction`, `ParsedFlagsValue.Key`: package/main-repo context and mapping participate in the parse key; unavailable dependencies restart | `slug_identity_v2` owns canonical labels; `slug_bzlmod_v2` owns the repository mapping; core producer composes them without reparsing in commands |
| `platform_mappings` path, bytes, and parse | `PlatformMappingFunction`: workspace-relative path; ordered package-root search; missing default means empty; missing explicit, directory, malformed, or duplicate entry errors | `slug_workspace_v2` owns path observation; `slug_loading_v2` owns mapping-file loading/parsing; core owns the DICE composition |
| Selected platform `flags` | `PlatformProducer` and `ParsedFlagsFunction`: platform flags win over mapping, parse in the platform package mapping, and merge over source options | `slug_loading_v2` owns platform target/flag loading; core producer owns application |
| Command Starlark setting values and defaults | `StarlarkOptionsParser`, `BuildOptions#of`, `ParsedFlagsValue`: load canonical setting label/type/default; explicit default elides unless defaults are requested; platform flags may remove a value equal to default | `slug_loading_v2` owns schema/default discovery; typed value lives in `slug_configuration_v2`; core producer owns default elision |
| Starlark option scopes | `BuildOptionsScopeFunction`: canonical label-to-scope map is separately checksum-encoded; missing project/package dependencies restart | `slug_loading_v2` owns scope discovery; core producer owns the final map |
| Per-target `PROJECT.scl` scope baseline | `BuildConfigurationKeyProducer#possiblyApplyScopes`: after platform processing, out-of-scope values reset to baseline or disappear if absent there | `slug_loading_v2` owns project discovery/evaluation; core producer owns reset/removal |
| Top-level `PROJECT.scl` configuration | `BuildTool` and `AnalysisPhaseRunner`: selected project options are reparsed into the initial native/Starlark options before BuildOptions creation | `slug_loading_v2` owns project input; `slug_commands_v2` owns its single ordered merge before request identity |
| Native and Starlark transitions | `TransitionApplier`, `FunctionTransitionUtil`, and `BuildConfigurationKeyMapProducer`: preserve split-map order, then apply platform/mapping and scope processing independently to every result | `slug_analysis_v2` owns transition evaluation; core configuration producer owns post-transition canonicalization |
| Per-attempt input injection and final configuration | Bazel creates the key only after the preceding stages | `slug_core_v2::runtime::dice` owns the committed transaction, Needs/retries, and complete result; build/cquery construct roots only from that result |

`BAZEL_SH`, `PATH`, and later shell/action-environment defaults are configured
or action behavior but are not `BuildOptions#checksum` inputs. They remain a
downstream Host/execution boundary rather than a hidden row in this producer.

### Configuration ledger result and serial implementation route

**Status: REPLAN to bounded serial packets.** Live Slug preserves raw build
flags but ignores them, rejects cquery configuration flags, carries no target
options on the daemon wire, injects only existing bzlmod/repository/path state,
and constructs `target:first-build` before the command transaction.
`ConfigurationKey` accepts an opaque checksum, stable serialization omits the
string-setting side channel, and transitions overlay that side channel without
recomputing identity.

A new lowest-level `slug_configuration_v2` crate is the sole owner of immutable
configuration descriptors and values. It may initially depend only on retained
utility crates; it never owns IO, DICE, command parsing, wire transport, or
loading. This avoids both a core-to-commands dependency and making commands
depend on analysis. Commands own argv/RC normalization, server/CLI are lossless
transport adapters, workspace/bzlmod/loading retain their existing observation
and graph domains, core owns the committed producer, and analysis consumes only
a complete configuration for keys and acyclic transitions.

The lifecycle contract is frozen. Parse/normalization failure occurs before
DICE, injects nothing, and cannot replace an accepted snapshot. During graph
production, the structural union of Needs precedes all sibling semantic errors;
Needs are invalid/nonterminal, while complete errors and values are valid and
structurally equal independent of events. Retry events never publish. Equal
normalized requests do not invalidate. Native request, Host, repository
mapping, mapping/platform/BUILD/`.bzl`/`PROJECT.scl`, or setting changes
invalidate only the producer and downstream roots. Fresh one-shot C0 equals
daemon C0; same-daemon `C0 -> C1 -> C0` restores the exact structure, key, and
checksum for native, platform/mapping, Starlark/default/scope, and acyclic
transition changes. No caller checksum, `first-build`, REAPI digest, direct
filesystem bypass, lock across DICE, or retry event may substitute.

The implementation order is: metadata/cache grammar; native value-cohort and
rendering design; a pure converter/default and context-free normalization
kernel; Host/repository-context converters; shared command/wire identity;
Host/platform graph inputs; Starlark values and scopes; complete-fragment P/C/T
normalization and a transactional producer plus structural checksum/key/acyclic
transition replacement; and build/cquery root integration. Configured artifacts,
per-action platforms, Bazel ActionKey, and aquery remain later serial owners.

Run next only `WP-6-m2-native-configuration-metadata-and-cache-grammar`.
Reserved review rejected executing every converter in this first packet:
`AutoCpuConverter` needs Host facts, label-family converters need repository
context, RC/expansion belongs to commands, and Starlark settings need graph
schemas. The bounded packet creates `slug_configuration_v2`, retains the exact
341-row descriptor registry, and implements only FQCN/option ordering plus the
native `NULL`/`EMPTY`/quoted-escape cache-field grammar.

Allow only root `Cargo.toml` and the new crate's `Cargo.toml`, `src/lib.rs`,
`src/native/{mod,registry,cache_grammar,tests}.rs`, plus the single new-crate
allowlist entry in `scripts/v2_archive_status.sh`: eight files, 2,400
production, 1,400 test, and 3,800 total formatted net lines. The workspace
intentionally ignores `Cargo.lock`; do not track or modify it. Use a static
descriptor slice—no generated source, map, interner, cache, global, weak hash,
wire, DICE, Host access, converter execution, normalized values, mixed checksum,
or existing-crate edit. The implementation must apply the Buck2 utility-reuse
skill and prove 17/341 count and uniqueness, exact class/option ordering, all
metadata through an independent compact expected row for every descriptor,
selected complex rows, and cache escaping from the pinned ledger.
Configured-target dependency cycles remain deferred by user approval.

### Native configuration metadata/cache grammar accepted (2026-08-04)

Commit `b043d54d` accepts the new lowest-level `slug_configuration_v2` crate.
Its immutable static slice retains all seventeen FQCN-ordered classes and 341
option-name-ordered descriptors, including every pinned name/old name, field
type, raw default, converter identifier, repeat/expansion/implicit metadata,
and I/P/C/T normalizer marker. An independent compact 341-row test table
compares every field. The three previously omitted source rows are explicit.

The crate also owns only the outer Bazel `OptionsBase` cache-field grammar:
distinct `NULL`, empty-list `EMPTY`, and quoted scalars escaping backslash and
double quote. It executes no converter or normalizer and computes no class or
mixed checksum. Buck2 utility review selected borrowed static strings and a
static slice; no dependency, map, interner, cache, global, or hash was added.
The archive guard received only the new V2 crate exclusion, while the ignored
workspace lockfile remains untracked and unchanged.

Root validation passed four crate tests plus doctests, focused check,
formatting, an exact source-ledger comparison, archive, scope, cap, and diff
checks. Independent retained-representation review matched all 341 rows
against pinned Bazel `8220c619`, accepted the cache bytes and eight-file scope,
and measured 446 production/600 test/1,047 total formatted net lines against
2,400/1,400/3,800 caps.

### Native value/converter successor REPLAN (2026-08-04)

**Status: design before Rust.** The proposed combined context-free converters,
typed defaults, and whole P/C/T normalization packet was not truthful. The 341
rows partition into 288 apparently pure rows, seven Java-regex-dependent rows,
five Host-dependent rows, and 41 repository/package/loading-dependent rows.
`AutoCpuConverter`, path handling, and test resource macros observe Host state;
label families require package/repository context; six symbolic label defaults
are source expressions rather than parsed literals.

Even the normalizers cross that boundary. P truncates the label-valued
`platforms` list and C deduplicates label-valued `flag_alias`; doing so before
repository-context conversion can hide an invalid discarded value. T and the
remaining P/C members appear pure, but Bazel converts every occurrence before
`BuildOptions` invokes `getNormalized`. Command-owned old names, repeats,
boolean negation, six expansions, and two implicit requirements must likewise
be flattened in order before configuration conversion, not reimplemented in
the configuration crate.

The existing formatter proves only outer escaping, not Java `toString()` for
forty field types. Lists, entries/maps, enum spellings, environment values,
duration maps, per-label values, and regexes need explicit structural equality
and rendering rules. Java regex generation and lone UTF-16 surrogate behavior
are named stops; Rust regex or UTF-8 lexical order cannot silently substitute.

Run next only `WP-6-m2-native-value-cohort-and-rendering-design`. Classify every
descriptor into disjoint pure, Host, repository/loading, or unsupported
converter cohorts; separately inventory the 45 repeatable, 13 old-name, six
expansion, and two implicit-requirement command rows; freeze the command-
flattened-occurrence precondition; pin default special-null/empty behavior and
the six symbolic defaults; define the closed value algebra and exact Java
rendering/equality for every admitted pure family; and separate context-free
P/C/T members from label-bearing members without changing Bazel's convert-
before-normalize error semantics.

The design must select retained representations under the Buck2 utility skill,
including compact dynamic strings, immutable shared slices, clone cost, memory
accounting, deterministic Java UTF-16 ordering, and no runtime registry map or
global interner. It must return a bounded pure-codec implementation packet or
`REPLAN`. Allow only Stage 6 plus current/canonical scheduling and one terminal
routing row at 720 formatted documentation lines. No Rust, test, fixture,
dependency, oracle, generated data, command/wire, DICE, checksum, or downstream
work is authorized. Configured-target cycles remain user-deferred.
