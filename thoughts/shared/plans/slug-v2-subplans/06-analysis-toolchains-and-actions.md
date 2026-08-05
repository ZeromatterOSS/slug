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
configuration is not a bounded constant. Bazel registers fourteen native
`FragmentOptions` classes, sorts them by fully qualified class name, and hashes
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
