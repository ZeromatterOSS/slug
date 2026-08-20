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

### 6.4A Immutable action-owner context

Schedule this owner after the first M1 request-revision/source-certificate
vertical and the just-in-time action/toolchain oracle subset, but before any
M7A packet admits broader action registration. Acceptance of the immutable
owner context is therefore an M7A entry gate. It is not blocked on the complete
Wave A fixture catalog, M8 bootstrap, M7B command breadth, or M9 exact identity
bytes. The current M7 repository source-consumer audit and its fixed cutover
remain unchanged.

Before Stage 6 admits another general action kind, named exec groups, applied
aspect actions, or multi-platform selection, retain one immutable owner
context at action registration. The target shape is:

```text
ActionOwnerContext {
    configured_owner,
    semantic_configuration_identity,
    admitted_checksum_or_display_projection,
    exec_group,
    execution_platform,
    exec_properties,
    selected_toolchain_context,
    aspect_provenance,
}
```

The exact Rust representation may be smaller or split into authenticated
projections, but every field that can affect action behavior must participate
structurally in equality and invalidation. The default exec group is an
explicit identity, not the absence of a context. An action retains the group-
selected platform, combined platform/target/group execution properties, and
selected toolchain context at creation time.

Do not later reconstruct an action's platform or properties from only its
label, the owner's current topology, a process-global current platform, or a
new toolchain-resolution run. `aquery`, Stage 7 execution, Slug-native action
provenance, progress observations, and future explain output must consume the
same retained owner context and action row. Bazel checksum and ActionKey bytes
remain separate M9 domains; admitting Slug-native internal bytes does not
permit semantic inputs to be omitted.

The design packet must audit the current `ConfiguredActionExecGroup::Default`
and `ConfiguredActionView` topology-derived platform path, then prove:

- default and named groups select and retain distinct contexts;
- two actions of one owner may use different groups/platforms/properties;
- `cfg = "exec"` dependencies use the group-selected exec configuration;
- aspect-created actions retain their applicable aspect/owner context;
- configuration, platform, property, registration, mapping, and toolchain
  edits invalidate through named DICE dependencies;
- aquery and REAPI projections are derived from the identical retained row;
- conflicting-output and other diagnostics preserve Bazel's error precedence;
  and
- retained data is immutable, compact, `Allocative`, and released with the
  owning analysis value.

Compatibility classification:

- selected platform/toolchain, exec-group behavior, property merging, and
  action-visible provenance are **exact** for admitted Bazel 9.2 slices;
- private Rust rows, compact identities, and added explain fields are
  **Slug-native**; and
- exact configuration checksum, configured output token, and Bazel ActionKey
  bytes remain **deferred to M9**.

Use the Stage 1 provider, action-conflict, aquery-topology, and toolchain
fixture backlog before implementation. This section freezes a future owner
contract; it does not modify the accepted bounded FileWrite action surface or
widen the active M7 packet.

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
rows partition into 287 pure rows, eight Java-regex-dependent rows,
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

### Native value cohort and rendering design (2026-08-04)

**Decision: ACCEPT a smaller pure value/default/rendering kernel; defer all
whole-fragment P/C/T normalization and every dynamic-context converter.** This
is a pinned-source design for Bazel `8220c6198837d5c13d53fea211cf3282aa12408a`.
It supersedes the broad converter proposal in the preceding REPLAN without
changing its metadata/cache-grammar acceptance. The existing static registry is
the complete routing source: all 341 rows are classified below, and no source
generation, runtime descriptor map, interner, cache, global, or hash is needed.

#### Cohort-complete converter and default routing

The counts are deliberately by descriptor, not merely by converter class. They
are disjoint and sum to `287 + 8 + 5 + 41 = 341`. An identifier with a
conditional branch is placed in the first cohort that cannot be implemented
exactly by a context-free native value kernel.

| Cohort | Count | Descriptor routing and pinned reason |
|---|---:|---|
| Pure now | 287 | `Converter.class` for the 227 built-in field-type rows (`boolean` 167, `int` 5, `String` 25, `TriState` 4, `Void` 6, repeated `List<String>` 20); plus the 60 rows in the explicit pure families below. `Converter.Contextless` is necessary but not by itself sufficient; the Java-regex exception remains separate. |
| Java-regex unsupported | 8 | `RegexFilter.RegexFilterConverter` 3 (`toolchain_resolution_debug`, `archived_tree_artifact_mnemonics_filter`, `instrumentation_filter`); `ExecutionInfoModifier.Converter` 1 (`modify_execution_info`); `PerLabelOptions.PerLabelOptionsConverter` 3 (`host_per_file_copt`, `per_file_copt`, `per_file_ltobackendopt`); and `RunsPerTestConverter` 1 (`runs_per_test`). Each can compile a Java pattern; `RegexFilter` sorts/deduplicates components and renders the generated union, not the input. |
| Host | 5 | `AutoCpuConverter` 2 (`cpu`, `host_cpu`); `PathFragmentConverter` 1 (`shell_executable`); `PlatformMappingKeyConverter` 1 (`platform_mappings` explicit-path branch); `TestResourcesConverter` 1 (`default_test_resources`). `AutoCpuConverter` reads OS/CPU; `OptionsUtils.PathFragmentConverter` expands `~/` through `user.home` and uses the host path policy; resources resolve `HOST_CPUS`/`HOST_RAM`. |
| Repository/package/Starlark/loading | 41 | The label-bearing and conditional-label families listed below. `CoreOptionConverters.convertOptionsLabel` uses either `Label.PackageContext`, `RepositoryMapping`, or its first-round null context. No future generic parser may substitute a source-string label for that ownership. |

The 60 explicit pure rows are completely covered by this family table. A count
of one means one descriptor uses the identifier; adjacent qualified and
unqualified comma families have the same source behavior but remain distinct
metadata identifiers.

| Pure converter identifier(s) | Count | Typed family |
|---|---:|---|
| `AndroidManifestMergerConverter`, `ApkSigningMethodConverter`, `CancelConcurrentTests.Converter`, `ExecConfigurationDistinguisherSchemeConverter`, `IncludeConfigFragmentsEnumConverter`, `JavaClasspathModeConverter`, `ManifestMergerOrderConverter`, `OneVersionEnforcementLevelConverter`, `OutputDirectoryNamingSchemeConverter`, `OutputPathsConverter`, `PlatformTypeConverter`, `ShardingStrategyConverter`, `StrictDepsConverter`, `StripModeConverter` | 14 | Finite enum, case/alias rules pinned per declaring class. |
| `BooleanConverter` | 2 | Same boolean spelling family as the built-in boolean converter. |
| `CompilationMode.Converter`, `ConfigurationDistinguisherConverter`, `DynamicModeConverter` | 6 | Finite enum. |
| `CommaSeparatedOptionListConverter`, `Converters.CommaSeparatedOptionListConverter` | 15 | Ordered string list; empty members retained. |
| `CommaSeparatedOptionSetConverter`, `Converters.CommaSeparatedOptionSetConverter` | 4 | Lexically sorted, deduplicated string list. |
| `Converters.AssignmentConverter` | 1 | One ordered `String=String` entry. |
| `Converters.EnvVarsConverter` | 3 | `Set`, `Inherit`, or `Unset` environment occurrence. |
| `CoreOptionConverters.StrictDepsConverter` | 2 | Finite enum. |
| `DottedVersionConverter` | 10 | Parsed dotted-version option retaining Bazel's original raw input string. |
| `EmptyListConverter` | 1 | The one typed empty string list. |
| `FissionOptionConverter` | 1 | Ordered list of distinct compilation-mode enum values. |
| `TestTimeout.TestTimeoutConverter` | 1 | Ordered four-key timeout/duration map. |

The repository/loading cohort is complete: `LabelConverter` 16,
`LabelListConverter` 6, `LabelOrderedSetConverter` 1, `LabelMapConverter` 1,
`LabelToStringEntryConverter` 1, `EmptyToNullLabelConverter` 5,
`CoreOptionConverters.LabelConverter` 2,
`CoreOptionConverters.EmptyToNullLabelConverter` 3, `HostPlatformConverter` 1,
`LibcTopLabelConverter` 2, `RunUnderConverter` 1,
`CoreOptionConverters.CustomFlagConverter` 1, and
`CoreOptionConverters.FlagAliasConverter` 1. `RunUnderConverter` is a command
only for a non-label first token, and `CustomFlagConverter` is a raw define for
a non-label value; their label branches make their descriptor family deferred.

`RunsPerTestConverter` belongs wholly to the unsupported descriptor cohort
because an explicit `regex@N` occurrence delegates to `PerLabelOptions`. Its
annotated default `"1"` nevertheless has a source-pinned numeric branch that the
default materializer may admit as one `PerLabelOptions` seed without exposing a
general occurrence converter. This exception does not move the descriptor into
the pure cohort or authorize Java-pattern parsing.

The default source families are also exhaustive: 97 `"null"` annotations,
six symbolic label expressions, and 238 Java string literals. The symbolic
expressions are `PlatformOptions.DEFAULT_HOST_PLATFORM` plus the five
`ProtoConstants.DEFAULT_{PROTOC,CC_PROTO,J2OBJC_PROTO,JAVA_PROTO,JAVA_LITE_PROTO}_LABEL`
values. Their exact pinned texts must be retained in a private source-default
table, not passed to a command converter as the identifier spelling. Literal
label defaults (coverage tools, `xcode_version_config`, and `crosstool_top`)
remain in the repository cohort even though their default text is absolute.
`experimental_exec_config` is a pure `String` conversion only: resolving its
`@_builtins//...%...` value is later Starlark/loading work.

#### Command occurrence boundary and defaults

`slug_configuration_v2` accepts an already command-flattened, canonically named,
ordered sequence of individual occurrences. `slug_commands_v2` alone expands RC
sections/imports and `--config`, applies old-name warnings/canonicalization,
boolean `--no` spelling, flag aliases, repetitions, the six expansion flags,
and the two implicit requirements, then hands the ordered occurrences to this
crate. The descriptor metadata proves the independent counts: 45 repeatable,
13 old-name, 6 expansion, and 2 implicit-requirement rows. They are orthogonal
to the four conversion cohorts and must never be reinterpreted by a value
decoder.

For a field default, `FieldOptionDefinition#getDefaultValue` first recognizes
the exact Java annotation string `"null"`: it invokes no converter and yields
`None` for a nonrepeatable reference field or `[]` for a repeatable field. An
explicit command value `null` is ordinary converter input. A non-null default
is converted exactly once; for an `allowMultiple` field it is then wrapped if
the converter returned a scalar and left flat if it returned a list. Thus all
repeat defaults are empty except `runs_per_test="1"`, whose successful numeric
conversion is a one-element `List<PerLabelOptions>` default. Empty scalar,
empty nonrepeatable list, null, and repeatable empty list are not conflated.

The command layer must convert every flattened occurrence before it applies a
repeat merge or this crate receives a completed fragment. This preserves a
Bazel error in a discarded later occurrence: normalization is never permitted
to hide an invalid label, Host form, regex, or otherwise invalid duplicate.
The value kernel therefore parses one admitted occurrence/default only; it has
no argv grammar, priority, expansion, implicit-requirement, old-name, alias,
or merge API.

#### Closed admitted value algebra and cache rendering

The first implementation uses a closed `NativeValue` algebra, not `Debug`,
Rust `Display`, serde, or a generic map formatter. `NULL` is represented by
the enclosing optional field, not as a scalar value. The table fixes structural
equality and the Java `value.toString()` input to the already accepted outer
`OptionsBase.mapToCacheKey` grammar.

| Variant | Equality / retained order | Exact Java cache text before outer quoting |
|---|---|---|
| `Bool`, `Int`, `Text`, `TriState`, finite `Enum`, `DottedVersion` | Exact variant/value equality; dotted-version equality uses its original raw input string. | Java primitive/string spelling; `TriState` renders `AUTO`, `YES`, `NO`; enums render their declared Java name; dotted version renders its original raw `stringRepresentation`. |
| `List(Arc<[NativeValue]>)` | Length and element equality, insertion order. | Java list form: `[` + element cache text joined by `, ` + `]`. Only a zero-length list routes to outer `EMPTY`; `[""]` renders `[]`-style element text rather than `EMPTY`. |
| `Entry { key, value }` | Both fields equal. | `key=value`, matching Java map-entry rendering. This is used by `--define`. |
| `OrderedMap(Arc<[(NativeValue, NativeValue)]>)` | Length, pair order, and pair equality. | `{` + entry texts joined by `, ` + `}`. It is admitted only where the source fixes construction order: the `TestTimeout` enum order is `short`, `moderate`, `long`, `eternal`. No generic unordered-map adapter is admitted. |
| `Env::{Set, Inherit, Unset}` | Variant plus fields equal. | Pin the Java record strings individually: `Set[name=N, value=V]`, `Inherit[name=N]`, `Unset[name=N]`; do not depend on a Rust derived formatter. |
| `Fission(Arc<[CompilationMode]>)` | Ordered distinct enum members. | Java list rendering of enum spellings. |
| `RunsPerTestSeed { positive_runs: NonZeroI32 }` | Java positive-`int` equality and bounds; constructible only by the source-default materializer. | `(?:(?>.*)) Options: [N]`, matching the pinned singleton catch-all union from `RegexFilter#takeUnionOfRegexes` plus `PerLabelOptions#toString`; a surrounding list renders `[(?:(?>.*)) Options: [N]]`. |
| `Void` | Singleton equality. | This has no admitted explicit scalar path; the expansion-owned rows default through special-null. |

`OptionsBase.mapToCacheKey` routes only an empty typed list to `EMPTY`, a null
field to `NULL`, and every other `java_to_string()` result to a quoted scalar.
It escapes precisely backslash as `\\` and double quote as `\"`, then appends
`, `. A class key is `FQCN + "{" + ordered fields + "}"`; mixed checksums,
Starlark maps, and scope maps remain out of scope. List/map ordering is the
source-defined construction ordering, never Rust hash order.

Java `String` and Java lexical sorting are UTF-16-code-unit semantics. Rust
`str` is acceptable only after the input boundary proves a lossless valid-Unicode
domain and a helper implements Java UTF-16 comparison for the two admitted
sort/dedup families. A lone surrogate, a request to replace it with U+FFFD/NUL,
or any use of a Rust Unicode-code-point/byte ordering is a hard stop. The
existing Stage 9 rejection of `java_regex` is direct evidence that this is not a
small dependency substitution.

#### P/C/T normalizer routing and retained representation

`FragmentOptions#getNormalized` runs only after every field has converted.
Consequently this successor implements no whole P/C/T normalizer; it records
the exact later split instead of applying an unsafe partial normalization.

| Normalizer | Context-free members / future operation | Deferred members / reason |
|---|---|---|
| P | `extra_toolchains`: deduplicate while keeping each final occurrence's relative order. | `platforms`: label list truncated to first only after every label converts. `host_platform` is label, `platform_mappings` is Host, and `toolchain_resolution_debug` is Java-regex. Full P waits for all of them. |
| C | `allowed_cpu_values`: Java UTF-16 sort/dedup; `define`: final value per key then UTF-16 key sort; `features`: sorted enables followed by sorted disables, with disable winning; `action_env` and `host_action_env`: first-key position, last value. | `flag_alias` is label-bearing and must convert before its final-value/key-sort step. `cpu`/`host_cpu` are Host; `instrumentation_filter`/`modify_execution_info` are Java-regex; all remaining C fields are clone/identity members. Full C waits for all fields. |
| T | `test_env`: first-key position, last value. | `coverage_support` is label, `default_test_resources` is Host, and `runs_per_test` has its explicit regex branch. All other T fields are clone/identity members. Full T waits for all fields. |

For long-lived configuration values, use `CompactString` for dynamic valid
Unicode scalar text and `Arc<[NativeValue]>`/`Arc<[(NativeValue, NativeValue)]>`
for immutable repeated values and ordered maps. This matches existing V2
retained-string/slice practice and makes aggregate clones pointer-cheap. Derive
`Allocative` on retained value/container types. Use `Dupe` only for an
aggregate newtype proven pointer-cheap through its `Arc` members; do not label a
`CompactString`-carrying leaf as cheap to clone. The static descriptor registry
continues to use borrowed `&'static str`. No global interner, weak identity,
runtime map, or hash is justified: the 341 lookup/order remains a caller-owned
linear/static-slice concern until command routing supplies an ordered request.

#### Serial successor packet

Historical successor, superseded by the implementation REPLAN below:
`WP-6-m2-pure-native-value-default-and-rendering-kernel`.

- Owner/result: one `slug_configuration_v2` private pure value algebra, source
  default materializer, per-occurrence conversion for the 287 admitted paths,
  the one numeric `runs_per_test` default seed, and explicit Java cache
  projection. It must reject—not approximate—the eight Java-regex paths, five
  Host paths, 41 repository/loading paths, and the `runs_per_test` regex branch.
- Allowlist: `app/slug_configuration_v2/Cargo.toml`,
  `src/native/{mod.rs,cache_grammar.rs,tests.rs,value.rs,defaults.rs,convert.rs}`.
  No registry edit, root/workspace edit, external dependency/version, generated
  source, fixture/oracle growth, or scheduling/documentation edit.
- Caps: 1,550 production, 1,250 test, 2,800 formatted net lines over the seven
  existing/new crate files. `compact_str`, `allocative`, and `dupe` are already
  retained workspace utilities; add only the dependencies demonstrated by the
  selected representation, not a regex, map, or interner crate.
- Acceptance: source-pinned tests must cover all default families and all six
  symbolic texts; 287/8/5/41 routing totals; special-null versus explicit
  `null`; repeatable empty versus scalar/list empty; `runs_per_test="1"` and its
  exact `[(?:(?>.*)) Options: [1]]` cache seed;
  every admitted converter identifier and enum spelling; list/entry/map/env/
  duration/dotted-version cache text; `NULL`/`EMPTY`/escaping; equality and
  UTF-16 ordering on valid non-BMP inputs; and explicit refusal tests for every
  deferred family. Reuse the existing 341 independent metadata table rather
  than duplicating it.
- Stops: Java pattern generation/rendering, a lone surrogate or lossy UTF-8
  conversion, Host access, label/repository/package conversion, loading or
  Starlark resolution, any argv/RC/repeat/expansion/implicit/alias behavior,
  whole P/C/T normalization, generic unordered-map/record rendering, cache
  checksums, DICE/wire work, a runtime registry map/interner/hash, cap breach,
  or a second material source/rendering correction. Any stop is `REPLAN`, not a
  broadened packet.

After this kernel, a dedicated Host/repository conversion-context design must
first define ownership and exact value types. Only after all fields of a
fragment are valid and typed may a small full-fragment P/C/T normalization
packet run; it must prove convert-before-normalize errors and does not belong
in the pure kernel.

### Pure native kernel implementation REPLAN (2026-08-04)

`WP-6-m2-pure-native-value-default-and-rendering-kernel` stopped after its one
permitted correction. The first independent review found decimal-only timeout
parsing, lowercase `StripMode`, empty-fission `EMPTY`, positive/private runs
seeds, and Void-as-absence corrections. Root source review added canonical
`Duration.toString()` and uppercase `TestTimeout` enum-map keys. The corrected
seven-file draft passed 13 focused tests, check, formatting, archive, scope,
cap, and diff gates, but the terminal pinned-source audit then found a second
material miss: its DottedVersion parser neither enforced Java signed-`int`
bounds for numeric components/suffixes nor accepted underscore-bearing
descriptive components. Per packet and orchestration stops, the entire
unaccepted Rust diff was discarded with `apply_patch`; the worktree returned
to accepted commit `a5d135de`.

Pinned Bazel source is complete enough for a retry without a JVM oracle or a
new representation decision. `DottedVersion` uses component pattern
`(\\d+)([a-z0-9]*?)?(\\d+)?` and descriptive pattern `([a-z]\\w*)`, both
case-insensitive; each numeric capture passes through `Integer.parseInt`, the
first descriptive component terminates parsing, and the value retains the
entire original source string for equality and `toString()`. Discriminators are
maximum-versus-overflowing leading and suffix integers,
`1.internal_build`, ignored later bytes in `1.2.internal_build.!`, and exact
retention of trailing-zero text such as `1.0.0`.

Run next only
`WP-6-m2-pure-native-value-default-and-rendering-kernel-retry`. It restores the
same seven-file, 1,550 production/1,250 test/2,800 total implementation boundary
and all accepted 287/8/5/41, value, default, rendering, UTF-16, and refusal
decisions. In addition to the original acceptance matrix, tests must freeze the
entire correction set above: Java decimal/splitting timeout behavior, uppercase
timeout keys, canonical duration units, lowercase strip text, empty fission,
positive/private runs seed, absent-only Void, DottedVersion integer bounds,
descriptive underscore/early stop, and original-string retention. Any further
material source/rendering correction is another `REPLAN`; do not broaden into
an oracle, general `PerLabelOptions`/`runs_per_test` regex branch, contextual
converter, normalizer, checksum, command, wire, or DICE packet.
Configured-target dependency cycles remain user-deferred.

### Pure native kernel retry REPLAN (2026-08-04)

The clean `WP-6-m2-pure-native-value-default-and-rendering-kernel-retry`
stopped before validation and discarded its entire unaccepted seven-file diff.
Pinned source proved that generic lowercase enum retention/rendering is false:
`EnumConverter` matches case-insensitively against `value.toString()` and
returns the typed enum, so ordinary Java enums retain identity and render names
such as `MAIN`, `OFF`, `EXPLICIT`, and `DISABLED`; only specific enums such as
`CompilationMode` and `StripMode` override `toString()` to lowercase. Cache
text cannot serve as enum structural identity because distinct enum types can
render the same name. `BoolOrEnumConverter`, parameterized forced sharding,
and custom `PlatformType` further disprove one generic enum/string rule.

This is a new material rendering correction under the retry's zero-tolerance
stop. HEAD returned cleanly to `368f8fa1`; no Rust, registry, dependency,
fixture, oracle, or lockfile change remains. Two independent route audits
rejected an enum-only patch: the repeated misses show that all admitted pure
converter input, retained-value, equality, and `toString()` chains need one
complete pinned-source authority before more implementation.

Run next only `WP-6-m2-pure-native-converter-source-closure-ledger`. Add one
compact Stage 6 row for every existing pure descriptor and shared family rules
covering the 227 built-in plus 60 explicit rows. Each row must link its existing
ordinal/FQCN/name/type/converter to default route, explicit grammar/rejection,
case and converter-alias rules, typed returned value and equality, exact Java
rendering, pinned owner/line, and discriminator IDs. Enumerate every finite
member/alias/output; specify typed enum kind plus member/parameter rather than
rendered-string identity; preserve 287/8/5/41 and all command metadata.

At no more than 1,050 total/900 Stage 6 documentation lines, freeze later tests
for ordinary-uppercase versus override-lowercase enums, cross-kind inequality,
boolean/TriState/Integer synonyms and bounds, raw/list/set/entry/env/fission,
timeout/Duration, DottedVersion, defaults/runs/Void, `NULL`/`EMPTY`/escaping,
all descriptor defaults, one nondefault per family, and every finite enum
member. No Rust is authorized. Missing source chains, semantic family splits,
new representation/context/oracle needs, count changes, or cap breach are
`REPLAN`. Host, repository/loading, Java-regex/`PerLabelOptions`, command,
normalization, checksum, wire, DICE, and configured-cycle semantics remain
deferred.

### Java/Guava renderer authority evidence retry (2026-08-04)

This is evidence only: it adds no JVM, Java source, fixture, or runtime
dependency to Slug. Authority is Bazel 9.2 pinned source
`8220c6198837d5c13d53fea211cf3282aa12408a`, not the sibling worktree HEAD.
`bazel info java-home java-runtime` in `../bazel` reported:

```
INFO: Invocation ID: a58d770b-bc2f-46e6-be8b-4c024fde0be0
java-home: /run/media/system/Colossus/dev-home/.cache/bazel/_bazel_wgray/install/3e6f3b7d6fdac67aed908160850e082b/embedded_tools/jdk
java-runtime: OpenJDK Runtime Environment (build 25.0.2+10-LTS) by Azul Systems, Inc.
```

The exact `java-home/bin/java -version` is `openjdk version "25.0.2" 2026-01-20
LTS`, `Zulu25.32+17-CA (build 25.0.2+10-LTS)`, and Zulu 64-bit Server VM. The
embedded JRE has no compiler, so the nonpersistent probe used cached Zulu
`javac 25.0.1` at
`.../cache/repos/v1/contents/d9a6fe8fadec0ff7dc65b029aed97a8d9fe270e492ba26b4ac8fa49c55c6d31d/37f719ba-fef0-4b1e-a3e7-8870ce12dbe5/bin/javac`.
That compiler is evidence plumbing only: the probe's generated record
`ObjectMethods` and every `toString()` execute on Bazel's exact 25.0.2 JRE.

The isolated directory `/tmp/slug-renderer-authority-retry` contained only a
standard-Java `RendererProbe` (same-shaped EnvVar records, collections,
`EnumMap`, duration, and owner-equivalent enum overrides). Its UTF-16 input is
actual U+E000 then actual U+10000, and its exact discriminator is
`Stream.of("\uE000", "\uD800\uDC00").distinct().sorted().toList()`; it neither
joins strings nor skips the distinct-then-natural-sort path. Commands and
stdout were:

```
$ JAVAC=/run/media/system/Colossus/dev-home/.cache/bazel/_bazel_wgray/cache/repos/v1/contents/d9a6fe8fadec0ff7dc65b029aed97a8d9fe270e492ba26b4ac8fa49c55c6d31d/37f719ba-fef0-4b1e-a3e7-8870ce12dbe5/bin/javac
$ JRE=/run/media/system/Colossus/dev-home/.cache/bazel/_bazel_wgray/install/3e6f3b7d6fdac67aed908160850e082b/embedded_tools/jdk/bin/java
$ "$JAVAC" -version
javac 25.0.1
$ "$JAVAC" /tmp/slug-renderer-authority-retry/RendererProbe.java
$ "$JRE" -version
openjdk version "25.0.2" 2026-01-20 LTS
OpenJDK Runtime Environment Zulu25.32+17-CA (build 25.0.2+10-LTS)
OpenJDK 64-Bit Server VM Zulu25.32+17-CA (build 25.0.2+10-LTS, mixed mode)
$ "$JRE" -cp /tmp/slug-renderer-authority-retry RendererProbe
EMPTY=x=EMPTY,<SP><LF>
SINGLETON_EMPTY=x="[]",<SP><LF>
MULTI=x="[a, ]",<SP><LF>
ENTRY=x="a=b=c",<SP><LF>
SET=x="Set[name=N, value=V]",<SP><LF>
INHERIT=x="Inherit[name=N]",<SP><LF>
UNSET=x="Unset[name=N]",<SP><LF>
BOOL=x="true",<SP><LF>
INT=x="-16",<SP><LF>
STRING=x="a\\b\"c",<SP><LF>
ENUM=x="OFF",<SP><LF>
COMPILATION_MODE=x="dbg",<SP><LF>
STRIP_MODE=x="sometimes",<SP><LF>
PLATFORM_TYPE=x="mÄcos",<SP><LF>
DURATION=x="PT1H1M1S",<SP><LF>
TIMEOUT_DEFAULT=x="{short=PT1M, moderate=PT5M, long=PT15M, eternal=PT1H}",<SP><LF>
TIMEOUT_MIXED=x="{short=PT1H1M1S, moderate=PT1M1S, long=PT15M, eternal=PT1H}",<SP><LF>
UTF16_LIST=x="[𐀀, ]",<SP><LF>
UTF16_INPUT_UNITS=E000,D800 DC00
UTF16_OUTPUT_UNITS=D800 DC00,E000
```

`<SP><LF>` denotes output byte `0x20` followed by LF, not literal output text.
The input/output unit rows and bracketed list prove the required reversal.

| Retained value / concrete renderer authority | Exact `toString()` | Exact outer field bytes |
| --- | --- | --- |
| empty list / Bazel `OptionsBase.java:96-116` | not used | `x=EMPTY, ` |
| singleton-empty, multi-element, `ImmutableList` / Java SE 21 [`AbstractCollection#toString`](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/util/AbstractCollection.html#toString()); Guava `ImmutableList extends ImmutableCollection extends AbstractCollection` | `[]`; `[a, ]` | `x="[]", `; `x="[a, ]", ` |
| reverse U+E000/U+10000 input through `distinct().sorted().toList()` / Java SE 21 [`String#compareTo`](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/String.html#compareTo(java.lang.String)) UTF-16 order and `AbstractCollection#toString`; exact runtime probe | `[𐀀, ]` | `x="[𐀀, ]", ` |
| Guava `Maps.immutableEntry("a","b=c")` / `Maps.java:1470-1473` returns `AbstractMap.SimpleImmutableEntry`; Java SE 21 [`SimpleImmutableEntry#toString`](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/util/AbstractMap.SimpleImmutableEntry.html#toString()) | `a=b=c` | `x="a=b=c", ` |
| `EnvVar.Set`, `Inherit`, `Unset` / Bazel `Converters.java:599-617`; Java SE 21 [`Record#toString`](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/Record.html#toString()) | `Set[name=N, value=V]`; `Inherit[name=N]`; `Unset[name=N]` | corresponding quoted rows above |
| Boolean, Integer, String / Java SE 21 [`Boolean`](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/Boolean.html#toString()), [`Integer`](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/Integer.html#toString(int)), [`String`](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/String.html#toString()) | `true`; `-16`; `a\b"c` | `x="true", `; `x="-16", `; `x="a\\b\"c", ` |
| ordinary enum / Java SE 21 [`Enum#toString`](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/Enum.html#toString()); Bazel `EnumConverter.java:50-60` returns the typed member | `OFF` | `x="OFF", ` |
| `CompilationMode.DBG` / Bazel `CompilationMode.java:24-42` | `dbg` | `x="dbg", ` |
| `CppConfiguration.StripMode.SOMETIMES` / Bazel `CppConfiguration.java:119-133` | `sometimes` | `x="sometimes", ` |
| `PlatformTypeConverter.convert("MÄCOS")` / Bazel `AppleCommandLineOptions.java:393-406`, ASCII lowercase `String` result | `mÄcos` | `x="mÄcos", ` |
| `TestTimeout` / Bazel `TestTimeout.java:159-162` | lowercase `short`, `moderate`, `long`, `eternal` | timeout rows above |
| `Duration`; `EnumMap<TestTimeout,Duration>` / Java SE 21 [`Duration#toString`](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/time/Duration.html#toString()) and [`AbstractMap#toString`](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/util/AbstractMap.html#toString()) | `PT1H1M1S`; lowercase-key ordered maps | exact duration/default/mixed rows above |
| all nonempty/non-null values / Bazel `OptionsBase.java:51-52,96-116` | value bytes before outer escaping | `\\` becomes `\\\\`, `"` becomes `\"`, then surrounding `"..."`; only null is `NULL` |

Guava is bound by artifact and source, never an interface inference: pinned
`MODULE.bazel:175` names `com.google.guava:guava:33.5.0-jre`; pinned
`maven_install.json:217-221` locks JAR SHA-256
`1e301f0c52ac248b0b14fdc3d12283c77252d4d6f48521d572e7d8c4c2cc4ac7`.
The earlier temporary Maven JAR hashed exactly to that value. Official Guava
tag `v33.5.0` is `8868c096cfdabbe38170b6e395369c315cfb72a1`; its inspected
source provides the stated immutable-list chain and concrete entry return.
The retry probe and all earlier temporary Java/JAR/source material are deleted.

This closes only renderer authority. Preserve 287/8/5/41; descriptor/family
grammar, Rust, contextual and regex conversion, normalization, checksums,
wire, DICE, and configured-target cycles remain deferred. Any disagreement,
retained fixture, or production JVM implication is `REPLAN`.

Independent terminal review accepted the reverse-order UTF-16 discriminator,
exact renderer owners/bytes, runtime/compiler separation, Guava artifact/source
binding, cleanup, scope, and cap. Run next only
`WP-6-m2-pure-native-family-byte-contract-ledger-retry`: reuse this accepted
renderer authority and freeze one compact row per pure conversion/value/default
family, without repeating the 287 descriptor rows or adding Rust.

### Pure native family byte-contract ledger retry REPLAN (2026-08-04)

`WP-6-m2-pure-native-family-byte-contract-ledger-retry` produced a compact
family/route table, then source and two independent reviews exposed a new
retained-equality boundary. `TestShardingStrategyForced` allocates a fresh
object per `forced=N` conversion and defines no `equals` or `hashCode`, while
`OptionsBase.equals` compares option fields with `Objects.equals`. Two fresh
`forced=0` objects therefore have identical count and cache text but Java
identity inequality, contradicting the previously assumed structural native
`Forced(i32)` value. The draft also combined allow-empty and reject-empty list
grammars, omitted the repeatable assignment `N=[]` route, lacked the timeout
split-limit/empty-token discriminators, and left finite enum attachments
non-mechanical. The entire unaccepted Stage 6 ledger was discarded; no Rust,
probe, or generated artifact existed.

Run next only `WP-6-m2-forced-sharding-identity-evidence`. At no more than 150
Stage 6/220 total documentation lines, trace fresh parse, clone, equality,
hashing, cache key/checksum, BuildOptions/configuration identity, configured
key, and warm-daemon reuse at pinned Bazel source and live Slug owners. Decide
whether forced-object identity is discarded before semantic configuration
identity or survives and requires new scoped ownership. Do not invent a global
counter, interner, pointer identity, mutable singleton, or hidden DICE state.
If identity is canonicalized away, resume the corrected family ledger with a
structural forced-count value; if it survives, `REPLAN` to a separate retained
identity representation design. Preserve 287/8/5/41 and defer every family or
descriptor reconstruction, Rust, contextual/regex/Host/repository conversion,
normalization, checksum/wire implementation, DICE change, and configured-target
cycle semantic.

### Forced-sharding semantic-identity evidence (2026-08-04)

**Outcome 1 — structural `Forced(i32)` is authorized for the future semantic
configuration key.** No counter, interner, pointer identity, mutable singleton,
or hidden DICE state is permitted. This source-only decision is pinned to Bazel
`8220c6198837d5c13d53fea211cf3282aa12408a`; no probe is needed.

`ShardingStrategyConverter` allocates `new TestShardingStrategyForced(count)`
for every accepted `forced=` input (`TestShardingStrategy.java:25-59`). The
forced class stores the count and defines only `toString() -> "forced=" + count`
(`TestShardingStrategyForced.java:17-32`): it inherits `Object.equals/hashCode`.
Thus two fresh `forced=0` parses are unequal and may have different inherited
object hashes, while their text is identically `forced=0`. This identity is
real but is not semantic configuration identity.

| Boundary | Fresh `forced=0` versus fresh `forced=0` | Clone / semantic consequence |
| --- | --- | --- |
| Raw option object | unequal under inherited `Object.equals`; object hash need not match | `FragmentOptions.clone()` is shallow (`FragmentOptions.java:31-41`), and `BuildOptions.clone()` clones fragments but preserves that field reference (`BuildOptions.java:252-269`), so a clone remains reference-equal to its source. |
| `OptionsBase` direct comparison | unequal: fields use `Objects.equals` (`OptionsBase.java:119-140`); its map hash incorporates the forced object's inherited hash | `cacheKey()` does **not** use object equality/hash: it calls `value.toString()` (`OptionsBase.java:86-116`), yielding identical `forced=0` field text. |
| Raw `BuildOptions.matches` | unequal when it directly compares a stored field with a newly parsed converted value using `Objects.equals` (`BuildOptions.java:331-369`) | This is parsed-command matching, not the retained semantic configuration family or configured-target key; it stays outside the future pure converter/key boundary. |
| `BuildOptions` semantic identity | equal: checksum fingerprints fragment `cacheKey()` text (`BuildOptions.java:178-195`), then `equals/hashCode` use only that checksum (`BuildOptions.java:271-285`) | same checksum; the checksum cache keeps one representative by `putIfAbsent` (`BuildOptions.java:665-678`). |
| configuration/configured-target key | equal and convergent: `BuildConfigurationKey.create` interns keys and delegates equality/hash to `BuildOptions` (`BuildConfigurationKey.java:29-86`); `ConfiguredTargetKey` compares that configuration key and interns its result (`ConfiguredTargetKey.java:122-139,291-310`) | no identity distinction reaches configured-key identity or warm reuse. |

The live Slug key boundary is likewise structural: `ConfigurationKey` derives
`Eq`, `Hash`, and ordering over kind/checksum/root setting
(`app/slug_analysis_v2/src/key.rs:83-142`), and
`RootConfiguredTargetAnalysisKey` derives `Eq`/`Hash` over workspace plus the
resolved key (`app/slug_analysis_v2/src/dice.rs:125-215`). Its warm replay test
recomputes the same key and observes the same `Arc` value
(`app/slug_analysis_v2/tests/root_analysis.rs:345-385`). The existing cache
formatter is text-only (`app/slug_configuration_v2/src/native/cache_grammar.rs:1-34`);
it has no pointer/allocator channel. Therefore structural count equality is
both Bazel's configuration-key behavior and Slug's live retained-key boundary.

Resume only `WP-6-m2-pure-native-family-byte-contract-ledger-retry`, corrected
to use structural `Forced(i32)` for semantic configuration/cache identity while
documenting the raw Java `OptionsBase`/`BuildOptions.matches` boundary as
deferred command/parser behavior. Preserve 287/8/5/41 and defer contextual,
regex, Host/repository, normalization, checksum/wire implementation, DICE
changes, and configured-target-cycle work.

### Corrected pure native family byte-contract retry REPLAN (2026-08-04)

The corrected `WP-6-m2-pure-native-family-byte-contract-ledger-retry` retained
the accepted renderer and forced-identity boundaries and fixed list grammar,
repeatable assignment defaults, timeout split discriminators, and reverse
UTF-16 ordering. Terminal source review still found the table non-mechanical:
Android and Java grouped distinct enum converter/value families; `F-Text`
omitted scalar `N=None`; Bool/Int/Tri/list/set/Dotted/Env/generic-enum and
CompilationMode citations used inaccurate or broad ranges; and the default-only
runs branch omitted that positive `Integer.parseInt` accepts `+2` while keeping
the original `"+2"` option text. The entire unaccepted Stage 6 table was
discarded; no Rust, JVM probe, or artifact existed.

Run next only
`WP-6-m2-pure-native-family-source-anchor-and-enum-route-evidence`. At no more
than 190 Stage 6/260 total documentation lines, record exact full paths,
class/method owners, inclusive pinned ranges, and returned-value owners for
every future family; add one stable row per concrete finite enum with complete
members and `D/E/X` route template; bind scalar String special-null; and record
runs `D("1")` plus deferred `U("+2")` original-text behavior. Do not recreate
the family contract, descriptor rows, renderer/UTF-16/list/Entry/timeout/forced
evidence, Java, or Rust. An unverifiable range/member/alias, grouped enum,
descriptor-specific judgment need, or cap breach is `REPLAN`. On acceptance,
run only `WP-6-m2-pure-native-family-byte-contract-ledger-retry-2`. Preserve
287/8/5/41 and defer contextual/regex/Host/repository conversion,
normalization, checksum/wire implementation, DICE changes, and
configured-target cycles.

### Pure native family source-anchor evidence REPLAN (2026-08-04)

`WP-6-m2-pure-native-family-source-anchor-and-enum-route-evidence` split all
concrete enums, recorded scalar String special-null and runs `+2`, removed an
unadmitted assignment-list family, and corrected its first source-range audit.
Terminal review still found five class-versus-method citation failures:
ShardingStrategy, FissionOption, EmptyList, TestTimeout, and the combined
PerLabelOptions constructor/toString span. Because exact inclusive ownership
was the packet's acceptance criterion and its correction was exhausted, the
entire unaccepted Stage 6 table was discarded.

Run next only
`WP-6-m2-pure-native-family-source-anchor-and-enum-route-evidence-retry` under
the same 190 Stage 6/260 total documentation cap. Rebuild the source-only table
with all accepted correction facts plus these frozen anchors: shard interface
22-24, converter class 26-59/`convert` 35-58, forced class 17-33, ordinary enum
19-33; Fission class 39-58/`convert` 41-52; EmptyList class 436-447/`convert`
438-440; TestTimeout converter class 201-247/`convert` 207-241; PerLabelOptions
constructor 88-91 and `toString` 119-122. Do not recreate family bytes,
descriptors, accepted evidence, Java, or Rust. Any remaining path/class/method
range mismatch or second correction is `REPLAN`. On acceptance run only
`WP-6-m2-pure-native-family-byte-contract-ledger-retry-2`; preserve 287/8/5/41
and all existing deferrals, including configured-target cycles.

### Pure native family source-anchor and enum-route evidence retry (2026-08-04)

Every inclusive range below was read from `git show
8220c6198837d5c13d53fea211cf3282aa12408a:<path>`. This is source closure
only: it neither recreates the family-byte table nor reopens accepted renderer,
reverse-UTF-16, list/Entry/timeout, or forced-identity evidence. `N`, `D`,
`E`, `U`, and `X` retain their existing meanings.

| Future family | Full source path; explicit class range; explicit method range | Returned Java owner and parameter boundary |
| --- | --- | --- |
| F-Bool | `src/main/java/com/google/devtools/common/options/Converters.java`; aliases 39-43; `BooleanConverter` 46-66; `convert` 48-60 | `Boolean`; common true/false aliases are separate from the converter class. |
| F-Int | `src/main/java/com/google/devtools/common/options/Converters.java`; `IntegerConverter` 82-96; `convert` 84-90 | `Integer` via `Integer.decode`. |
| F-Text | `src/main/java/com/google/devtools/common/options/Converters.java`; `StringConverter` 69-79; `convert` 71-73. Special default: `src/main/java/com/google/devtools/common/options/FieldOptionDefinition.java`; `getDefaultValue` 327-359 | `String` identity. Annotation default `"null"` has `N=None` at the 337-339 special-null branch; it invokes no converter. |
| F-Tri | `src/main/java/com/google/devtools/common/options/Converters.java`; aliases 39-43; `TriStateConverter` 133-156; `convert` 135-150 | `TriState` AUTO/YES/NO; shared boolean aliases are a distinct input boundary. |
| F-Void | `src/main/java/com/google/devtools/common/options/Converters.java`; `VoidConverter` 162-175; `convert` 164-169 | `Void` absence (`null`), not a scalar sentinel. |
| F-Duration | `src/main/java/com/google/devtools/common/options/Converters.java`; `DurationConverter` 178-216; `convert` 182-210 | `java.time.Duration`; unit switch is 193-209. |
| F-AllowList | `src/main/java/com/google/devtools/common/options/Converters.java`; `SeparatedOptionListConverter` 253-287; constructor 259-264; `convert` 267-281. `CommaSeparatedOptionListConverter` 319-323/constructor 320-322 and `ColonSeparatedOptionListConverter` 342-346/constructor 343-345 | `ImmutableList<String>`; separator and `allowEmptyValues` constructors are distinct parameterized subclass boundaries. |
| F-NonEmptyList | `src/main/java/com/google/devtools/common/options/Converters.java`; `CommaSeparatedNonEmptyOptionListConverter` 330-335; constructor 332-334; inherited `SeparatedOptionListConverter.convert` 267-281 | `ImmutableList<String>`; `allowEmptyValues=false` is its distinct boundary. |
| F-StringSet | `src/main/java/com/google/devtools/common/options/Converters.java`; `SeparatedOptionSetConverter` 293-312; constructor 296-300; `convert` 303-306. `CommaSeparatedOptionSetConverter` 353-357/constructor 354-356 | deduped/sorted `ImmutableList<String>`; the set subclass, rather than a raw list, is the boundary. |
| F-Entry | `src/main/java/com/google/devtools/common/options/Converters.java`; `AssignmentConverter` 483-501; `convert` 486-495 | `Map.Entry<String,String>` from `Maps.immutableEntry`; first-`=` split is 487-494. |
| F-Env | `src/main/java/com/google/devtools/common/options/Converters.java`; `EnvVarsConverter` 627-670; `convert` 630-644. Value owner `EnvVar` 600-618 | sealed `EnvVar` records `Set`, `Inherit`, `Unset`; assignment form selects the record. |
| F-Dotted | `src/main/java/com/google/devtools/build/lib/rules/apple/DottedVersionConverter.java`; `DottedVersionConverter` 21-36; `convert` 24-30. Value owner `src/main/java/com/google/devtools/build/lib/rules/apple/DottedVersion.java`; `Option` 97-140; `fromString` 190-219, `isDescriptiveComponent` 223-225, `toComponent` 227-248, `parseNumber` 250-259 | `DottedVersion.Option`; original-text equality/hash is `Option.equals` 129-138 and `hashCode` 124-126. |
| F-Timeout | `src/main/java/com/google/devtools/build/lib/packages/TestTimeout.java`; `TestTimeoutConverter` 201-247; `convert` 207-241 | `Map<TestTimeout,Duration>` concretely built as `EnumMap` 221-240; value owner members 42-45, `toString` 159-162, `getTimeout` 176-178. |
| F-Runs-default | `src/main/java/com/google/devtools/build/lib/analysis/test/TestConfiguration.java`; `RunsPerTestConverter` 534-577; `convert` 536-542; `parseAsInteger` 544-554. Value owner `src/main/java/com/google/devtools/build/lib/analysis/config/PerLabelOptions.java`; constructor 88-91; `toString` 119-122 | default-only `D("1")` creates catch-all `.*` and preserves original text `"1"`. `U("+2")` is accepted by `Integer.parseInt` and preserves `"+2"`, but is deferred; no general `E` decoder. |
| F-Shard | `src/main/java/com/google/devtools/build/lib/analysis/test/TestShardingStrategy.java`; interface 22-24; `ShardingStrategyConverter` 26-59; `convert` 35-58. Value owners `src/main/java/com/google/devtools/build/lib/analysis/test/TestShardingStrategyForced.java`; class 17-33, constructor 20-22, `toString` 30-32; and `src/main/java/com/google/devtools/build/lib/analysis/test/TestShardingStrategyNotForced.java`; enum 19-33 | `TestShardingStrategy`: parameterized `Forced(int)` versus `EXPLICIT|DISABLED`; identity semantics remain the accepted forced-identity boundary. |
| F-Fission | `src/main/java/com/google/devtools/build/lib/rules/cpp/CppOptions.java`; `FissionOptionConverter` 39-58; `convert` 41-52 | `List<CompilationMode>`; `yes`/`no` special cases versus comma-mode conversion. |
| F-Platform | `src/main/java/com/google/devtools/build/lib/rules/apple/AppleCommandLineOptions.java`; `PlatformTypeConverter` 394-406; `convert` 398-400 | `String`; ASCII lower only, not an enum. |
| F-EmptyList | `src/main/java/com/google/devtools/build/lib/analysis/config/CoreOptions.java`; `EmptyListConverter` 436-447; `convert` 438-440 | `List<String>` always `ImmutableList.of()`. |

Generic finite-enum conversion is `src/main/java/com/google/devtools/common/options/EnumConverter.java`:
`EnumConverter` 32-88 and `convert` 52-60, which matches `toString()` ASCII
case-insensitively. Thus ordinary rows use `D(s∈members)`, `E(case(s))` to that
member, and `X(other)`; their renderer is Java `Enum#toString`. Each concrete
row below is separate; overrides name their own renderer.

| Stable enum route | Full converter source path; explicit converter class/method | Returned enum owner, complete route, renderer |
| --- | --- | --- |
| F-Enum-StrictDeps | `src/main/java/com/google/devtools/build/lib/analysis/config/CoreOptionConverters.java`; `StrictDepsConverter` 325-329; constructor 326-328; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `StrictDepsMode` in `src/main/java/com/google/devtools/build/lib/analysis/config/CoreOptionConverters.java:311-322`: OFF, WARN, ERROR, STRICT, DEFAULT; ordinary template. |
| F-Enum-ExecConfigurationDistinguisher | `src/main/java/com/google/devtools/build/lib/analysis/config/CoreOptions.java`; `ExecConfigurationDistinguisherSchemeConverter` 482-489; constructor 484-488; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `ExecConfigurationDistinguisherScheme` in `src/main/java/com/google/devtools/build/lib/analysis/config/CoreOptions.java:470-479`: LEGACY, OFF, FULL_HASH, DIFF_TO_AFFECTED; ordinary template. |
| F-Enum-OutputDirectoryNaming | `src/main/java/com/google/devtools/build/lib/analysis/config/CoreOptions.java`; `OutputDirectoryNamingSchemeConverter` 750-755; constructor 752-754; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `OutputDirectoryNamingScheme` in `src/main/java/com/google/devtools/build/lib/analysis/config/CoreOptions.java:740-747`: LEGACY, DIFF_AGAINST_BASELINE, DIFF_AGAINST_DYNAMIC_BASELINE; ordinary template. |
| F-Enum-OutputPaths | `src/main/java/com/google/devtools/build/lib/analysis/config/CoreOptions.java`; `OutputPathsConverter` 926-930; constructor 927-929; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `OutputPathsMode` in `src/main/java/com/google/devtools/build/lib/analysis/config/CoreOptions.java:913-923`: OFF, STRIP; ordinary template. |
| F-Enum-IncludeConfigFragments | `src/main/java/com/google/devtools/build/lib/analysis/config/CoreOptions.java`; `IncludeConfigFragmentsEnumConverter` 1186-1191; constructor 1188-1190; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `IncludeConfigFragmentsEnum` in `src/main/java/com/google/devtools/build/lib/analysis/config/CoreOptions.java:1173-1183`: OFF, DIRECT, TRANSITIVE; ordinary template. |
| F-Enum-AndroidConfigurationDistinguisher | `src/main/java/com/google/devtools/build/lib/rules/android/AndroidConfiguration.java`; `ConfigurationDistinguisherConverter` 52-57; constructor 54-56; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `ConfigurationDistinguisher` in `src/main/java/com/google/devtools/build/lib/rules/android/AndroidConfiguration.java:93-102`: MAIN, ANDROID; ordinary template. |
| F-Enum-ApkSigningMethod | `src/main/java/com/google/devtools/build/lib/rules/android/AndroidConfiguration.java`; `ApkSigningMethodConverter` 60-64; constructor 61-63; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `ApkSigningMethod` in `src/main/java/com/google/devtools/build/lib/rules/android/AndroidConfiguration.java:116-156`: V1, V2, V1_V2, V4; ordinary template. |
| F-Enum-AndroidManifestMerger | `src/main/java/com/google/devtools/build/lib/rules/android/AndroidConfiguration.java`; `AndroidManifestMergerConverter` 67-72; constructor 69-71; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `AndroidManifestMerger` in `src/main/java/com/google/devtools/build/lib/rules/android/AndroidConfiguration.java:159-185`: LEGACY, ANDROID, FORCE_ANDROID; ordinary template. |
| F-Enum-ManifestMergerOrder | `src/main/java/com/google/devtools/build/lib/rules/android/AndroidConfiguration.java`; `ManifestMergerOrderConverter` 75-80; constructor 77-79; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `ManifestMergerOrder` in `src/main/java/com/google/devtools/build/lib/rules/android/AndroidConfiguration.java:188-195`: ALPHABETICAL, ALPHABETICAL_BY_CONFIGURATION, DEPENDENCY; ordinary template. |
| F-Enum-AppleConfigurationDistinguisher | `src/main/java/com/google/devtools/build/lib/rules/apple/AppleCommandLineOptions.java`; `ConfigurationDistinguisherConverter` 386-391; constructor 388-390; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `ConfigurationDistinguisher` in `src/main/java/com/google/devtools/build/lib/rules/apple/AppleConfiguration.java:370-404`: UNKNOWN, APPLEBIN_IOS, APPLEBIN_VISIONOS, APPLEBIN_WATCHOS, APPLEBIN_TVOS, APPLEBIN_MACOS, APPLEBIN_CATALYST, APPLE_CROSSTOOL; ordinary template; filesystem accessor is not `toString`. |
| F-Enum-DynamicMode | `src/main/java/com/google/devtools/build/lib/rules/cpp/CppOptions.java`; `DynamicModeConverter` 61-65; constructor 62-64; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `DynamicMode` in `src/main/java/com/google/devtools/build/lib/rules/cpp/CppConfiguration.java:112-116`: OFF, DEFAULT, FULLY; ordinary template. |
| F-Enum-JavaClasspathMode | `src/main/java/com/google/devtools/build/lib/rules/java/JavaOptions.java`; `JavaClasspathModeConverter` 37-42; constructor 38-40; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `JavaClasspathMode` in `src/main/java/com/google/devtools/build/lib/rules/java/JavaConfiguration.java:49-58`: OFF, JAVABUILDER, BAZEL, BAZEL_NO_FALLBACK; ordinary template. |
| F-Enum-JavaOneVersionLevel | `src/main/java/com/google/devtools/build/lib/rules/java/JavaOptions.java`; `OneVersionEnforcementLevelConverter` 44-49; constructor 46-48; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `OneVersionEnforcementLevel` in `src/main/java/com/google/devtools/build/lib/rules/java/JavaConfiguration.java:61-74`: OFF, WARNING, ERROR; ordinary template. |
| F-Enum-Cancel | `src/main/java/com/google/devtools/build/lib/analysis/test/TestConfiguration.java`; nested `Converter` 303-307; constructor 304-306; inherited `BoolOrEnumConverter.convert` at `src/main/java/com/google/devtools/common/options/BoolOrEnumConverter.java:52-66` | `CancelConcurrentTests` in `src/main/java/com/google/devtools/build/lib/analysis/test/TestConfiguration.java:297-308`: NEVER, ON_FAILED, ON_PASSED; generic `D/E/X`, aliases `E(true)=ON_PASSED`, `E(false)=NEVER`; ordinary uppercase renderer. |
| F-Enum-CompilationMode | `src/main/java/com/google/devtools/build/lib/analysis/config/CompilationMode.java`; nested `Converter` 47-51; constructor 48-50; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `CompilationMode`/`toString` in `src/main/java/com/google/devtools/build/lib/analysis/config/CompilationMode.java:24-42`: FASTBUILD, DBG, OPT; `D/E` case-fold `fastbuild|dbg|opt`, `X(other)`; overridden lowercase renderer. |
| F-Enum-StripMode | `src/main/java/com/google/devtools/build/lib/rules/cpp/CppOptions.java`; `StripModeConverter` 68-72; constructor 69-71; inherited `EnumConverter.convert` at `src/main/java/com/google/devtools/common/options/EnumConverter.java:52-60` | `StripMode`/`toString` in `src/main/java/com/google/devtools/build/lib/rules/cpp/CppConfiguration.java:119-134`: ALWAYS, SOMETIMES, NEVER; `D/E` case-fold `always|sometimes|never`, `X(other)`; overridden lowercase renderer. |

This source closure preserves `287 + 8 + 5 + 41 = 341` and the independent 45
repeatable/13 old-name/6 expansion/2 implicit metadata. On acceptance, run only
`WP-6-m2-pure-native-family-byte-contract-ledger-retry-2`; contextual/regex,
Host/repository, normalization, checksum/wire implementation, DICE, and
configured-target cycles remain deferred.

Independent terminal review accepted all family/value/method anchors, the 16
separate finite-enum routes, String special-null, runs default/deferred `+2`,
scope, and cap. The next family-contract retry must cite this accepted source
closure rather than rediscover or broaden any owner.

### Pure native family byte-contract ledger retry 2 REPLAN (2026-08-04)

The unaccepted 64-line retry-2 ledger was discarded after terminal review found
materially incomplete default/null, repeat/whole-field cache, timeout, forced
radix/rendering, fission, and ordinary-enum substitutions. It made no source,
probe, Rust, descriptor, or runtime change.

Run only `WP-6-m2-pure-native-family-byte-contract-ledger-retry-3`, as a
docs-only synthesis of accepted `[R]` renderer, `[I]` identity, and `[S]`
source-route evidence. Freeze Bool `null→false`/concrete `D`, Tri
`null→AUTO`/concrete `D`, Text/Dotted `N=None`, and Void `N=None` plus
`E("null")=None`; list/Entry/Env `N=[]`, ordered post-conversion `A`, and
whole-field bytes; StringSet `D(s)` plus reverse UTF-16; timeout `D("-1")`,
mixed `3661,61,900,3600`, and split cases; nonnegative `Integer.decode`
decimal/`0x`/`#`/octal shard input with decimal `forced=N` rendering/structural
identity; fission `no` and `[dbg, fastbuild]`; all 16 explicit enum
substitutions; and runs `D("1")`/`U("+2")`. Preserve `287 + 8 + 5 + 41` and
defer raw parser, contextual, normalization, checksum/wire, DICE, and
user-approved later configured-target dependency cycles. Stop again on any
undefined route, byte, identity, scope, or cap fact; after acceptance, run
only mechanical 287-descriptor attachment.

### Pure native family byte-contract ledger retry 3 REPLAN (2026-08-04)

The unaccepted 59-line retry-3 ledger was discarded: terminal review found it
did not freeze nonrepeat versus attachment routes, repeat scope, empty/default
forms, full shard grammar, per-entry timeout fallback, dotted discriminators,
runs cache bytes, or explicit platform/default paths. No source lookup, probe,
descriptor, Rust, or runtime change occurred.

Run only `WP-6-m2-pure-native-family-byte-contract-ledger-retry-4`, docs-only
synthesis from accepted `[R]`/`[I]`/`[S]` plus the already closed Stage 6 value
algebra and Dotted discriminator evidence. Preserve every retry-3 freeze and
add AllowComma nonrepeat `D("")=Ø`/`EMPTY` versus repeat flattening; built-in
`m=T List<String>` Text repeat scope; StringSet `D("")=Ø` and reverse `E`;
Shard `D("explicit")`, ASCII prefix, full nonnegative `Integer.decode` forms
and decimal rendering; Fission `D("no")=Ø`; timeout per-entry fallback;
Dotted regex/descriptive early-stop; exact retained/cache runs `D("1")`; and
Platform `D/E` ASCII lower. Preserve `287 + 8 + 5 + 41` and cycle deferral.
Stop again on undefined grammar, default, byte, identity, scope, or cap fact;
after acceptance run only mechanical 287-descriptor attachment.

### Pure native family byte-contract ledger retry 4 REPLAN (2026-08-04)

The unaccepted 59-line retry-4 ledger was discarded because actual enum default
inventories and generic case-folded `D(s)` routes were not frozen, AllowComma
did not distinguish generic nonempty default from repeat flattening, Dotted
claimed a default for `1.0`, and runs did not state accepted-but-deferred `+2`.
No source lookup, probe, descriptor, Rust, or runtime change occurred.

Run only `WP-6-m2-pure-native-family-byte-contract-ledger-retry-5`, docs-only
synthesis from the same accepted evidence. Preserve every retry-4 fact; add the
actual enum `D` inventory/generic case-folding; AllowComma `D(s)` and
`D("-O0,-DDEBUG=1")`; Dotted only `N=None` with distinct `E("1.0")`/`E("1")`;
and accepted-but-deferred runs `U("+2")`. Preserve `287 + 8 + 5 + 41`, caps,
hard stops, and all deferrals including user-approved later configured-target
dependency cycles. After acceptance run only mechanical 287 attachment.

### Pure native family byte-contract ledger retry 5 REPLAN (2026-08-04)

Terminal review discarded the unaccepted 59-line retry-5 ledger. It did not
make clear that source-known `AllowColonList` and `NonEmptyCommaList` have no
admitted attachment or `A`; it treated F-Runs-default as an admitted pure
attachment instead of a default-materializer-only exception; and its Dotted
early-stop wording lost the full-input equality/hash/cache/rendering owner.
No source lookup, probe, descriptor, Rust, or runtime change occurred.

Run only `WP-6-m2-pure-native-family-byte-contract-ledger-retry-6`, a
docs-only synthesis of the same accepted `[R]`/`[I]`/`[S]` and closed Stage 6
facts. Preserve every retry-5 freeze, state both unadmitted list families
explicitly, exempt F-Runs-default from pure descriptor attachment in the
opening, and distinguish Dotted parsed-component early-stop from full original
input semantic identity/rendering. Preserve `287 + 8 + 5 + 41`, the 380/480
caps, hard stops, and all deferrals including user-approved later
configured-target dependency cycles. After acceptance run only mechanical 287
attachment.

### Pure native family byte-contract ledger retry 6 REPLAN (2026-08-04)

Independent review discarded the unaccepted 57-line retry-6 ledger despite its
reserved acceptance. It lacked a global retained-value structural
equality/order rule and did not freeze Fission's exact `yes` result/cache.
No source lookup, probe, descriptor, Rust, or runtime change occurred.

Run only `WP-6-m2-pure-native-family-byte-contract-ledger-retry-7`, docs-only
from the same accepted `[R]`/`[I]`/`[S]` and closed Stage 6 facts. Preserve
retry-6's unadmitted-list, Runs, and Dotted distinctions; unless a row differs,
retained values use structural equality/order for semantic `BuildOptions`
identity and raw parser-object matching remains deferred; add exact
`F-Fission E("yes")=[fastbuild, dbg, opt]→x="[fastbuild, dbg, opt]", `.
Preserve `287 + 8 + 5 + 41`, 380/480 caps, hard stops, all deferrals including
user-approved later configured-target dependency cycles. After acceptance run
only mechanical 287 attachment.

### Pure native family byte-contract ledger retry 7 (2026-08-05)

This docs-only synthesis uses accepted `[R]` renderer, `[I]` identity, `[S]`
routes, and closed Stage 6 value/Dotted facts. Unless a row differs, retained
values use structural equality/order for semantic `BuildOptions` identity; raw
parser-object matching is deferred. The 17 `F-*` rows freeze contracts;
source-known AllowColonList and NonEmptyCommaList have no admitted attachment
or `A`. F-Runs-default is default-materializer-only, never a pure attachment.

| Family | Exact grammar, route/value, and cache bytes |
| --- | --- |
| F-Bool | Case-folded `true,1,yes,t,y` / `false,0,no,f,n`; `null→false`, else `X`; `D("false")=false`, `E("yes")=true`, `C(true)=x="true", `. |
| F-Int | Signed-i32 `Integer.decode` decimal/`0x`/`#`/octal; malformed/overflow → `X`; `D(s)`, `E("-16")=-16`, `C(-16)=x="-16", `. |
| F-Text | Scalar `"null"` is `N=None→x=NULL, `; ordinary `D/E(s)=s`. Only admitted built-in `m=T List<String>` repeats: `N=[]→x=EMPTY, `, ordered `A[E(a),E(b)]=[a,b]→x="[a, b]", `. |
| F-Tri | `null→AUTO`; case-folded `auto` and Bool aliases → `AUTO|YES|NO`, else `X`; `D("auto")=AUTO`, `E("yes")=YES`, `C(YES)=x="YES", `. |
| F-Void | `N=None→x=NULL, `; `E("null")=None→x=NULL, `; other scalar → `X`. |
| F-Duration | `0` or nonnegative decimal plus `d|h|m|s|ms|ns`, else `X`; structural `Duration`; `D(s)`, `E("3661s")=PT1H1M1S→x="PT1H1M1S", `. |
| F-AllowCommaList | Nonrepeat `D(s)=split(s)`: `D("")=Ø→x=EMPTY, `, `D("-O0,-DDEBUG=1")=[-O0,-DDEBUG=1]→x="[-O0, -DDEBUG=1]", `, `E("a,,b")=[a,"",b]→x="[a, , b]", `. Admitted repeat `N=[]→x=EMPTY, `, `A[E("a,,b"),E("c")]=[a,"",b,c]→x="[a, , b, c]", `. |
| F-StringSet | Nonrepeat scalar-list: `D("")=Ø→x=EMPTY, `; `D(s)` comma-splits, de-duplicates, Java UTF-16-sorts; reverse `E(U+E000,U+10000)=[𐀀, ]→x="[𐀀, ]", `. |
| F-Entry | First `=` after nonempty key: `a=b=c→(a,b=c)`; `=v`/no `=` → `X`; `N=[]→x=EMPTY, `, `A[E(a=b),E(c=d)]=[a=b,c=d]→x="[a=b, c=d]", `. |
| F-Env | `N=V→Set(N,V)`, `N→Inherit(N)`, `=N→Unset(N)`; empty/`=` → `X`; `N=[]→x=EMPTY, `, `A[E(N=V),E(N),E(=N)]→x="[Set[name=N, value=V], Inherit[name=N], Unset[name=N]]", `. |
| F-Dotted | Case-insensitive component `(\d+)([a-z0-9]*?)?(\d+)?`, descriptive `([a-z]\w*)`, signed-i32 `Integer.parseInt`; first descriptive (including underscore) stops component parsing and later components do not affect parsed sequence, yet the full original input controls structural equality/hash/cache/rendering. Only `N=None→x=NULL, ` default; distinct `E("1.0")`/`E("1")` retain text, `C(1.0)=x="1.0", `. |
| F-Timeout | `.limit(6)` seconds: `2,`/`2,,3,4,5` accept, `1,2,,3,4` → `X`; nonpositive entries individually fall back in enum-order map. `D("-1")→{short=PT1M, moderate=PT5M, long=PT15M, eternal=PT1H}→x="{short=PT1M, moderate=PT5M, long=PT15M, eternal=PT1H}", `; `E("3661,61,900,3600")→{short=PT1H1M1S, moderate=PT1M1S, long=PT15M, eternal=PT1H}→x="{short=PT1H1M1S, moderate=PT1M1S, long=PT15M, eternal=PT1H}", `; `E("3661,0,0,0")→{short=PT1H1M1S, moderate=PT5M, long=PT15M, eternal=PT1H}→x="{short=PT1H1M1S, moderate=PT5M, long=PT15M, eternal=PT1H}", `. |
| F-Runs-default | Default-materializer-only, not a pure attachment: `D("1")` retains `[(?:(?>.*)) Options: [1]]→x="[(?:(?>.*)) Options: [1]]", `; `U("+2")` is accepted/deferred retaining `"+2"`; no general numeric/regex `E` or `A`. |
| F-Shard | `D("explicit")=EXPLICIT→x="EXPLICIT", `. ASCII-case-folded `forced=` accepts nonnegative signed `Integer.decode` decimal/`0x`/`#`/octal; malformed/negative → `X`; `E("FORCED=+0x10")=Forced(16)→x="forced=16", `; structural `Forced(i32)`, raw Java object identity deferred. |
| F-Fission | Nonrepeat scalar-list: `D("no")=Ø→x=EMPTY, `; `E("yes")=[fastbuild, dbg, opt]→x="[fastbuild, dbg, opt]", `; comma `E("dbg,fastbuild")=[dbg, fastbuild]→x="[dbg, fastbuild]", `; invalid mode → `X`. |
| F-Platform | `D(s)=E(s)=ASCII-lower(s)`: `MÄCOS→mÄcos`, never Unicode fold; structural `String`; `C(mÄcos)=x="mÄcos", `. |
| F-EmptyList | Nonrepeat scalar-list: every `D(s)`/`E(s)`/`Ø` is `[]→x=EMPTY, `. |

Every listed enum default `D(s)` case-folds through its ordinary route; `E` is
concrete nondefault and `X(other)` rejects. Ordinary cache is Java
`Enum#toString`; Compilation and Strip alone override lowercase.

| Enum route | Actual generic defaults, nondefault route, and exact cache |
| --- | --- |
| F-Enum-StrictDeps | `D(s∈{default,error,off})`; `E("warn")=WARN`; `C(DEFAULT)=x="DEFAULT", `. |
| F-Enum-ExecConfigurationDistinguisher | `D(s∈{off})=OFF`; `E("legacy")=LEGACY`; `C(OFF)=x="OFF", `. |
| F-Enum-OutputDirectoryNaming | `D(s∈{diff_against_dynamic_baseline})=DIFF_AGAINST_DYNAMIC_BASELINE`; `E("legacy")=LEGACY`; `C(DIFF_AGAINST_DYNAMIC_BASELINE)=x="DIFF_AGAINST_DYNAMIC_BASELINE", `. |
| F-Enum-OutputPaths | `D(s∈{off})=OFF`; `E("strip")=STRIP`; `C(OFF)=x="OFF", `. |
| F-Enum-IncludeConfigFragments | `D(s∈{off})=OFF`; `E("direct")=DIRECT`; `C(OFF)=x="OFF", `. |
| F-Enum-AndroidConfigurationDistinguisher | `D(s∈{MAIN})=MAIN`; `E("android")=ANDROID`; `C(MAIN)=x="MAIN", `. |
| F-Enum-ApkSigningMethod | `D(s∈{v1_v2})=V1_V2`; `E("v2")=V2`; `C(V1_V2)=x="V1_V2", `. |
| F-Enum-AndroidManifestMerger | `D(s∈{android})=ANDROID`; `E("legacy")=LEGACY`; `C(ANDROID)=x="ANDROID", `. |
| F-Enum-ManifestMergerOrder | `D(s∈{alphabetical})=ALPHABETICAL`; `E("dependency")=DEPENDENCY`; `C(ALPHABETICAL)=x="ALPHABETICAL", `. |
| F-Enum-AppleConfigurationDistinguisher | `D(s∈{UNKNOWN})=UNKNOWN`; `E("applebin_ios")=APPLEBIN_IOS`; filesystem accessor is not renderer; `C(UNKNOWN)=x="UNKNOWN", `. |
| F-Enum-DynamicMode | `D(s∈{off,default})`; `E("fully")=FULLY`; `C(OFF)=x="OFF", `. |
| F-Enum-JavaClasspathMode | `D(s∈{bazel})=BAZEL`; `E("off")=OFF`; `C(BAZEL)=x="BAZEL", `. |
| F-Enum-JavaOneVersionLevel | `D(s∈{OFF})=OFF`; `E("warning")=WARNING`; `C(OFF)=x="OFF", `. |
| F-Enum-Cancel | `D(s∈{never})=NEVER`; `E("on_failed")=ON_FAILED`, aliases `E(true)=ON_PASSED`/`E(false)=NEVER`; `C(NEVER)=x="NEVER", `. |
| F-Enum-CompilationMode | `D(s∈{fastbuild,opt})`; `E("dbg")=DBG`; lowercase `C(FASTBUILD)=x="fastbuild", `. |
| F-Enum-StripMode | `D(s∈{sometimes})=SOMETIMES`; `E("always")=ALWAYS`; lowercase `C(SOMETIMES)=x="sometimes", `. |

This is disjoint from `287 + 8 + 5 + 41 = 341` and independent
repeat/old-name/expansion/implicit metadata. After acceptance attach only the
287 pure descriptors mechanically; raw parser matching, contextual/regex/Host/
repository conversion, normalization, checksum/wire, DICE, and user-approved
later configured-target dependency cycles remain deferred.

Independent terminal review accepted retry 7: its 17 family and 16 enum rows,
global structural retained-value identity/order rule, raw parser-object
deferral, unadmitted-list boundary, Runs exception, Dotted full-input identity,
and exact Fission `yes`/`no`/comma routes close the pure byte contract. No
source, probe, descriptor, Rust, or runtime change occurred.

Run next only `WP-6-m2-pure-native-descriptor-family-attachment-ledger`: a
docs-only, mechanical 287-row attachment map from the committed 341 registry
and accepted cohort/family ledgers. It must retain `287 + 8 + 5 + 41`, keep
Runs-default outside the 287, and stop rather than invent a route. After its
acceptance, advance only to bounded pure-native kernel implementation planning.

### Pure native descriptor-family attachment ledger (2026-08-05)

This is a mechanical projection of the committed 341-row registry through the
accepted cohort and retry-7 family ledgers. Class marker `Axx` is the accepted
17-class registry order; the two-digit row key is that class's existing local
ordinal, including gaps occupied by excluded cohorts. Together the heading,
ordinal, and canonical name identify `FQCN#name` without a new global ordinal.

Route `S:N/E` is a scalar special-null default plus explicit occurrence;
`S:D/E` is a converted annotation default plus explicit occurrence; and
`R:N/A` is a repeatable special-null empty default plus the family-specific
ordered accumulation. The accepted family row supplies all grammar, retained
value, equality/order, rendering, and exact cache bytes.

#### A01 `com.google.devtools.build.lib.analysis.PlatformOptions`
01|`extra_execution_platforms`|`F-AllowCommaList`|`S:D/E`
02|`extra_toolchains`|`F-AllowCommaList`|`R:N/A`
04|`incompatible_use_toolchain_resolution_for_java_rules`|`F-Bool`|`S:D/E`
#### A03 `com.google.devtools.build.lib.analysis.config.CoreOptions`
01|`action_env`|`F-Env`|`R:N/A`
02|`affected by starlark transition`|`F-EmptyList`|`S:D/E`
03|`allow_analysis_failures`|`F-Bool`|`S:D/E`
04|`allow_unresolved_symlinks`|`F-Bool`|`S:D/E`
05|`allowed_cpu_values`|`F-StringSet`|`S:D/E`
06|`analysis_testing_deps_limit`|`F-Int`|`S:D/E`
08|`build_runfile_links`|`F-Bool`|`S:D/E`
09|`build_runfile_manifests`|`F-Bool`|`S:D/E`
10|`check_licenses`|`F-Bool`|`S:D/E`
11|`check_visibility`|`F-Bool`|`S:D/E`
12|`collect_code_coverage`|`F-Bool`|`S:D/E`
13|`compilation_mode`|`F-Enum-CompilationMode`|`S:D/E`
15|`define`|`F-Entry`|`R:N/A`
16|`enable_runfiles`|`F-Tri`|`S:D/E`
17|`enforce_constraints`|`F-Bool`|`S:D/E`
18|`evaluating for analysis test`|`F-Bool`|`S:D/E`
19|`exec_aspects`|`F-AllowCommaList`|`R:N/A`
21|`experimental_allow_map_directory`|`F-Bool`|`S:D/E`
22|`experimental_collect_code_coverage_for_generated_files`|`F-Bool`|`S:D/E`
23|`experimental_debug_selects_always_succeed`|`F-Bool`|`S:D/E`
24|`experimental_enforce_transitive_visibility`|`F-Bool`|`S:D/E`
25|`experimental_exclude_defines_from_exec_config`|`F-Bool`|`S:D/E`
26|`experimental_exec_config`|`F-Text`|`S:D/E`
27|`experimental_exec_configuration_distinguisher`|`F-Enum-ExecConfigurationDistinguisher`|`S:D/E`
28|`experimental_extended_sanity_checks`|`F-Bool`|`S:D/E`
29|`experimental_output_directory_naming_scheme`|`F-Enum-OutputDirectoryNaming`|`S:D/E`
30|`experimental_output_paths`|`F-Enum-OutputPaths`|`S:D/E`
32|`experimental_platform_in_output_dir`|`F-Tri`|`S:D/E`
34|`experimental_remotable_source_manifests`|`F-Bool`|`S:D/E`
35|`experimental_strict_fileset_output`|`F-Bool`|`S:D/E`
36|`experimental_throttle_action_cache_check`|`F-Bool`|`S:D/E`
37|`experimental_use_platforms_in_output_dir_legacy_heuristic`|`F-Bool`|`S:D/E`
38|`experimental_writable_outputs`|`F-Bool`|`S:D/E`
39|`features`|`F-Text`|`R:N/A`
41|`host_action_env`|`F-Env`|`R:N/A`
42|`host_compilation_mode`|`F-Enum-CompilationMode`|`S:D/E`
44|`host_features`|`F-Text`|`R:N/A`
45|`include_config_fragments_provider`|`F-Enum-IncludeConfigFragments`|`S:D/E`
46|`incompatible_always_include_files_in_data`|`F-Bool`|`S:D/E`
47|`incompatible_auto_exec_groups`|`F-Bool`|`S:D/E`
48|`incompatible_bazel_test_exec_run_under`|`F-Bool`|`S:D/E`
49|`incompatible_bep_cpu_from_platform`|`F-Bool`|`S:D/E`
50|`incompatible_check_testonly_for_output_files`|`F-Bool`|`S:D/E`
51|`incompatible_compact_repo_mapping_manifest`|`F-Bool`|`S:D/E`
52|`incompatible_disable_select_on`|`F-StringSet`|`S:D/E`
53|`incompatible_exclude_starlark_flags_from_exec_config`|`F-Bool`|`S:D/E`
54|`incompatible_filegroup_runfiles_for_data`|`F-Bool`|`S:D/E`
56|`incompatible_merge_genfiles_directory`|`F-Bool`|`S:D/E`
57|`incompatible_modify_execution_info_additive`|`F-Bool`|`S:D/E`
58|`incompatible_target_cpu_from_platform`|`F-Bool`|`S:D/E`
59|`instrument_test_targets`|`F-Bool`|`S:D/E`
61|`is exec configuration`|`F-Bool`|`S:D/E`
62|`min_param_file_size`|`F-Int`|`S:D/E`
64|`platform_suffix`|`F-Text`|`S:N/E`
66|`scl_config`|`F-Text`|`S:N/E`
67|`stamp`|`F-Bool`|`S:D/E`
68|`strict_filesets`|`F-Bool`|`S:D/E`
70|`use_target_platform_for_tests`|`F-Bool`|`S:D/E`
71|`verbose_visibility_errors`|`F-Bool`|`S:D/E`
#### A05 `com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions`
01|`allow_local_tests`|`F-Bool`|`S:D/E`
02|`cache_test_results`|`F-Tri`|`S:D/E`
05|`experimental_cancel_concurrent_tests`|`F-Enum-Cancel`|`S:D/E`
06|`experimental_fetch_all_coverage_outputs`|`F-Bool`|`S:D/E`
07|`experimental_retain_test_configuration_across_testonly`|`F-Bool`|`S:D/E`
08|`experimental_split_coverage_postprocessing`|`F-Bool`|`S:D/E`
09|`incompatible_check_sharding_support`|`F-Bool`|`S:D/E`
10|`incompatible_exclusive_test_sandboxed`|`F-Bool`|`S:D/E`
12|`runs_per_test_detects_flakes`|`F-Bool`|`S:D/E`
13|`test_arg`|`F-Text`|`R:N/A`
14|`test_env`|`F-Env`|`R:N/A`
15|`test_filter`|`F-Text`|`S:N/E`
16|`test_result_expiration`|`F-Int`|`S:D/E`
17|`test_runner_fail_fast`|`F-Bool`|`S:D/E`
18|`test_sharding_strategy`|`F-Shard`|`S:D/E`
19|`test_timeout`|`F-Timeout`|`S:D/E`
20|`trim_test_configuration`|`F-Bool`|`S:D/E`
21|`zip_undeclared_test_outputs`|`F-Bool`|`S:D/E`
#### A06 `com.google.devtools.build.lib.bazel.rules.BazelRuleClassProvider.StrictActionEnvOptions`
01|`incompatible_strict_action_env`|`F-Bool`|`S:D/E`
#### A07 `com.google.devtools.build.lib.bazel.rules.python.BazelPythonConfiguration.Options`
01|`experimental_python_import_all_repositories`|`F-Bool`|`S:D/E`
02|`incompatible_remove_ctx_bazel_py_fragment`|`F-Bool`|`S:D/E`
03|`python_path`|`F-Text`|`S:N/E`
#### A08 `com.google.devtools.build.lib.rules.android.AndroidConfiguration.Options`
01|`Android configuration distinguisher`|`F-Enum-AndroidConfigurationDistinguisher`|`S:D/E`
02|`android_compiler`|`F-Text`|`S:N/E`
03|`android_databinding_use_androidx`|`F-Bool`|`S:D/E`
04|`android_databinding_use_v3_4_args`|`F-Bool`|`S:D/E`
05|`android_dynamic_mode`|`F-Enum-DynamicMode`|`S:D/E`
06|`android_fixed_resource_neverlinking`|`F-Bool`|`S:D/E`
07|`android_manifest_merger`|`F-Enum-AndroidManifestMerger`|`S:D/E`
08|`android_manifest_merger_order`|`F-Enum-ManifestMergerOrder`|`S:D/E`
09|`android_migration_tag_check`|`F-Bool`|`S:D/E`
11|`android_resource_shrinking`|`F-Bool`|`S:D/E`
12|`apk_signing_method`|`F-Enum-ApkSigningMethod`|`S:D/E`
13|`break_build_on_parallel_dex2oat_failure`|`F-Bool`|`S:D/E`
14|`desugar_for_android`|`F-Bool`|`S:D/E`
15|`desugar_java8_libs`|`F-Bool`|`S:D/E`
16|`dexopts_supported_in_dexmerger`|`F-AllowCommaList`|`S:D/E`
17|`dexopts_supported_in_dexsharder`|`F-AllowCommaList`|`S:D/E`
18|`dexopts_supported_in_incremental_dexing`|`F-AllowCommaList`|`S:D/E`
19|`experimental_allow_android_library_deps_without_srcs`|`F-Bool`|`S:D/E`
20|`experimental_always_filter_duplicate_classes_from_android_test`|`F-Bool`|`S:D/E`
21|`experimental_android_assume_minsdkversion`|`F-Bool`|`S:D/E`
22|`experimental_android_compress_java_resources`|`F-Bool`|`S:D/E`
23|`experimental_android_databinding_v2`|`F-Bool`|`S:D/E`
24|`experimental_android_library_exports_manifest_default`|`F-Bool`|`S:D/E`
25|`experimental_android_resource_cycle_shrinking`|`F-Bool`|`S:D/E`
26|`experimental_android_resource_name_obfuscation`|`F-Bool`|`S:D/E`
27|`experimental_android_resource_path_shortening`|`F-Bool`|`S:D/E`
28|`experimental_android_resource_shrinking`|`F-Bool`|`S:D/E`
29|`experimental_android_rewrite_dexes_with_rex`|`F-Bool`|`S:D/E`
30|`experimental_android_use_parallel_dex2oat`|`F-Bool`|`S:D/E`
31|`experimental_check_desugar_deps`|`F-Bool`|`S:D/E`
32|`experimental_disable_instrumentation_manifest_merge`|`F-Bool`|`S:D/E`
33|`experimental_filter_library_jar_with_program_jar`|`F-Bool`|`S:D/E`
34|`experimental_filter_r_jars_from_android_test`|`F-Bool`|`S:D/E`
35|`experimental_get_android_java_resources_from_optimized_jar`|`F-Bool`|`S:D/E`
36|`experimental_incremental_dexing_after_proguard`|`F-Int`|`S:D/E`
37|`experimental_incremental_dexing_after_proguard_by_default`|`F-Bool`|`S:D/E`
38|`experimental_omit_resources_info_provider_from_android_binary`|`F-Bool`|`S:D/E`
39|`experimental_one_version_enforcement_use_transitive_jars_for_binary_under_test`|`F-Bool`|`S:D/E`
40|`experimental_persistent_aar_extractor`|`F-Bool`|`S:D/E`
41|`experimental_remove_r_classes_from_instrumentation_test_jar`|`F-Bool`|`S:D/E`
42|`experimental_use_dex_splitter_for_incremental_dexing`|`F-Bool`|`S:D/E`
43|`experimental_use_rtxt_from_merged_resources`|`F-Bool`|`S:D/E`
44|`fat_apk_hwasan`|`F-Bool`|`S:D/E`
45|`incompatible_disable_native_android_rules`|`F-Bool`|`S:D/E`
46|`incompatible_remove_ctx_android_fragment`|`F-Bool`|`S:D/E`
47|`incremental_dexing`|`F-Bool`|`S:D/E`
48|`internal_persistent_android_dex_desugar`|`F-Bool`|`S:D/E`
49|`internal_persistent_busybox_tools`|`F-Bool`|`S:D/E`
50|`internal_persistent_multiplex_android_dex_desugar`|`F-Bool`|`S:D/E`
51|`internal_persistent_multiplex_busybox_tools`|`F-Bool`|`S:D/E`
53|`non_incremental_per_target_dexopts`|`F-AllowCommaList`|`S:D/E`
55|`output_library_merged_assets`|`F-Bool`|`S:D/E`
56|`persistent_android_dex_desugar`|`F-Void`|`S:N/E`
57|`persistent_android_resource_processor`|`F-Void`|`S:N/E`
58|`persistent_multiplex_android_dex_desugar`|`F-Void`|`S:N/E`
59|`persistent_multiplex_android_resource_processor`|`F-Void`|`S:N/E`
60|`persistent_multiplex_android_tools`|`F-Void`|`S:N/E`
#### A09 `com.google.devtools.build.lib.rules.android.BazelAndroidConfiguration.Options`
01|`merge_android_manifest_permissions`|`F-Bool`|`S:D/E`
#### A10 `com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions`
01|`apple configuration distinguisher`|`F-Enum-AppleConfigurationDistinguisher`|`S:D/E`
02|`apple_platform_type`|`F-Platform`|`S:D/E`
04|`apple_split_cpu`|`F-Text`|`S:D/E`
05|`catalyst_cpus`|`F-AllowCommaList`|`R:N/A`
06|`experimental_include_xcode_execution_requirements`|`F-Bool`|`S:D/E`
07|`experimental_objc_provider_from_linked`|`F-Bool`|`S:D/E`
08|`experimental_prefer_mutual_xcode`|`F-Bool`|`S:D/E`
09|`host_macos_minimum_os`|`F-Dotted`|`S:N/E`
10|`incompatible_enable_apple_toolchain_resolution`|`F-Bool`|`S:D/E`
11|`ios_minimum_os`|`F-Dotted`|`S:N/E`
12|`ios_multi_cpus`|`F-AllowCommaList`|`R:N/A`
13|`ios_sdk_version`|`F-Dotted`|`S:N/E`
14|`macos_cpus`|`F-AllowCommaList`|`R:N/A`
15|`macos_minimum_os`|`F-Dotted`|`S:N/E`
16|`macos_sdk_version`|`F-Dotted`|`S:N/E`
17|`tvos_cpus`|`F-AllowCommaList`|`R:N/A`
18|`tvos_minimum_os`|`F-Dotted`|`S:N/E`
19|`tvos_sdk_version`|`F-Dotted`|`S:N/E`
20|`use_platforms_in_apple_crosstool_transition`|`F-Bool`|`S:D/E`
21|`visionos_cpus`|`F-AllowCommaList`|`R:N/A`
22|`watchos_cpus`|`F-AllowCommaList`|`R:N/A`
23|`watchos_minimum_os`|`F-Dotted`|`S:N/E`
24|`watchos_sdk_version`|`F-Dotted`|`S:N/E`
25|`xcode_version`|`F-Text`|`S:N/E`
#### A11 `com.google.devtools.build.lib.rules.config.ConfigFeatureFlagOptions`
01|`all feature flag values are present (internal)`|`F-Bool`|`S:D/E`
02|`enforce_transitive_configs_for_config_feature_flag`|`F-Bool`|`S:D/E`
#### A12 `com.google.devtools.build.lib.rules.cpp.CppOptions`
01|`apple_generate_dsym`|`F-Bool`|`S:D/E`
02|`build_test_dwp`|`F-Bool`|`S:D/E`
03|`cc_dotd_files`|`F-Bool`|`S:D/E`
04|`cc_include_scanning`|`F-Bool`|`S:D/E`
05|`cc_output_directory_tag`|`F-Text`|`S:D/E`
06|`compiler`|`F-Text`|`S:N/E`
07|`conlyopt`|`F-Text`|`R:N/A`
08|`copt`|`F-Text`|`R:N/A`
10|`cs_fdo_absolute_path`|`F-Text`|`S:N/E`
11|`cs_fdo_instrument`|`F-Text`|`S:N/E`
14|`cxxopt`|`F-Text`|`R:N/A`
15|`dynamic_mode`|`F-Enum-DynamicMode`|`S:D/E`
16|`enable_propeller_optimize_absolute_paths`|`F-Bool`|`S:D/E`
17|`enable_remaining_fdo_absolute_paths`|`F-Bool`|`S:D/E`
18|`experimental_cc_implementation_deps`|`F-Bool`|`S:D/E`
19|`experimental_cpp_compile_resource_estimation`|`F-Bool`|`S:D/E`
20|`experimental_cpp_modules`|`F-Bool`|`S:D/E`
21|`experimental_generate_llvm_lcov`|`F-Bool`|`S:D/E`
22|`experimental_inmemory_dotd_files`|`F-Bool`|`S:D/E`
23|`experimental_link_static_libraries_once`|`F-Bool`|`S:D/E`
24|`experimental_omitfp`|`F-Bool`|`S:D/E`
25|`experimental_save_feature_state`|`F-Bool`|`S:D/E`
26|`experimental_unsupported_and_brittle_include_scanning`|`F-Bool`|`S:D/E`
27|`experimental_use_cpp_compile_action_args_params_file`|`F-Bool`|`S:D/E`
28|`experimental_use_llvm_covmap`|`F-Bool`|`S:D/E`
29|`fdo_instrument`|`F-Text`|`S:N/E`
30|`fdo_optimize`|`F-Text`|`S:N/E`
33|`fission`|`F-Fission`|`S:D/E`
34|`force_pic`|`F-Bool`|`S:D/E`
36|`host_compiler`|`F-Text`|`S:N/E`
37|`host_conlyopt`|`F-Text`|`R:N/A`
38|`host_copt`|`F-Text`|`R:N/A`
39|`host_cxxopt`|`F-Text`|`R:N/A`
41|`host_linkopt`|`F-Text`|`R:N/A`
43|`incompatible_disable_legacy_cc_provider`|`F-Bool`|`S:D/E`
44|`incompatible_disable_nocopts`|`F-Bool`|`S:D/E`
45|`incompatible_dont_enable_host_nonhost_crosstool_features`|`F-Bool`|`S:D/E`
46|`incompatible_enable_cc_toolchain_resolution`|`F-Bool`|`S:D/E`
47|`incompatible_make_thinlto_command_lines_standalone`|`F-Bool`|`S:D/E`
48|`incompatible_remove_legacy_whole_archive`|`F-Bool`|`S:D/E`
49|`incompatible_require_ctx_in_configure_features`|`F-Bool`|`S:D/E`
50|`incompatible_use_cpp_compile_header_mnemonic`|`F-Bool`|`S:D/E`
51|`incompatible_use_specific_tool_files`|`F-Bool`|`S:D/E`
52|`incompatible_validate_top_level_header_inclusions`|`F-Bool`|`S:D/E`
53|`interface_shared_objects`|`F-Bool`|`S:D/E`
54|`legacy_whole_archive`|`F-Bool`|`S:D/E`
55|`linkopt`|`F-Text`|`R:N/A`
56|`ltobackendopt`|`F-Text`|`R:N/A`
57|`ltoindexopt`|`F-Text`|`R:N/A`
59|`minimum_os_version`|`F-Text`|`S:N/E`
60|`objc_enable_binary_stripping`|`F-Bool`|`S:D/E`
61|`objc_generate_linkmap`|`F-Bool`|`S:D/E`
62|`objc_use_dotd_pruning`|`F-Bool`|`S:D/E`
63|`objccopt`|`F-Text`|`R:N/A`
66|`process_headers_in_dependencies`|`F-Bool`|`S:D/E`
68|`propeller_optimize_absolute_cc_profile`|`F-Text`|`S:N/E`
69|`propeller_optimize_absolute_ld_profile`|`F-Text`|`S:N/E`
70|`proto_profile`|`F-Bool`|`S:D/E`
72|`save_temps`|`F-Bool`|`S:D/E`
73|`share_native_deps`|`F-Bool`|`S:D/E`
74|`start_end_lib`|`F-Bool`|`S:D/E`
75|`strict_system_includes`|`F-Bool`|`S:D/E`
76|`strip`|`F-Enum-StripMode`|`S:D/E`
77|`stripopt`|`F-Text`|`R:N/A`
#### A13 `com.google.devtools.build.lib.rules.java.JavaOptions`
01|`bytecode_optimization_pass_actions`|`F-Int`|`S:D/E`
03|`enforce_proguard_file_extension`|`F-Bool`|`S:D/E`
04|`experimental_add_test_support_to_compile_time_deps`|`F-Bool`|`S:D/E`
05|`experimental_enable_jspecify`|`F-Bool`|`S:D/E`
06|`experimental_fix_deps_tool`|`F-Text`|`S:D/E`
07|`experimental_inmemory_jdeps_files`|`F-Bool`|`S:D/E`
08|`experimental_java_classpath`|`F-Enum-JavaClasspathMode`|`S:D/E`
09|`experimental_java_test_auto_create_deploy_jar`|`F-Bool`|`S:D/E`
11|`experimental_local_java_optimizations`|`F-Bool`|`S:D/E`
12|`experimental_one_version_enforcement`|`F-Enum-JavaOneVersionLevel`|`S:D/E`
13|`experimental_run_android_lint_on_java_rules`|`F-Bool`|`S:D/E`
14|`experimental_strict_java_deps`|`F-Enum-StrictDeps`|`S:D/E`
15|`experimental_turbine_annotation_processing`|`F-Bool`|`S:D/E`
16|`explicit_java_test_deps`|`F-Bool`|`S:D/E`
18|`host_javacopt`|`F-Text`|`R:N/A`
19|`host_jvmopt`|`F-Text`|`R:N/A`
20|`incompatible_disallow_java_import_exports`|`F-Bool`|`S:D/E`
21|`incompatible_multi_release_deploy_jars`|`F-Bool`|`S:D/E`
22|`java_debug`|`F-Void`|`S:N/E`
23|`java_deps`|`F-Bool`|`S:D/E`
24|`java_header_compilation`|`F-Bool`|`S:D/E`
25|`java_language_version`|`F-Text`|`S:D/E`
27|`java_runtime_version`|`F-Text`|`S:D/E`
28|`javacopt`|`F-Text`|`R:N/A`
29|`jvmopt`|`F-Text`|`R:N/A`
30|`one_version_enforcement_on_java_tests`|`F-Bool`|`S:D/E`
33|`split_bytecode_optimization_pass`|`F-Bool`|`S:D/E`
34|`tool_java_language_version`|`F-Text`|`S:D/E`
35|`tool_java_runtime_version`|`F-Text`|`S:D/E`
36|`use_ijars`|`F-Bool`|`S:D/E`
#### A14 `com.google.devtools.build.lib.rules.objc.J2ObjcCommandLineOptions`
02|`j2objc_translation_flags`|`F-AllowCommaList`|`R:N/A`
#### A15 `com.google.devtools.build.lib.rules.objc.ObjcCommandLineOptions`
01|`device_debug_entitlements`|`F-Bool`|`S:D/E`
02|`experimental_objc_fastbuild_options`|`F-AllowCommaList`|`S:D/E`
03|`incompatible_avoid_hardcoded_objc_compilation_flags`|`F-Bool`|`S:D/E`
04|`incompatible_builtin_objc_strip_action`|`F-Bool`|`S:D/E`
05|`incompatible_disable_native_apple_binary_rule`|`F-Bool`|`S:D/E`
06|`incompatible_disallow_sdk_frameworks_attributes`|`F-Bool`|`S:D/E`
07|`incompatible_objc_alwayslink_by_default`|`F-Bool`|`S:D/E`
08|`incompatible_strip_executable_safely`|`F-Bool`|`S:D/E`
09|`ios_memleaks`|`F-Bool`|`S:D/E`
10|`ios_signing_cert_name`|`F-Text`|`S:N/E`
11|`ios_simulator_device`|`F-Text`|`S:N/E`
12|`ios_simulator_version`|`F-Dotted`|`S:N/E`
13|`objc_debug_with_GLIBCXX`|`F-Bool`|`S:D/E`
#### A16 `com.google.devtools.build.lib.rules.proto.ProtoConfiguration.Options`
01|`cc_proto_library_header_suffixes`|`F-StringSet`|`S:D/E`
02|`cc_proto_library_source_suffixes`|`F-StringSet`|`S:D/E`
03|`experimental_proto_descriptor_sets_include_source_info`|`F-Bool`|`S:D/E`
09|`protocopt`|`F-Text`|`R:N/A`
10|`strict_proto_deps`|`F-Enum-StrictDeps`|`S:D/E`
11|`strict_public_imports`|`F-Enum-StrictDeps`|`S:D/E`
#### A17 `com.google.devtools.build.lib.rules.python.PythonOptions`
01|`build_python_zip`|`F-Tri`|`S:D/E`
02|`experimental_py_binaries_include_label`|`F-Bool`|`S:D/E`
03|`incompatible_default_to_explicit_init_py`|`F-Bool`|`S:D/E`
04|`incompatible_python_disallow_native_rules`|`F-Bool`|`S:D/E`
05|`incompatible_remove_ctx_py_fragment`|`F-Bool`|`S:D/E`

**Default-materializer exception outside the 287:** registry
`A05.11#runs_per_test` remains in the Java-regex cohort; only its accepted
`F-Runs-default D("1")` seed is admitted, with deferred `U("+2")`. It is not
a pure descriptor attachment.

The table contains exactly 287 unique registry keys: 227 built-in attachments
plus 60 explicit pure-converter attachments. The 54 omitted registry rows remain
exactly eight Java-regex, five Host, and 41 repository/loading descriptors, so
`287 + 8 + 5 + 41 = 341`. The independent 45 repeatable, 13 old-name, six
expansion, and two implicit-requirement metadata rows are unchanged and are not
value families. No source, converter, default, or route was inferred beyond the
accepted ledgers.

Independent terminal review accepted all 287 registry-order attachments: 227
built-in plus 60 explicit pure rows, with route totals `33 R:N/A + 221 S:D/E +
33 S:N/E`, no duplicate key, and the exact excluded `8 + 5 + 41`. Runs remains
default-materializer-only and the orthogonal `45/13/6/2` metadata is unchanged.
No source, registry, runtime, or Rust change occurred.

Run next only
`WP-6-m2-pure-native-value-default-and-rendering-kernel-retry-2`: implement the
now-closed 287-family/default/cache contract in the bounded seven-file
`slug_configuration_v2` owner. Preserve all contextual, normalization,
checksum/wire, DICE, and configured-target-cycle deferrals.

### Pure native kernel retry 2 REPLAN (2026-08-05)

`WP-6-m2-pure-native-value-default-and-rendering-kernel-retry-2` stopped after
its one permitted correction. Two Terra writers built a private seven-file
draft that compiled and passed seven focused tests, but the correction review
found multiple material mismatches with already accepted contracts: explicit
Void `null` used a text sentinel instead of absence; timeout did not implement
the frozen `.limit(6)` split/rejection cases; Dotted accepted arbitrary
digit-prefixed text without the component/descriptive grammar, signed-i32
bounds, or early stop; and ordered maps used compact-string pairs instead of
the frozen `Arc<[(NativeValue, NativeValue)]>` representation. Its tests also
did not discriminate those failures. The entire unaccepted crate diff was
discarded with `apply_patch`; HEAD returned cleanly to `d6570a74`.

Run next only
`WP-6-m2-pure-native-value-default-and-rendering-kernel-retry-3` with the same
seven crate files and 1,550 production/1,250 test/2,800 total caps. Before
implementing breadth, freeze typed focused tests for Void absence, exact
timeout split cases, full Dotted bounds/underscore/early-stop/original-text
identity, exact NativeValue ordered-map pairs, Bool/Tri versus reference/repeat
annotation-null semantics, and a positive typed private Runs seed. Then reuse
the accepted retry-7 and 287-row ledgers for the remaining routes. Add no new
source/oracle evidence or public API. Any new material correction is `REPLAN`.
Preserve all Java-regex, Host, repository/loading, command/repeat,
normalization, checksum/wire, DICE, downstream activation, and user-approved
configured-target-cycle deferrals.

### Pure native kernel retry 3 REPLAN (2026-08-05)

`WP-6-m2-pure-native-value-default-and-rendering-kernel-retry-3` stopped at its
zero-new-correction rule. One Terra writer produced a seven-file draft that
passed crate test/check, formatting, and GNU-Windows check, but root and an
independent Terra review found direct contradictions with the accepted retry-7
ledger: the private kernel was publicly re-exported; empty comma/set input
became a singleton empty string instead of `Ø`; Env rejected `=N`, mapped `N=`
to Unset, and rendered raw strings instead of exact Set/Inherit/Unset record
text; Explicit/Disabled shard cache text was lowercase; the Runs seed permitted
public/nonpositive construction; and Fission comma input retained case and
duplicates instead of ordered-distinct typed compilation modes. The passing
tests omitted those discriminators. The entire unaccepted crate diff was
discarded with `apply_patch`; HEAD returned cleanly to `fdb5fdb5`.

Run next only
`WP-6-m2-pure-native-value-default-and-rendering-kernel-retry-4` with the same
seven files and 1,550 production/1,250 test/2,800 total caps, but enforce a
serial two-phase gate. Phase 1 changes tests only: mechanically cover every 17
family and 16 enum ledger row, all 287 attachments and exact 8/5/41 exclusions,
every retry-2 correction, and the retry-3 privacy/empty/Env/shard/Runs/Fission
misses; independent source review must accept this matrix before Phase 2 writes
production. Tests may reach child-private items from the parent test module;
no kernel item is publicly re-exported. Phase 2 implements exactly that matrix.
Any production-before-review or new material correction is `REPLAN`. Add no
source/oracle evidence, registry/lockfile change, public API, command merge,
contextual converter, normalization, checksum/wire, DICE, downstream
activation, or user-deferred configured-target-cycle behavior.

### Pure native kernel retry 4 REPLAN (2026-08-05)

`WP-6-m2-pure-native-value-default-and-rendering-kernel-retry-4` stopped in its
test-only Phase 1; no production file or dependency changed. Two serial Terra
writers and root produced a compact literal 287/8/5/41 registry partition and
active count reconciliation, but the independent pre-production review found
the behavioral matrix unfit to authorize implementation. It contradicted the
accepted Runs contract by marking `U("+2")` rejected instead of
accepted/deferred, checked family/enum strings tautologically rather than
through discriminators, did not assert attachment order or family/route values,
and hid missing Bool/Tri/null, Dotted, timeout, UTF-16, Integer, enum, and cache
cases behind a disabled block of undefined helper shims. The complete
unaccepted `tests.rs` diff was discarded mechanically; HEAD returned cleanly
to `28e9ddc9`.

Run next only
`WP-6-m2-pure-native-value-default-and-rendering-kernel-retry-5` with the same
seven-file and 1,550 production/1,250 test/2,800 total caps. Preserve the
two-phase gate, but route Phase 1 to root mechanical transcription from the
accepted retry-7 and attachment ledgers; Terra is review-only until acceptance.
The test matrix must enumerate the exact registry partition/order/family/route,
all finite members/defaults/aliases/renderers, and direct planned-private API
assertions for every discriminator without checklist text, `todo!`, or
undefined helper semantics. Runs `U("+2")` is accepted/deferred and must not
be exposed as a general occurrence. Only after independent acceptance may one
Terra writer implement the private kernel. Any new material correction is
`REPLAN`; all contextual, command, normalization, checksum/wire, DICE,
downstream, and configured-cycle deferrals remain unchanged.

### Pure native kernel retry 5 REPLAN (2026-08-05)

`WP-6-m2-pure-native-value-default-and-rendering-kernel-retry-5` stopped after
its serial test-only Phase 1; no production file or dependency is retained.
Root transcribed an active direct 287/8/5/41 matrix within the test cap, and an
independent Terra review accepted it after correcting temporary-reference,
duration, and enum-default misses. The authorized Phase-2 writer then found a
new material contradiction: the matrix used empty-default
`PlatformOptions#extra_execution_platforms` while expecting the nonempty
`[-O0, -DDEBUG=1]` value owned by
`ObjcCommandLineOptions#experimental_objc_fastbuild_options`. Root discarded
the complete unaccepted 1,021-line `tests.rs` addition; HEAD returned cleanly
to `a64f0661`.

Run next only
`WP-6-m2-pure-native-value-default-and-rendering-kernel-retry-6` with the same
seven-file and 1,550 production/1,250 test/2,800 total caps. Preserve the
root-owned test-only Phase 1, Terra review-only gate, exact private API and all
retry-5 freezes. Before behavioral acceptance, add an active literal binding
row for every one of the 287 attachments: exact attachment ordinal, FQCN,
canonical name, registry field type/raw default/converter/repeat bit, accepted
family/route, and expected materialized-default outcome/cache. Each row must
resolve the descriptor by FQCN/name and assert its registry tuple before
materialization; no constructed descriptor or family-only expected value may
stand in for that binding. Review same-family collision rows explicitly,
including the empty `extra_execution_platforms` default versus the nonempty
ObjC fastbuild-options default. Keep Runs' default-only seed separate and the
8/5/41 exclusions assertion-only. Only after independent acceptance may one
Terra writer implement the private kernel. Any new material correction is
`REPLAN`; all contextual, command, normalization, checksum/wire, DICE,
downstream, and user-approved configured-target-cycle deferrals remain
unchanged.

### Pure native kernel retry 6 REPLAN (2026-08-05)

`WP-6-m2-pure-native-value-default-and-rendering-kernel-retry-6` completed its
serial gate but failed terminal production review. Root built a 1,155-line
test-only matrix that actively bound every 287 attachment's ordinal and exact
registry tuple to its family, route, materialized outcome, and cache bytes,
retained exact 8/5/41 exclusions, and covered the private behavior surface.
Independent Terra review accepted it after bounded test-only corrections. One
authorized Terra writer then supplied 677 production lines; all 13 tests,
crate check, formatting, GNU-Windows no-run, archive, scope, cap, and diff gates
passed.

Latest-diff review nevertheless found five new material contradictions. The
converter returned occurrence lists for all list-valued families instead of
distinguishing nonrepeat scalar lists from repeat expansion lists. Dotted
descriptive matching rejected uppercase despite its case-insensitive pattern.
Timeout special-cased one rejected string instead of implementing the accepted
`.limit(6)` split/arity/decimal-validation grammar and incorrectly defaulted
malformed entries. Total `u64` nanoseconds both admitted inputs beyond Java
signed-long parsing and rejected or overflowed valid large Java durations.
Fission case-folded exact `yes`/`no` specials, admitting `YES`/`No`. Root
discarded the entire unaccepted seven-file diff; HEAD returned cleanly to
`875b4006`.

Run next only
`WP-6-m2-pure-native-value-default-and-rendering-kernel-retry-7` with the same
seven-file and 1,550 production/1,250 test/2,800 total caps. Restore the
accepted retry-6 matrix under the same root-owned Phase-1/Terra-review gate and
add direct discriminators for all five failures before production: repeat-bit
occurrence shape; uppercase Dotted descriptive early stop; general `.limit(6)`
timeout splitting with malformed rejection and nonpositive-only fallback;
Java signed-long parsing plus seconds/nanos retained duration range; and exact
case-sensitive Fission specials. Only after independent acceptance may one
Terra writer implement the private kernel. Any new material correction is
`REPLAN`; all contextual, command, normalization, checksum/wire, DICE,
downstream, and user-approved configured-target-cycle deferrals remain
unchanged.

### Pure native kernel retry 7 accepted (2026-08-05)

Commit `e7067bfc` accepts the private context-free value/default/cache kernel.
Its literal matrix binds all 287 pure descriptors to exact registry tuples,
attachment ordinals, families, routes, materialized defaults, and cache bytes;
it separately proves the exact `8 + 5 + 41` exclusions and the default-only
Runs seed. Phase-1 source review corrected the Dotted discriminator before any
production was retained: a descriptive component is case-insensitive but may
terminate only after at least one numeric component, so `1.A`,
`1.A_internal`, and `1.A_internal.!` accept while bare `A`/`A_internal` do not.
The accepted implementation also distinguishes scalar list values from repeat
occurrence lists, implements the general TestTimeout `.limit(6)` path, retains
signed-long durations as seconds plus nanos, and recognizes only exact
lowercase Fission `yes`/`no` specials.

The algebra uses `CompactString`, `Arc<[NativeValue]>`, and
`Arc<[(NativeValue, NativeValue)]>`, derives `Allocative`, and limits `Dupe` to
the two pointer-cheap aggregate newtypes. It is child-private and contains no
runtime registry map, hash, interner, static cache, mutable global, command
accumulation, normalization, DICE, wire, or configured-target edge. Validation
passed 13 focused tests, crate check, formatting, GNU-Windows no-run, archive,
scope, cap, and diff checks at 734 production/1,234 test/1,968 total net lines.
Two independent Terra terminal reviews returned `ACCEPT`.

Run next only `WP-6-m2-host-and-repository-conversion-context-design`, a
docs-only design for the smallest immutable input boundary shared by the five
Host and 41 repository/package/loading descriptors. It must assign every
required fact to an existing layer, preserve conversion-before-normalization,
and stop rather than invent a DICE key, filesystem bypass, repository loader,
configured target, or cycle edge. Java-regex, command flattening, checksum/wire,
normalization, activation, and user-approved configured-target dependency
cycles remain deferred.

### Host and repository conversion-context design (2026-08-05)

**Decision: ACCEPT a two-context split with two serial prerequisites before
contextual conversion.** One 46-field bag would mix process/Host facts with
package-relative label identity and give `slug_configuration_v2` an implicit
IO/loading role. Instead conversion consumes supplied immutable values only:

```text
HostConversionContext {
  os, cpu, host_cpus, host_ram_mb,
  host_path_policy, user_home_unicode,
}

LabelConversionContext =
  FirstRoundCanonical
  | MainRepository { mapping }
  | Package { base_package, mapping }
```

After the Host-input prerequisite lands, command/request bootstrap will observe
the Host once and inject the new snapshot through core request assembly toward
configuration conversion; no such Host conversion snapshot exists today.
`slug_bzlmod_v2` remains the producer of repository mappings,
`slug_loading_v2` remains the package/loading owner, and `slug_identity_v2`
remains the parser/resolver owner. The context carries an `Arc`-shared mapping
and package identity; conversion does not load a package, evaluate a module,
read an environment variable, inspect a path, or compute a DICE key.

#### Descriptor-complete contextual routes

The five Host descriptors are closed and disjoint from label context:

| Descriptors | Converter/result | Immutable input |
| --- | --- | --- |
| `cpu`, `host_cpu` | `AutoCpuConverter`; explicit text is unchanged, empty maps the finite OS/CPU pair to Bazel's legacy token | `os`, `cpu` |
| `shell_executable` | `PathFragmentConverter`; special-null default is absent; explicit input starting `~/` replaces every `~` in the full string with `user.home` before lexical Host-policy normalization | `host_path_policy`, valid-Unicode `user_home` |
| `platform_mappings` | `PlatformMappingKey::{Default, ExplicitWorkspaceRelative}`; empty is default and absolute is rejected | same lexical path inputs; bytes/search/parse remain workspace/loading/core-owned |
| `default_test_resources` | repeat `(resource name, {SMALL, MEDIUM, LARGE, ENORMOUS -> ResourceAmount})`; one amount broadcasts, four bind in enum order; direct doubles are range-checked, while `HOST_CPUS`/`HOST_RAM` optionally apply source `-`/`*` Float arithmetic without a second bounds check | captured ceil `HOST_CPUS`, ceil `HOST_RAM` MiB |

The 41 repository/package/loading descriptors are complete in the following
table. `L` is the mapping-provenance-free resolved option-label value specified
below; every listed label routes through exactly one active
`LabelConversionContext` mode.

| Converter/result | Exact descriptors |
| --- | --- |
| `HostPlatformConverter -> L`; empty uses the symbolic Host default | `host_platform` |
| `LabelListConverter -> Arc<[L]>`, comma order, empty pieces omitted | `platforms`, `experimental_action_listener`, `incompatible_limit_platforms_in_output_dir_to`, `target_environment`, `apple_platforms`, `plugin` |
| `LabelOrderedSetConverter -> Arc<[L]>`, first occurrence wins | `android_platforms` |
| `LabelMapConverter -> Arc<[(CompactString, Option<L>)]>`, insertion order and duplicate-key rejection | `bytecode_optimizers` |
| `LabelToStringEntryConverter -> (L, CompactString)` | `experimental_override_platform_cpu_name` |
| ordinary `LabelConverter -> L` after special-null default handling | `coverage_output_generator`, `coverage_report_generator`, `coverage_support`, `legacy_main_dex_list_generator`, `xcode_version_config`, `crosstool_top`, `cs_fdo_profile`, `custom_malloc`, `fdo_prefetch_hints`, `memprof_profile`, `propeller_optimize`, `proto_profile_path`, `experimental_local_java_optimization_configuration`, `proguard_top`, `j2objc_dead_code_report`, `python_native_rules_allowlist` |
| `EmptyToNullLabelConverter -> Option<L>` | `optimizing_dexer`, `fdo_profile`, `xbinary_fdo`, `host_java_launcher`, `java_launcher` |
| core `LabelConverter -> L`; finite symbolic default | `proto_compiler`, `proto_toolchain_for_javalite` |
| core `EmptyToNullLabelConverter -> Option<L>`; finite symbolic default | `proto_toolchain_for_cc`, `proto_toolchain_for_j2objc`, `proto_toolchain_for_java` |
| `LibcTopLabelConverter -> Option<L>`; exact `default` is absent, other input must start `//` and target becomes `everything` | `grte_top`, `host_grte_top` |
| `RunUnder::{Label { original, suffix, label: L }, Command { original, suffix, command }}` after source-equivalent shell tokenization | `run_under` |
| `CustomFlagConverter -> CompactString`; nonlabel define is raw, label branch canonicalizes including `/...` | `experimental_propagate_custom_flag` |
| `FlagAliasConverter -> (CompactString, L)` after exact alias validation | `flag_alias` |

The repository counts are `1 + 6 + 1 + 1 + 1 + 16 + 5 + 2 + 3 + 2 + 1 + 1 + 1 = 41`;
with the Host `2 + 1 + 1 + 1 = 5`, the accepted
`287 + 8 + 5 + 41 = 341` partition is unchanged. `RunUnder` and `CustomFlag`
have genuine nonlabel branches; they are inventoried here but may not be
silently forced through label conversion.

#### Exact context and retained-value boundaries

`FirstRoundCanonical` preserves Bazel's deliberately mapping-free first parse;
it is not an empty mapping and must not be replaced by second-round output.
`MainRepository` implements second-round command parsing from the main
repository plus its mapping. `Package` implements Starlark/package-relative
forms with the supplied base package and mapping. The converter prepends the
accepted main-repository form only where Bazel's option converter does; it may
not reuse the stricter existing absolute-only `ApparentLabel::parse` for all
three modes.

The six symbolic defaults are a private finite enum/table, never caller text:
`DEFAULT_HOST_PLATFORM` plus the five Proto defaults for protoc, CC, J2ObjC,
Java, and Java Lite. Each expands to its pinned `@bazel_tools` source spelling
and then uses the active label context. Java constant names never enter a
runtime cache field or diagnostic as option input.

Retained `L` contains only canonical repository, package, and target identity.
The live `CanonicalLabel` cannot substitute: its derived equality/order/hash
and stable serialization include `mapping_id`; only `bazel_natural_cmp`
ignores provenance. `slug_identity_v2` must therefore own a distinct
mapping-free resolved option-label projection and the three-mode parser seam
before configuration stores any label. Loading's provenance-bearing label
remains unchanged.

Host paths are valid-Unicode lexical `PathFragment` values, not filesystem
paths and not `slug_workspace_v2::NormalizedAbsolutePath` (which cannot
represent the required relative values). Retain their normalized spelling in
`CompactString` with an explicit finite Host path policy. Reject a lone
surrogate or lossy host conversion. Resource amounts wrap Java double bits so
Rust provides lawful Java `Double`-shaped equality/order/rendering. Direct
numeric input accepts only the source range `[0, Double.MAX_VALUE]`; keyword
arithmetic may produce negative or infinite results because Bazel does not call
`checkAndLimit` after applying the source `-`/`*` Float operand. The four-key
result is an ordered fixed aggregate, never a hash map.

Use `Arc` for the Host snapshot, mapping, immutable label collections, and
ordered entry collections. Derive `Allocative` on retained values and `Dupe`
only for Arc-backed aggregate wrappers; do not deep-clone `RepositoryMapping`
or introduce a runtime descriptor map, interner, cache, global, or hash.

Conversion-before-normalization remains mandatory. P cannot truncate
`platforms` and C cannot deduplicate `flag_alias` until every contextual
occurrence has converted successfully. Command flattening, old-name handling,
boolean negation, repetition, expansion, and implicit requirements remain
`slug_commands_v2` work. No converter constructs a target or configuration,
so this design adds no dependency edge and leaves configured-target cycles at
the user-approved later boundary.

#### Bounded serial implementation sequence

1. `WP-6-m2-option-label-context-identity`: in `slug_identity_v2` only, add the
   mapping-free resolved option-label value and source-pinned three-mode parsing
   primitives. Prove package-relative/main-repository/first-round distinctions,
   mapping resolution, equality/order/hash/rendering without provenance, and no
   loading/configuration/target dependency.
2. A Host-input prerequisite defines the Arc-backed observation schema and
   lexical path policy, then separately connects existing request/bootstrap
   observation to core request assembly. It must pin OS/CPU legacy tokens,
   valid-Unicode home handling, CPU/RAM capture timing, and one-shot/daemon
   structural equality before any contextual converter consumes it.
3. Only then may bounded configuration packets add the 41 label/conditional
   routes and five Host routes, followed later by full-fragment P/C/T
   normalization. Java-regex, checksum/wire, DICE producer ownership,
   downstream activation, and configured-target cycles remain deferred.

Independent read-only source, live-substrate, and Buck2-utility audits accept
the descriptor counts and two-context split. The live audit found both serial
prerequisites genuinely absent: no Host CPU/RAM/home snapshot exists, current
absolute label parsing lacks `PackageContext`, and the only live resolved label
retains mapping provenance. Implementing converters first would invent
ownership or encode the wrong configuration identity.

Run next only `WP-6-m2-option-label-context-identity`: add the mapping-free
resolved option-label value and closed first-round/main-repository/package
parser seam inside `slug_identity_v2`. Preserve the existing provenance-bearing
`CanonicalLabel` unchanged and stop on any loading, materialization, DICE,
configuration, target, command-tokenization, or configured-cycle edge.

### Option-label context identity REPLAN (2026-08-05)

`WP-6-m2-option-label-context-identity` stopped during its pinned-source test
matrix before production. Bazel does not reject an apparent repository absent
from the supplied mapping. `RepositoryMapping#get` returns a non-visible
`RepositoryName` carrying the apparent name plus the current repository; label
construction succeeds, and repository use fails later with context-sensitive
identity/diagnostics. Slug's live `RepositoryMapping::resolve` instead falls
back to an ordinary visible `CanonicalRepoName`, while the planned
mapping-free `(repository, package, target)` option label had no representation
for the non-visible state. Constraining mappings to referenced repositories
would hide observable Bazel behavior and is not accepted.

Root discarded the test-only draft with `apply_patch`; no Rust, test,
dependency, API, or runtime change remains. No production was written. The
mapping-provenance distinction remains valid, but the resolved option-label
repository component must first distinguish visible canonical identity from
source-equivalent non-visible apparent/owner identity without changing the
existing loading `CanonicalLabel` or `RepositoryMapping::resolve` contract.

Run next only
`WP-6-m2-option-label-nonvisible-repository-identity-design`, docs-only. Pin the
non-visible repository construction, equality/order/hash/rendering/diagnostic
facts and audit live identity/mapping consumers; then select the smallest
additive option-label representation and bounded implementation packet or
`REPLAN`. Add no Rust, source probe, fixture, DICE, loading/materialization,
configuration, target, command, or configured-cycle behavior.

### Option-label non-visible repository identity design (2026-08-05)

**Decision: ACCEPT a three-field non-visible identity plus source-ordered
mapping candidates.** Bazel 9.2 `RepositoryMapping#get` returns a mapped
visible `RepositoryName` when an entry exists. On a miss it constructs
`RepositoryName.createUnvalidated(requested).toNonVisible(contextRepo,
SpellChecker.didYouMean(requested, entries.keySet()))`; it does not reject or
fall back to a visible canonical name. The resulting structural repository
identity is exactly bare requested apparent name, visible owner/context
repository, and the produced did-you-mean suffix. `RepositoryName` equality
and hash include all three fields.

The non-visible canonical, unambiguous, and null-mapping display forms all keep
the state explicit:

```text
@@[unknown repo '<requested>' requested from <owner><suffix>]//<package>:<target>
```

The owner is the mapping context: main for command/main-repository parsing and
the supplied base package repository for package parsing. Repository use fails
later, before fetching, with `No repository visible as '@<requested>' from
<owner-display>`; main renders as `main repository`, while a nonmain owner
renders as `repository '@@owner+'`. This packet retains the identity and exact
label rendering only. Repository use/materialization and that later diagnostic
remain outside the identity parser.

#### Equality, ordering, and suggestion closure

Bazel's natural `Label.compareTo` is deliberately weaker than repository
equality: `PackageIdentifier.compareTo` compares only the repository's bare
name, then package and target. A visible `@@missing` label and a non-visible
`@missing` label, or non-visible labels with different owners/suffixes, may
therefore compare equal while being unequal and having different hashes. Rust
`Ord` cannot use that relation. The option-label value must use lawful
structural `Eq`/`Ord`/`Hash` over visibility and all non-visible fields, and
expose a separate non-key `bazel_natural_cmp` over bare repository name,
package, and target using Java UTF-16 string order.

The suffix is also source-order-sensitive. `SpellChecker.suggest` lowercases
with Java semantics, starts at `min(5, (UTF-16 length + 1) / 2)`, computes
bounded UTF-16 Levenshtein distance, and replaces the current result only for a
strictly smaller distance. Equal-distance candidates keep the first
`ImmutableMap` key. Slug's `RepositoryMapping` currently stores only a
`BTreeMap`, although every live producer supplies entries serially through
`insert`; sorted-key traversal is not exact. The retry must retain final unique
keys in insertion order alongside the existing map. Replacement keeps the
key's first position. Custom `RepositoryMapping` equality must continue to use
only its existing ID and entry contents, not candidate order, matching the
current contract and map equality; `resolve` remains byte-for-byte equivalent.
Repository names are validated ASCII and Bazel's launcher forces the JVM root
locale, so the private port needs only source-equivalent ASCII case folding
while retaining the source threshold and first-wins rules. No second map or
new dependency is needed.

The accepted additive shape is conceptually:

```text
OptionRepository =
  Visible(CanonicalRepoName)
  | NonVisible {
      requested: ApparentRepoName,
      owner: CanonicalRepoName,
      did_you_mean_suffix: String,
    }

ResolvedOptionLabel {
  repository: OptionRepository,
  package: PackagePath,
  target: TargetName,
}
```

The repository variants and fields may remain crate-private behind the public
resolved option-label parser/value. The suffix is the exact third Bazel
identity field, produced only by the private source-equivalent mapping lookup;
it is not a formatted-label sentinel. Derive `Allocative` and structural
clone/equality/order/hash on the retained value. Reuse the existing owned
identity strings and mapping `BTreeMap`; add only the ordered key vector. Add no
`Dupe`, `Arc` inside each label, interner, cache, global, or runtime identity
map. Callers share the enclosing mapping/context with the already accepted
`Arc` boundary.

#### Live boundary and direct discriminators

Keep `CanonicalRepoName`, `CanonicalLabel`, `PackageIdentifier`,
`RepositoryMapping::resolve`, mapping IDs, and `StableSerialize` unchanged.
The live `resolve` consumers are loading/analysis identity paths and tests; its
visible-name fallback is intentionally not retrofitted. `CanonicalLabel`
continues to include `mapping_id` in derived equality/order/hash and stable
serialization. `ResolvedOptionLabel` is the separate configuration value and
gets no `StableSerialize` implementation while checksum/wire ownership is
deferred. It cannot embed `PackageIdentifier`, which cannot represent a
non-visible repository.

The retry must directly prove:

- mapped aliases from different mapping IDs resolving to the same canonical
  repository produce the same resolved option label; different mapped results
  differ;
- unmapped `@missing//p:t` from main and from `@@owner+//base` retain different
  owners and are structurally unequal, while their Bazel natural comparison is
  equal;
- unmapped apparent `@missing` differs from direct visible `@@missing`, even
  though their Bazel natural comparison is equal;
- first-round canonical parsing treats one-`@` spelling as a visible literal,
  while second-round main/package parsing performs apparent lookup;
- explicit apparent-root `@//p:t` is not collapsed with unqualified `//p:t`;
- candidate order `baa, aab` for missing `aaa` suggests `baa`, while reversed
  insertion suggests `aab`; the mappings retain current content equality but
  the non-visible identities, hashes, and renderings differ; and
- visible root canonical rendering remains `//p:t`, unambiguous rendering is
  `@@//p:t`, and the non-visible rendering above is exact with and without its
  ` (did you mean '<candidate>'?)` suffix.

Run next only `WP-6-m2-option-label-context-identity-retry`. It is confined to
`slug_identity_v2`: retain mapping candidate order without changing existing
mapping semantics, port the private spellchecker path, and add the distinct
resolved option-label value plus first-round/main/package parser. Stop on any
need to change an existing visible identity, load/materialize a repository or
package, add a dependency/lockfile, serialize configuration wire, or introduce
a loading/configuration/target/DICE/configured-cycle edge. Configured-target
dependency cycles remain explicitly deferred by user approval.

### Option-label context identity retry implementation (2026-08-05)

**Decision: ACCEPT.** Commit `b035dfbb` adds the public
`OptionLabelContext`/`ResolvedOptionLabel` seam inside `slug_identity_v2` while
keeping its visible/non-visible repository variant private. Mapping IDs do not
enter the new label identity. Existing `CanonicalLabel`, stable serialization,
and `RepositoryMapping::resolve` behavior are unchanged.

`RepositoryMapping` now retains final unique keys in insertion order beside its
existing `BTreeMap`; replacement preserves first position, while manual mapping
equality still uses only ID and entries. The option-only raw-string lookup uses
that order for the exact strict-better Bazel spellchecker path and returns the
three-field non-visible requested/owner/suffix identity on a miss. This closes
mapped, unmapped, apparent-root, direct-canonical, first-round, package-relative,
special-main-package, exact repository/package validation, target-triple-dot,
rendering, structural-order, and Java UTF-16 natural-order discriminators.

The four-file change is 365 production, 447 test, and 812 total net lines.
Twenty crate tests, direct dependent checks, GNU-Windows no-run, formatting,
archive, scope, cap, and diff checks pass. Independent Terra source and
representation reviews returned `ACCEPT` after the test-first gate and bounded
validation/triple-dot corrections. No dependency, lockfile, serializer, DICE,
loading/materialization, configuration, target, command, or configured-cycle
edge was added.

Run next only `WP-6-m2-host-input-observation-contract-design`, docs-only.
Freeze the smallest supplied immutable Host snapshot, exact capture timing, and
one-way ownership/dependency handoff before adding any Host observation or
contextual converter. Configured-target dependency cycles remain explicitly
deferred by user approval.

### Windows option-path long-name observation primitive (2026-08-05)

`WP-6-m2-windows-option-path-long-name-observation-primitive` adds the
producer-free option-specific Host/DICE fact in the existing observation
layers. `WindowsOptionPathLongNameOutcome` retains exact pre-lexical UTF-16 as
either `Resolved(Arc<[u16]>)` or distinct `IOExceptionFallback`; demand
identity includes Host namespace, operation, normalized identity path, and one
shared raw UTF-16 Arc. Generic demands reject both UTF-16 operations, while
the exact injected epoch preserves mismatch/duplicate rejection, transient
`Need`, and A -> B -> A success/fallback/payload replay.

The core native adapter reuses the pinned resolver eligibility, sizing/fill,
extended-prefix, and slash-transform helpers. It returns fallback for every
ineligible/native failure and performs no lexical normalization for the new
operation. The Unix adapter is a defensive no-IO fallback. The accepted
repository-oriented `WindowsLongPath` keeps its original direct resolver ->
transform -> lexical-normalization flow; the implementation review restored
that direct flow rather than adding an Arc-to-Vec copy on the existing path.
All non-owner edits are exhaustive impossible-result arms or structural
validation of the new outcome.

The exact eight-file change is 86 production, 302 test, and 388 total net
lines. Focused workspace observation tests pass 18/18, new core tests 3/3,
existing `WindowsLongPath` guards 5/5, full workspace tests 39/39, bzlmod
check, GNU-Windows workspace/core/bzlmod no-run, formatting, archive, scope,
cap, no-Cargo, and diff gates pass. The full core suite reached 131/132; its
sole failure is an untouched external-query assertion expecting the older
`external repository visibility edges are deferred` text while the already
committed query owner returns the more specific wrong-kind-group diagnostic.
An isolated clean-HEAD compile intended to confirm that baseline exhausted
temporary disk quota and its disposable worktree/build output was removed.
The affected focused suites and two independent source-equivalence plus
retained-DICE/representation reviews returned `ACCEPT`.

Run next only `WP-6-m2-host-input-observation-contract-design-retry`,
docs-only. Select the immutable OS/CPU/resource/home snapshot, exact
process/daemon capture owner, one-way core -> configuration handoff, and the
separate complete option-path fact projection. Add no Host read, DICE,
configuration, request, wire, or activation behavior. Configured-target
dependency cycles remain explicitly deferred.

### Host input observation contract retry REPLAN (2026-08-05)

`WP-6-m2-host-input-observation-contract-design-retry` stops before selecting
a schema. The pinned-source/lifetime audit disproves its proposed single
immutable process snapshot: `user.home` is read for every eligible leading
`~/` option conversion, while OS, architecture, and `LocalHostCapacity`
CPU/RAM have independent lazy evaluation, failure, and timing behavior. Those
facts cannot be atomically captured once without changing Bazel-observable
lifetime semantics. The already accepted `WindowsOptionPathLongName` outcome
is a separate request-scoped fact and must retain its success/fallback branch
until later pure conversion.

No Host snapshot, producer, DICE key, configuration schema, converter,
request/daemon activation, Rust, probe, dependency, or runtime behavior was
added. Process facts must not be recaptured by a workspace runtime; the future
configuration boundary remains pure and can depend only one way from core to
configuration. Configured-target cycle semantics remain user-deferred.

Run next only `WP-6-m2-host-input-lifetime-partition-design`, docs-only.
Design an explicit process owner with lazy, independently fallible per-source
result cells, plus separate request/eligible-conversion option facts. Freeze
their exact ownership, capture timing, structural identity, DICE handoff, and
one-shot/daemon lifetimes before authorizing any Host or configuration work.
Stop on any proposed global/static, atomic snapshot, configuration IO,
cross-request option fact reuse, config -> core/workspace edge, new cycle, or
configured-target semantics.

### Host input lifetime partition design ACCEPT (2026-08-05)

`WP-6-m2-host-input-lifetime-partition-design` is **ACCEPT**. The exact Rust
design is a lifetime partition, not a generic `OnceLock<Result<...>>` snapshot.
The older single-snapshot proposal is superseded rather than a future fallback.

Pinned Bazel 9.2 source anchors establish the distinctions: `AutoCpuConverter`
reads `OS.getCurrent()` first and conditionally reads `CPU.getCurrent()` only
for Darwin, Windows, and Linux (`src/main/java/com/google/devtools/build/lib/analysis/config/AutoCpuConverter.java:28-65`);
OS and CPU initialize independent class state (`src/main/java/com/google/devtools/build/lib/util/OS.java:21-84`,
`src/main/java/com/google/devtools/build/lib/util/CPU.java:23-65`); capacity
retains successful local values separately from acquisition and post-ceiling
conversion (`src/main/java/com/google/devtools/build/lib/actions/LocalHostCapacity.java:25-55`,
`src/main/java/com/google/devtools/build/lib/actions/LocalHostResource.java:21-40`,
`src/main/java/com/google/devtools/build/lib/util/ResourceConverter.java:45-66`);
home expansion reads `user.home` during each leading-`~/` conversion
(`src/main/java/com/google/devtools/build/lib/util/OptionsUtils.java:169-174`);
and host path policy is class state (`src/main/java/com/google/devtools/build/lib/vfs/OsPathPolicy.java:66-85`,
`src/main/java/com/google/devtools/build/lib/vfs/PathFragment.java:60`).

`ProcessHostOwner` belongs to core. One-shot execution creates it explicitly
before its `WorkspaceRuntime`; `Daemon::new` creates the sole daemon owner,
and `serve` calls that constructor once before its request loop and never
constructs another. `WorkspaceRuntime` receives only an
`Arc<ProcessHostOwner>`. OS and CPU use independent lazy class-state cells;
`HostPathFlavor` is the pure Windows/Unix derivation reached through
`OsPathPolicy` and `PathFragment` initialization from that same OS state, not a
third Host observation. A first class-initialization failure becomes erroneous
reuse on later access. `AutoCpu` reads OS before its conditional CPU read.
Capacity memoizes only its successful value and retains source-class failure
state separately, so a pre-assignment retryable failure is not blanket-cached.

Home has no cached value. Every eligible flattened leading-`~/` occurrence
performs a fresh, lossless read; a missing/read failure is terminal for that
occurrence and unpaired UTF-16 is `Unsupported`. Before DICE, command-order
observation produces pure supplied values only: source errors do not enter
configuration, and no lock may cross a DICE compute or retry.

The accepted configuration-owned boundary is an Arc-backed
`HostConversionInputs` with no maps, caches, interner, raw source copy, or
producer logic. It contains an optional finite 15-value `AutoCpuToken`, whose
source renderings are `darwin_x86_64`, `darwin_arm64`, `freebsd`, `openbsd`,
`x64_windows`, `arm64_windows`, `piii`, `k8`, `ppc`, `arm`, `aarch64`,
`s390x`, `mips64`, `riscv64`, and `unknown`,
optional Unix/Windows `HostPathFlavor`, optional post-ceiling `i32`
`HostCapacity { host_cpus, host_ram_mib }`, occurrence-ordered unique
`HomeFact { occurrence: u32, home: CompactString }`, and raw-UTF-16
sorted/deduplicated Windows facts. A Windows fact keeps
`Resolved(Arc<[u16]>)` structurally distinct from its fallback outcome. The
schema has structural `Eq`, `Ord`, `Hash`, and `Allocative`; only Arc-backed
wrappers may be `Dupe`.

Later core bridges the existing workspace Windows outcome without importing
workspace types into configuration. Before a new request's pre-scan and first
DICE epoch injection/compute, it removes every inherited
`WindowsOptionPathLongName` demand. It then freshly observes every eligible
expanded raw input, merges only those new facts, and omits Windows facts when
none are demanded. The only dependency direction is core -> configuration.

Run next `WP-6-m2-host-conversion-inputs-schema-implementation`, limited to
the producer-free configuration schema in `native/host.rs` and `native/mod.rs`.
Later serial work is core process-owner/capture with exact source errors, core
request pre-scan/fresh projection, then configuration converters. A mandatory
REPLAN precedes configured-target or command activation; configured-target
cycle deferral remains unchanged.

### Host conversion inputs schema implementation ACCEPT (2026-08-05)

`WP-6-m2-host-conversion-inputs-schema-implementation` adds the public,
producer-free `HostConversionInputs` schema in `slug_configuration_v2`. Its
optional AutoCPU, path-flavor, and capacity fields preserve not-demanded
without forcing a Host read. Strict occurrence-ordered home facts and
raw-UTF-16-ordered Windows facts reject duplicates and reversals; the Windows
outcome retains resolved payload versus `IOExceptionFallback` structurally.

The Arc-backed aggregate and every leaf have structural equality, order, hash,
and `Allocative`; only the aggregate is `Dupe`. Tests freeze all 15 AutoCPU
spellings, full-range capacity values, valid and invalid fact order, unpaired
UTF-16, Arc sharing, and one-field-at-a-time aggregate identity changes. The
change is 233 production, 272 test, and 505 total net Rust lines. Seventeen
crate tests, crate check, GNU-Windows no-run, formatting, archive, scope, cap,
no-Cargo, and diff gates pass; independent source and representation reviews
return `ACCEPT`.

Run next only `WP-6-m2-process-host-owner-capture-design`, docs-only. Freeze
the exact native source, error, retry/latching, synchronization, process
construction, and test-injection contract before core owns any Host read. Add
no Rust, Host access, dependency, DICE, request scan, converter, command, or
configured-target behavior. Configured-target cycle deferral remains
unchanged.

### Process Host owner capture design ACCEPT (2026-08-05)

`WP-6-m2-process-host-owner-capture-design` is **ACCEPT** for the core state
and injection shape. Actual native capture is an explicit **Unsupported**
boundary, not permission to approximate JVM behavior with ordinary Rust OS,
home, CPU, memory, or cgroup APIs.

The pinned Bazel 9.2 (`8220c6198837d5c13d53fea211cf3282aa12408a`) anchors are
`src/main/java/com/google/devtools/build/lib/util/OS.java:48,67-85`
(`blaze.os` before `os.name`), `.../util/CPU.java:46,55-65`,
`.../analysis/config/AutoCpuConverter.java:30-64`, and
`.../vfs/OsPathPolicy.java:66-85`/`PathFragment.java:60`;
`.../util/OptionsUtils.java:169-174` for the fresh `user.home` read;
`.../actions/LocalHostResource.java:23-38` for RAM then CPU;
`.../actions/LocalHostCapacity.java:28-55` for success-only assignment; and
`.../util/ResourceConverter.java:51-54` for post-`ceil` narrowing. Selected
OpenJDK 21 `src/java.base/share/classes/jdk/internal/util/SystemProps.java:60-98`
owns default/overridden properties;
`src/jdk.management/share/classes/com/sun/management/OperatingSystemMXBean.java:102-120`,
`src/jdk.management/unix/classes/com/sun/management/internal/OperatingSystemImpl.java:228-235`,
and `src/java.base/share/classes/java/lang/Runtime.java:667-678` own
container-aware memory and VM-available processors. A Rust environment lookup,
`sysconf`, `/proc`, cgroup-file read, or home-directory helper does not prove
that contract and is not an exact substitute.

`ProcessHostSource` is injected into one non-global core `ProcessHostOwner`.
Its property read result is lossless UTF-16
`Present(Arc<[u16]>) | Absent | ReadError(SourceError)`; raw capture exposes
signed Java-long memory bytes and processor count before Java-derived
conversion, plus a post-resource completion hook that can fail before capacity
assignment. The owner is one source Arc, independent OS/CPU/resource
`ClassCell`s, and one capacity success cell behind the outer shared Arc.
`ClassCellState<T>` is exactly `Vacant | Initializing { thread } | Ready(T) |
Failed(Arc<ClassInitFailure>)`; capacity uses the same first three states but
returns a retryable pre-assignment failure to `Vacant`. The initializing caller
receives `InitialFailure`, while every later access receives the distinct
`ErroneousReuse`; same-thread reentry is a typed internal error, never a wait.
`HostPathFlavor` is derived
only from the OS cell. `AutoCpu` obtains OS first and invokes CPU only for its
supported OS branches. `LocalHostResource` is another class-state evaluation
that reads RAM bytes, immediately divides to a memory-MiB `double`, then reads
processors into its `ResourceSet`. `LocalHostCapacity` stores only that
successful resource value after the post-resource completion hook; a retryable
failure before assignment remains unassigned, while an erroneous resource
class is replayed as its class error. The route-specific CPU/RAM keyword
derivation later applies only `ceil` and Java `double`-to-`int` narrowing; the
process owner does not pretend Bazel caches post-ceiling integers.
Home has no cell and asks the source afresh on every eligible occurrence. Source calls
occur after releasing the mutex; a condition wait is confined to owner-local
publication, and no guard can cross a source call, DICE compute, or retry.
Mutex poison or an unwinding source must fail closed with a typed core owner
error, notify waiters, and never strand `Initializing`. The source trait and
raw state types remain core-private; only the Arc owner and its non-reading
constructor cross into server/runtime ownership. Cloning
`Arc<ProcessHostOwner>` shares one owner and never clones source state;
`WorkspaceRuntime` does not become `Clone`.

The first implementation owns the state machine and an injectable test source.
Its only native source is a non-reading placeholder that returns the typed
`Unsupported` error for every native demand. It must perform no Host I/O. A
future native backend is **REPLAN** until a HotSpot-equivalent mapping proves
property overrides/mutation, lossless platform-string handling, physical-memory
and processor semantics, cgroup behavior, error timing, and each platform's
source boundary.

The owner topology is fixed: six one-shot production construction sites—four
in `app/slug_core_v2/src/runtime/mod.rs` and two in
`app/slug_core_v2/src/runtime/dice.rs`—create an owner before constructing the
runtime. `app/slug_server_v2/src/lib.rs:Daemon::new` is the sole daemon-owner
constructor; `app/slug_server_v2/src/server.rs:serve` remains unchanged and
creates none. `WorkspaceRuntime` accepts only `Arc<ProcessHostOwner>`.

Run next `WP-6-m2-process-host-owner-state-and-injection`. It may implement
only the state machine, unsupported native placeholder, and six-site/daemon
runtime Arc injection. It adds no configuration dependency, DICE/request
bridge, converter, command activation, or real Host I/O. Test state/error
order, conditional CPU, capacity retry/latching, fresh mutable home, owner
isolation, runtime Arc identity, and daemon ownership. Later native capture
remains REPLAN; only a later request bridge may add core -> configuration.
Configured-target cycle deferral remains unchanged.

### Process Host owner state and injection ACCEPT (2026-08-05)

`WP-6-m2-process-host-owner-state-and-injection` is **ACCEPT**. Core now owns
the private injected source/state machine and only exposes an Arc-backed,
non-reading `ProcessHostOwner::unsupported()` constructor. Its OS/CPU/resource
class cells latch first failure versus erroneous reuse, reject same-thread
reentry, recover poisoned publication to a typed failure and wake waiters; the
capacity cell caches only a successful pre-ceil resource sample, restores
`Vacant` after retryable pre-assignment failure, and releases poisoned state.
Home remains fresh, lossless UTF-16, and terminal for absence/read/invalid
surrogate errors. The AutoCPU routes retain the pinned 15-token behavior and
conditional CPU read. No native Host API, DICE key/compute, configuration
dependency, request scan, converter, command activation, or configured-target
edge was added.

Exactly the four `runtime/mod.rs` and two `runtime/dice.rs` one-shot runtime
paths create an owner before runtime construction. `WorkspaceRuntime` accepts
the explicit Arc; `Daemon::new` remains the sole daemon constructor and
`serve` is unchanged. Tests prove source/error/resource order, retry/latch,
fresh home, unsupported isolation, runtime Arc identity, daemon strong count,
and a poisoned lock with an actual Condvar waiter that is not stranded. Two
independent reviews accepted the packet after the poison-recovery correction.

Validation passed: focused process-host 5/5, runtime Arc 1/1, daemon Arc 1/1,
integration runtime 13/13, core/server `cargo check`, formatting, archive,
scope, cap, no-Cargo, and diff gates; GNU-Windows core no-run also passed. The
combined core/server GNU-Windows no-run stops at the pre-existing server
`UnixListener`/`UnixStream` portability error. Full core has 138 passing tests
and one known stale `runtime::dice` external-visibility diagnostic assertion
(`dice.rs:5098`: expected `external repository visibility edges are deferred`,
actual `external repository visibility wrong-kind group is deferred`),
unaffected by this packet. The bounded Rust/doc scope stays within the 600
production, 620 test, 160 documentation, and 1,380 total net line caps. Native
capture remains **REPLAN**. User-approved configured-target cycle deferral
remains unchanged.

Run next only `WP-6-m2-host-request-observation-projection-design`, docs-only.
It freezes request pre-scan and fresh-home/Windows-fact projection, the one-way
core-to-configuration dependency, and DICE epoch/lifetime/error/order
contracts. It adds no Rust, Host capture, converter, activation, command, or
configured-target behavior.

### Host request observation projection design REPLAN (2026-08-05)

`WP-6-m2-host-request-observation-projection-design` is **REPLAN** before
Rust. Pinned converter review shows that Bazel validates defaults at multiple
parse checkpoints and `FieldOptionDefinition` memoizes them. Conversion is not
a generic pre-scan: only priority-accepted, single-value occurrences reach the
converter after their exact default/expansion route. Capacity is eligible only
through the exact `ResourceConverter` route, not merely because a descriptor
names a capacity-shaped option.

The accepted producer-free `HostConversionInputs` schema also cannot preserve
the observed Windows semantics: identical raw UTF-16 values resolve once per
accepted occurrence and may produce different outcomes. Its raw-keyed
deduplication therefore loses both occurrence identity and result multiplicity.
Fresh home has the same converter-call ownership problem. Native capture and a
production native-demand driver remain absent, so no live bridge, DICE input,
or configuration conversion is authorized. No Rust changed. Native capture
remains **REPLAN** and user-approved configured-target cycle deferral remains
unchanged.

Run next only `WP-6-m2-native-conversion-schedule-and-host-fact-redesign`,
docs-only. Freeze the command-owned actual converter-call event schedule
(default checkpoints, priorities, and expansions), per-occurrence home/Windows
identity and outcomes, exact capacity eligibility, and a revised producer-free
schema plus bridge prerequisites. Add no Rust, Host capture, converter,
native-demand driver, DICE/command activation, or configured-target behavior.

### Native conversion schedule and Host-fact redesign ACCEPT (2026-08-05)

`WP-6-m2-native-conversion-schedule-and-host-fact-redesign` is **ACCEPT**.
Converter calls belong to chronological parser batches: default validation
checkpoints, process-wide memoized defaults, full `OptionPriority` acceptance,
and expansion/config/policy boundaries decide whether a converter runs. Only an
actual call receives a dense checked request-local `ConverterCallId(u32)`; a
cold converted default is a call, while a memoized default read is not. Repeat
resource components retain source
and left-to-right component order, including calls made before a later
component-count failure. Full parser and native-demand driver behavior remain
prerequisites for producing this schedule.

The producer-free schema correction is fixed. `HomeFact` carries its call ID.
`WindowsFact` carries call ID, raw UTF-16, and outcome; each fact stream is
strictly call-ID ascending and unique, duplicate raw values are allowed and may
have different outcomes, and the same call may occur in both streams. OS/CPU/
path-flavor/capacity remain optional shared process facts; a successful capacity
value remains in `ProcessHostOwner` even if the request later fails. The
aggregate publishes only after the complete request schedule succeeds. The
direction remains core -> configuration only, with no DICE or configured-target
cycle. Native capture and the production driver remain prerequisites for the
first consumer; user-approved configured-target-cycle deferral is unchanged.

### Host conversion inputs event-schema correction ACCEPT (2026-08-05)

`WP-6-m2-host-conversion-inputs-event-schema-correction` is **ACCEPT**. Its
one-file Rust change (`app/slug_configuration_v2/src/native/host.rs`) is
`+168/-168` net zero, including `+54/-17` production and `+114/-151` test
lines. `ConverterCallId(u32)` provides checked stepping for the future dense
schedule ordinal; Home and Windows facts carry call IDs, while Windows facts
retain raw UTF-16 and their exact outcome.
Each stream accepts only strictly ascending, unique call IDs, but permits gaps,
duplicate raw Windows values with distinct outcomes, and a call ID in both
streams. Optional shared process facts and the Arc-backed aggregate remain
structural and immutable.

Source and representation reviews accepted the result after one test-only
correction that restored all public-leaf `Allocative` checks and full aggregate
Eq/Ord/Hash mutation coverage. Validation passed: focused configuration tests
`17/17`, focused check, GNU-Windows no-run, formatting, archive, scope, cap,
no-Cargo, and diff gates. No converter, core, DICE, driver, capture, Cargo, or
command/configured-target behavior changed.

### Production native conversion schedule-driver design REPLAN (2026-08-05)

`WP-6-m2-production-native-conversion-schedule-driver-design` is **REPLAN**
before Rust. One-shot and daemon build/query/cquery all converge on
`WorkspaceRuntime::drive_command`. The parser/request entrypoint before that
one-shot/daemon split must own a chronological typed converter-event plan and
transport it unchanged on the daemon wire; the shared driver consumes it, never
reconstructs it. `NativeDemandSessionOwner` must be acquired first, so `Busy`
precedes Host work, schedule materialization, and generations. It then filters
inherited Windows-option long-name observations before repository preflight.

The parser-owned immutable event plan is materialized into its schedule plus
Host bundle, including the first configuration-input binding, exactly once per
logical request before the first DICE updater or root attempt, and reused
unchanged across every `Need` retry. A later logical request is fresh; abort
restores the prior accepted bundle and terminal acceptance publishes exactly
the new bundle. No lock may span source, observation, or DICE work. The
chronological source phases are admin CLI,
each unconditional RC chunk, remaining CLI, project file, config
expansions/platform config, command `editOptions`, then invocation policy.
Defaults are memoized, but a cold conversion is a call; a priority-rejected
single option is no call; repeat resources call left-to-right even before a
cardinality error. Full global parser parity additionally needs non-Host
converter ordering/errors, so a five-row shortcut is unsound.

The exact blocker is existing `PathObservationDemand` Windows option identity:
namespace + normalized path + operation + raw UTF-16 has no `ConverterCallId`.
The epoch rejects/deduplicates equal demands and selected paths deduplicate, so
equal raw values at distinct calls cannot retain the distinct outcomes required
by the accepted schema. The dedicated `WindowsOptionPathLongNameOutcome`
already retains `Resolved` versus `IOExceptionFallback` before later lexical
normalization; it is distinct from the older generic `WindowsLongPath` result,
which carries only a UTF-16 path. Within the dedicated option-path
representation, missing converter-call identity is the remaining representation
blocker. The dependency remains core -> configuration: no reverse edge, new
crate, or cycle. Native capture remains separate **REPLAN** and configured-target
cycles remain explicitly user-deferred.

Run next only
`WP-6-m2-windows-option-path-per-converter-call-observation-identity-design`,
docs-only. Design a producer-free occurrence-keyed Windows observation,
selection, and rollback boundary without changing accepted generic observation
semantics. Add no Rust, Cargo, fixtures, DICE, capture, converter, driver, or
configured-target work.

### Per-converter-call Windows option-path observation identity ACCEPT (2026-08-05)

`WP-6-m2-windows-option-path-per-converter-call-observation-identity-design`
is **ACCEPT**. The existing dedicated workspace outcome already preserves
`Resolved` versus `IOExceptionFallback` before lexical normalization. Do not
change generic `PathObservationDemand` or its Eq/Ord/Hash, `Need`,
`PathObservationEpoch`, `SelectedWorkspaceDemands`, or add a DICE key.

After lease acquisition and repository begin, core clones
`prior.selected.unscoped_paths()` into a separate preflight input, filters only
`WindowsOptionPathLongName` from that clone before preflight, and keeps the
prior accepted snapshot intact. After preflight, core
processes each eligible converter call chronologically as a singleton existing
`WindowsOptionPathLongName` generic observation, with that call's
producer-derived normalized absolute identity and exact expanded raw UTF-16.
It extracts the one outcome immediately and never merges a singleton epoch into
`NativeDemandCommand.path_observations`, generic `Need` progress, selected
paths, or repository validation. Equal generic/raw demands at distinct call IDs
therefore invoke the resolver independently and may have distinct outcomes.

Core retains only an ephemeral ordered occurrence assembly
(ordinal/path/raw/outcome): it rejects duplicate or out-of-order IDs and permits
duplicate raw values. The final retained identity is the existing
configuration-owned `WindowsOptionPathFact`/`HostConversionInputs`; no second
retained collection is introduced. A sole one-way core -> configuration bridge
later maps the ordinal exactly to `ConverterCallId`; workspace and configuration
remain independent. The complete new Host input bundle is made once before the
first updater/root, retries reuse it, acceptance retains the exact current
bundle, abort restores the exact prior bundle, and the next request is fresh.
No lock spans observation or DICE. No bounded Rust sidecar is useful before a
live schedule and consumer, so no implementation is authorized. Native capture
remains **REPLAN** and configured-target cycles remain user-deferred.

Run next only `WP-6-m2-process-host-native-capture-source-boundary-evidence`,
docs/source-evidence-only. Pin exact bounded Rust equivalence for Bazel/HotSpot
property precedence/mutation/lossless platform strings, OS/CPU/path-policy
initialization/failures, physical-memory/available-processor/container/cgroup
semantics, and RAM-before-CPU/post-completion timing across supported
platforms; decide source APIs and error/latching mapping. Add no Rust, Cargo,
fixtures, probes, artifacts, DICE, driver, bridge, or configured-target work;
REPLAN or Unsupported any unprovable/JVM-delegation boundary without user
approval.

### Process Host native-capture source boundary evidence REPLAN (2026-08-05)

`WP-6-m2-process-host-native-capture-source-boundary-evidence` is terminal
**REPLAN/Unsupported**. Bazel 9.2 is pinned at
`8220c6198837d5c13d53fea211cf3282aa12408a`: `OS.java:48,67-85` reads
`blaze.os` before `os.name`, `CPU.java:46,55-65` reads `os.arch`, and
`OptionsUtils.java:169-174` reads `user.home` for every eligible conversion.
The actual oracle is Zulu `25.0.2+10`. Its closest official upstream source is
`jdk25u` tag object `935ed5353de37bad0b021a5df15e30e8db7de2fd`, peeled commit
`405a5699ebd097464ed3fc9345414b0774a2edc9`; no Azul artifact-to-source,
patch-set, or VM-flag mapping is proven.

That upstream has VM/`-D` values before platform defaults
(`SystemProps.java:64-75,104-111,284-322`), a mutable/replaced property table
(`System.java:711-815`), and platform-string conversion
(`System.c:79-89,115-139,210-214,240-249`; `jni_util.c:819-835`). Java strings
can retain arbitrary UTF-16 code units (`String.java:273-320`). Therefore no
environment, `uname`, home helper, `sysconf`, `/proc`, or cgroup-file backend
can reconstruct the JVM-observable properties. JDK 25 removed the
SecurityManager property callback: fixed-key property reads have no Java 21
security-error branch, so none is claimed here.

`LocalHostResource.java:23-38` initializes RAM then CPU once; its physical or
container memory is divided as a Java `double`. `Runtime.availableProcessors`
depends on VM configuration including `ActiveProcessorCount`,
`UseContainerSupport`, affinity, cgroup quota/cpuset and mount discovery,
short-lived caches, and platform fallbacks. External Rust lacks the selected
VM flags and vendor provenance, making an exact cross-platform backend
unbounded. `LocalHostResource` first failure is an erroneous class;
`LocalHostCapacity.java:28-55` assigns only after get/log success, and
`ResourceConverter.java:51-54` performs later `ceil`/narrowing. The current
`after_resource` callback is only a private owner publication/test seam, never
a native fact.

JLS 25 §12.4.2 requires same-thread recursive initialization to complete
normally and distinguishes initiating failure from later erroneous reuse. The
current private `SameThreadReentry` and fallible `after_resource` differ from
that parity, but production has only the non-reading `UnsupportedSource`, so
neither is reachable nor a native mapping. Do not expose, correct, or schedule
them now. A future callback or fallible post-step first requires an owner
correction design. Keep `ProcessHostOwner` non-reading and Unsupported; the
user-approved configured-target-cycle deferral remains unchanged.

Run next only `WP-6-m2-repository-label-conversion-route-split-design`,
docs-only. Detach the 41 supplied repository/package-label routes from the
terminal five Host routes using accepted `LabelConversionContext` and
`ResolvedOptionLabel`, and decide a bounded label-only converter successor.
Add no Rust, Cargo, fixtures, source lookup, Host/capture, DICE, command,
normalization, checksum, wire, configured-target, loader, or new-context work.
Stop with REPLAN on Host/capture dependence, a new context/loader, a reverse
edge/cycle, or violation of conversion-before-normalization.

### Repository label conversion route split ACCEPT (2026-08-05)

`WP-6-m2-repository-label-conversion-route-split-design` is **ACCEPT**. The
41 repository/package routes partition exactly into `30 + 9 + 2`: admit now
ordinary `LabelConverter` 16, `EmptyToNullLabelConverter` 5,
`LabelListConverter` 6, `LabelOrderedSetConverter` 1, and
`LibcTopLabelConverter` 2; defer six symbolic defaults (`host_platform` and
five Proto routes) plus incomplete `LabelMap`, `LabelToStringEntry`, and
`FlagAlias` composite grammars; retain mixed command/tokenization routes
`RunUnder` and `CustomFlag`. The five Host routes remain terminal Unsupported,
and the eight Java-regex routes remain separate.

For these 30 only, this supersedes the older all-contextual Host-first serial
step: they need no Host fact. Conceptual `LabelConversionContext` is the
existing `OptionLabelContext`; no context or loader is added, and existing
`ResolvedOptionLabel` is the sole mapping-free retained label. Literal/null/
empty defaults are closed; mappings are borrowed, never retained; conversion
precedes any list/set normalization. A direct configuration-to-identity edge is
acyclic. Retain Arc-backed ordered label slices, derive `Allocative`, and use
`Dupe` only on Arc wrappers; add no map, interner, cache, or global.

Run next only `WP-6-m2-label-only-30-route-converter-implementation`. Its
private API accepts `OptionLabelContext` and handles only the 30 routes and
their defaults. Its exact implementation files are configuration `Cargo.toml`,
`native/mod.rs`, new `native/label_convert.rs`, and `native/tests.rs`; the three
scheduling documents may change only for terminal disposition. No Cargo.lock,
root, identity, registry, convert, defaults, value, cache, command, loading,
DICE, normalization, checksum, wire, configured-target, or Host work is
allowed. Caps are 320 production, 600 test, 100 documentation, and 920 total
formatted net lines. User-approved configured-target-cycle deferral remains
explicit.

### Label-only 30-route converter implementation ACCEPT (2026-08-05)

`WP-6-m2-label-only-30-route-converter-implementation` is **ACCEPT**. The
exact code files are configuration `Cargo.toml`, `native/mod.rs`, new private
`native/label_convert.rs`, and `native/tests.rs`: 145 production Rust, 427 test
Rust, and 573 total formatted net lines including one Cargo line. The only new
dependency is direct, acyclic configuration-to-identity.

The private module uses unqualified family classification and the existing
`OptionLabelContext`; it retains only mapping-free `ResolvedOptionLabel` values,
with Arc label slices, `Allocative`, and wrapper-only `Dupe`. It converts every
ordered-set item before linear first-wins retention; materializes closed
literal/null/empty defaults; and implements LibcTop `default` absence plus
`//` package/colon rewriting to `:everything`. Exactly 30 routes are admitted;
the nine label, two mixed, five Host, and eight regex routes remain Unsupported.
There is no public API, mapping retention, map/interner/cache/global,
normalization, command, loading, DICE, checksum, wire, configured-target, or
Cargo.lock work.

Focused validation reports 22/22 tests, check, GNU-Windows tests check, and
formatting green; archive, scope, cap, and diff gates pass. Independent source
and representation review accepted after the root LibcTop discriminator and
membership/EmptyToNull test-only correction. User-approved configured cycles
remain deferred.

Run next only `WP-6-m2-label-nine-route-source-closure-evidence`, docs/source
evidence only: pin exact Bazel 9.2 spellings for six symbolic defaults and exact
`LabelMap`, `LabelToStringEntry`, and `FlagAlias` grammars/defaults/errors/order/
duplicate/alias validation, then decide a bounded nine-route successor.

### Nine-route source closure evidence REPLAN (2026-08-05)

`WP-6-m2-label-nine-route-source-closure-evidence` is **REPLAN** under pinned
Bazel `9.2.0` commit `8220c619`. `PlatformOptions.java:46,53-66,218-227` fixes
`host_platform` to `@bazel_tools//tools:host_platform`: its explicit empty input
uses that same default, not a Host fact. `ProtoConstants.java:44,47,50-58` and
`ProtoConfiguration.java:67-110` fix the other bytes: protoc
`@bazel_tools//tools/proto:protoc`; CC `@bazel_tools//tools/proto:cc_toolchain`;
J2ObjC `@bazel_tools//tools/j2objc:j2objc_proto_toolchain`; Java
`@bazel_tools//tools/proto:java_toolchain`; JavaLite
`@bazel_tools//tools/proto:javalite_toolchain`.

`FieldOptionDefinition.java:324-357` converts a non-null default with the
supplied active context then memoizes it. All six defaults are nonempty and use
the normal label path; the three core EmptyToNull Proto routes return null only
for explicit empty input, while compiler and JavaLite ordinary Label empty
inputs parse normally. Label syntax diagnostics are the helper's
`OptionsParsingException` message, wrapped for default construction.

`LabelToStringEntryConverter` is closed (`CoreOptionConverters.java:247-269`):
exactly one `=`, neither side empty, context-parsed lhs, and untrimmed
`CompactString` rhs,
and `Variable definitions must be in the form of a 'name=value' assignment.
'name' and 'value' must be non-empty and may not include '='.`; its repeatable
null default is absent. Seven
routes are therefore bounded without a loader, Host fact, command, or JVM.
`LabelMap` (`:196-244`) preserves linked insertion order and parses label values
before duplicate-key rejection; `JavaOptions.java:239-247` sets
`bytecode_optimizers` default `Proguard`. Its exact trim grammar depends on
Guava `Splitter`/`CharMatcher`,
outside the Bazel-only authority. `FlagAlias` (`:363-405`) validates `=` shape,
`--flag_alias=` diagnostic prefix, `\\w*`, embedded `=`, Starlark prefixes, and
then label-parses; exact Java Pattern word-domain evidence is likewise absent.
Its converter remains distinct from downstream command alias expansion and C
normalization.

Thus the full nine is REPLAN: defer LabelMap and FlagAlias to later Guava/JDK
evidence; keep the five Host routes Unsupported, RunUnder/CustomFlag mixed, and
eight regex routes deferred. No JVM need is claimed; configured cycles remain
user-deferred. Run next only `WP-6-m2-label-seven-route-converter-implementation`:
private additions only to existing `native/label_convert.rs` and `native/tests.rs`
(scheduling docs terminal-only), no Cargo/mod/identity or broader work; caps are
240 production, 420 test, 100 documentation, and 760 total formatted net lines.

### Seven-route label converter implementation ACCEPT (2026-08-05)

`WP-6-m2-label-seven-route-converter-implementation` is **ACCEPT**. It admits
exactly 37 label routes and leaves LabelMap/FlagAlias as the two deferred label
routes. The private extension adds the six literal defaults and
LabelToStringEntry, preserves mapping-free values, and covers first-round, main,
and package contexts. Its private delimiter failures return `LabelConvertError::Invalid`;
the fixed source diagnostic and all user-facing diagnostic projection remain
deferred. User-approved configured-target cycles remain explicitly deferred.

Focused validation reports 24/24 tests, crate check, GNU-Windows tests check,
formatting, archive, scope, cap, and diff gates green. Formatted net is 128
production Rust plus 221 test Rust (349 Rust net); the complete five-file diff
is 357 net. Run next only
`WP-6-m2-label-map-and-flag-alias-library-semantics-evidence`: docs/source only
for Guava 33.5.0 Splitter/CharMatcher trimming/order/duplicate behavior and JDK
25 Pattern `\w` plus exact FlagAlias validation/diagnostics; retain
converter-versus-command-alias/normalization ownership.

### LabelMap and FlagAlias library semantics evidence ACCEPT (2026-08-05)

`WP-6-m2-label-map-and-flag-alias-library-semantics-evidence` is **ACCEPT**.
Pinned Bazel `9.2.0` is `8220c619`; its MODULE selects Guava `33.5.0-jre`, and
official Guava `v33.5.0` is `8868c096`. [Its root POM](https://github.com/google/guava/blob/8868c096cfdabbe38170b6e395369c315cfb72a1/pom.xml#L6-L10)
pins that JRE version; [`trimResults`](https://github.com/google/guava/blob/8868c096cfdabbe38170b6e395369c315cfb72a1/guava/src/com/google/common/base/Splitter.java#L291-L342)
and [the iterator](https://github.com/google/guava/blob/8868c096cfdabbe38170b6e395369c315cfb72a1/guava/src/com/google/common/base/Splitter.java#L550-L611)
trim before omission and iterate left-to-right; [`CharMatcher`](https://github.com/google/guava/blob/8868c096cfdabbe38170b6e395369c315cfb72a1/guava/src/com/google/common/base/CharMatcher.java#L1232-L1265)
pins the 25 BMP whitespace characters: `U+0009-U+000D`, `U+0020`, `U+0085`,
`U+00A0`, `U+1680`, `U+2000-U+200A`, `U+2028-U+2029`, `U+202F`, `U+205F`,
and `U+3000`.

Bazel's [LabelMap converter](https://github.com/bazelbuild/bazel/blob/8220c6198837d5c13d53fea211cf3282aa12408a/src/main/java/com/google/devtools/build/lib/analysis/config/CoreOptionConverters.java#L196-L220)
is manual, not MapSplitter: every comma piece is whitespace-trimmed before
empty omission, then it splits the first `=` with no separate key/RHS trim,
parses a nonempty label before duplicate detection, retains LinkedHashMap
insertion order, returns an unmodifiable map, defaults `bytecode_optimizers` to
`Proguard`, and reports `Key '<key>' appears twice`. Guava MapSplitter's
separate malformed/duplicate rules are not substituted. Official OpenJDK
`jdk-25.0.2+10` tag object `935ed5353de37bad0b021a5df15e30e8db7de2fd` peels to
`405a5699ebd097464ed3fc9345414b0774a2edc9`: [Pattern's default `\w` table](https://github.com/openjdk/jdk25u/blob/405a5699ebd097464ed3fc9345414b0774a2edc9/src/java.base/share/classes/java/util/regex/Pattern.java#L182-L186)
and [Pattern.matches](https://github.com/openjdk/jdk25u/blob/405a5699ebd097464ed3fc9345414b0774a2edc9/src/java.base/share/classes/java/util/regex/Pattern.java#L1202-L1224)
fix unflagged `\w` to `[a-zA-Z_0-9]` and reject non-ASCII; [`Matcher.matches`](https://github.com/openjdk/jdk25u/blob/405a5699ebd097464ed3fc9345414b0774a2edc9/src/java.base/share/classes/java/util/regex/Matcher.java#L746-L753)
is whole-region. This establishes the Java SE 25 contract; vendor-build source
provenance remains unclosed, and no JVM/runtime dependency is claimed.

FlagAlias source validation is ordered: require nonempty left of the first `=`
(`Flag alias definitions must be in the form of a 'name=label' assignment`),
then build the `--flag_alias=` diagnostic prefix; require `\w*`
(`{short} should only consist of word characters to be a valid alias name.`);
reject later `=` (`--flag_alias does not support flag value assignment.`); only
then require the `--` target to start `--//`, `--no//`, `--@`, or `--no@`
(`--flag_alias only supports Starlark build settings.`); then parse the label
with the supplied context. Its repeatable-null default is absent. Later C
last-wins/sort and command alias expansion remain outside converter scope.
Configured-target cycles remain user-deferred.

Run next only `WP-6-m2-label-map-and-flag-alias-converter-implementation`:
private 39/0 extension in existing `native/label_convert.rs` and `native/tests.rs`
only (scheduling docs terminal-only), exact Unicode trim and no Guava/JDK/regex/
dependency; caps are 280 production, 440 test, 100 documentation, and 820 total
formatted net lines.

### LabelMap and FlagAlias converter implementation ACCEPT (2026-08-05)

`WP-6-m2-label-map-and-flag-alias-converter-implementation` is **ACCEPT**:
the private converter reaches the exact 39/0 label partition. LabelMap retains
an Arc ordered `(CompactString, Option<ResolvedOptionLabel>)` slice, applies
the exact 25-character trim before omission, takes the first `=` without
key/RHS trim, converts a label before linear duplicate rejection, and materializes
the `Proguard` default. FlagAlias retains an unnormalized per-occurrence compact
scalar, uses allocation-free ASCII/prefix checks, and materializes its
repeatable-null default as absent. Both use supplied context with mapping and
non-visible identity; private `Invalid` remains the boundary and user diagnostics
remain deferred. No downstream normalization, command activation, loader, or new
dependency is added. The Buck reuse boundary is unchanged: retained Arc/
CompactString values only, with no new hot-path utility, map, cache, or interner.
Configured-target cycles remain user-deferred.

Focused validation reports 26/26 tests, crate test/check, GNU-Windows test
check, formatting, archive, scope, cap, and diff gates green. Formatted net is
113 production plus 268 test Rust, 381 Rust net. Run next only
`WP-6-m2-run-under-and-custom-flag-source-closure-evidence`, docs/source only:
pin Bazel 9.2 RunUnder value/converter/default/error/rendering and ShellUtils
tokenization/original-suffix/context split, plus CustomFlag raw-define versus
label `/...` canonicalization/default/error/context; decide a bounded successor
without command activation, normalization, loader, checksum, wire, DICE, or
configured-target work.

### RunUnder and CustomFlag source closure; renderer REPLAN (2026-08-05)

`WP-6-m2-run-under-and-custom-flag-source-closure-evidence` is **ACCEPT** for
private conversion over well-formed Unicode input and **REPLAN** only for exact
RunUnder record rendering/cache and the full Java `String` domain. All Bazel
anchors below are pinned `9.2.0` (`8220c6198837d5c13d53fea211cf3282aa12408a`).

`RunUnderConverter.java:29-62` tokenizes into a fresh list, rejects no tokens
with `Empty command`, takes token zero as command/label candidate and the rest
as ordered suffix, and retains the unmodified input. A decoded first token
beginning `//` or `@` label-converts through the supplied context; every other
first token is a command. Tokenizer errors are wrapped exactly as `Not a valid
command prefix ` plus `ShellUtils`'s message; label errors are `Not a valid
label ` plus the label error. `RunUnder.java:22-62` supplies the raw original,
suffix, and label/command payload records. `ShellUtils.java:90-143` is a
self-contained UTF-16 state machine: only unquoted space/tab split; quote
fragments concatenate and force empty quoted tokens; single quotes make backslash
literal; unquoted backslash consumes the next code unit; double-quote backslash
removes it only before backslash or quote and otherwise retains it. Its exact errors are
`backslash at end of string` and `unterminated quotation`. `CoreOptions.java:
640-659` declares default `"null"`; `Option.java:63-70` assigns that literal its
special-default meaning and `FieldOptionDefinition.java:337-339` returns null
without invoking the converter. Thus absent RunUnder is null, while literal
command-line `null` converts as command `null`.

The label context is `CoreOptionConverters.java:331-360`: PackageContext first,
otherwise mapping-free canonical parse for null context or main-repository
mapping parse. `CoreOptionConverters.java:275-302` (enclosed by `:270-308`)
returns inputs not beginning `//` or `@` raw as defines. Its label branch
substitutes trailing `/...` with
`:__subpackages__`, converts through that same helper, renders
`Label.getUnambiguousCanonicalForm()` (`Label.java:468-469`), and rewrites any
result ending `:__subpackages__` to `/...`. In main-repository contexts, both
`//pkg/...` and the valid label `//pkg:__subpackages__` result in `@@//pkg/...`.
Corresponding `@apparent//pkg/...` and `@apparent//pkg:__subpackages__` result
in mapped/resolved `@@repo//pkg/...`; PackageContext omits `@` for its current
repository. This collision is required. `CoreOptions.java:121-135` is a
repeatable `"null"` default; `Option.java:63-70` and
`FieldOptionDefinition.java:337-339` therefore own its absent/empty result
without converter invocation. Explicit command-line `null` is literal. The raw
branch adds no error; label failures are the helper's unwrapped
`OptionsParsingException` message.

The conversion slice has no JDK Unicode-table dependency: for every valid
Unicode scalar string, Rust `&str` scanning is equivalent because only ASCII
control characters are inspected and all other text is copied. Java allows lone
UTF-16 surrogates, which Rust `&str` cannot represent; UTF-16/WTF-8 ingestion
and the full Java-String domain remain deferred.

The records in `RunUnder.java:52-62` have implicit Java `toString`. Bazel's
`OptionsBase.java:86-116` cache key calls `value.toString()` for every non-null
option. OpenJDK jdk25u tag `jdk-25.0.2+10` (tag
`935ed5353de37bad0b021a5df15e30e8db7de2fd`, source
`405a5699ebd097464ed3fc9345414b0774a2edc9`) documents in `Record.java:186-193`
that precise record rendering may change; its `ObjectMethods.java:237-307`
shows the upstream implementation only. Active Zulu renderer provenance is
unclosed and this packet forbids a JVM/probe, so no Rust record renderer or
cache-key claim is authorized.

Command activation, runfiles, non-test trim, normalization, loader, checksum,
wire, DICE, new dependencies, and user-deferred configured-target cycles remain
out of scope. Run next only
`WP-6-m2-run-under-and-custom-flag-converter-implementation`: private changes
to existing `app/slug_configuration_v2/src/native/label_convert.rs` and
`app/slug_configuration_v2/src/native/tests.rs`, then the three scheduling docs
terminal-only. Preserve classifier 39/0 and add two separate mixed 2/0 private
functions. `RunUnderSuffix(Arc<[CompactString]>)` is the sole `Dupe` wrapper;
the private `Allocative` `RunUnder` is either
`Label { original: CompactString, suffix: RunUnderSuffix, label: ResolvedOptionLabel }`
or `Command { original: CompactString, suffix: RunUnderSuffix, command: CompactString }`.
CustomFlag finishes as `CompactString`. Preserve source tokenizer state/error
ordering but map all failures to private `LabelConvertError::Invalid`; source
texts remain evidence and user diagnostic projection stays deferred. Implement
only the valid-Unicode source tokenizer, raw/suffix/context and `/...` collision
rules—never renderer/cache, full Java strings, activation, or a new dependency.
Caps: 300 production, 500 test, 100 documentation, 900 total formatted net
lines.

### RunUnder and CustomFlag converter implementation ACCEPT (2026-08-05)

`WP-6-m2-run-under-and-custom-flag-converter-implementation` is **ACCEPT**.
The existing label classifier remains exact 39/0; `classify_mixed` adds exactly
the two mixed routes and no label membership. The private valid-Unicode
ShellUtils state machine preserves space/tab splitting, quote concatenation and
empty tokens, single/double-backslash behavior and failure order, then classifies
decoded token zero. It retains the raw original plus
`RunUnderSuffix(Arc<[CompactString]>)`; its Label/Command payload is the
existing `ResolvedOptionLabel` or `CompactString`. First-round, mapped,
PackageContext/current-repository, and non-visible contexts pass; special-null
defaults are absent and explicit `null` remains literal.

CustomFlag preserves raw nonlabel defines and finishes as `CompactString`; its
`/...` sentinel/rewrite retains the `:__subpackages__` collision, unambiguous
main/mapped/package output, and bare `@repo` shorthand. All converter failures
remain private `LabelConvertError::Invalid`; user diagnostics stay deferred.
The allocation correction retains first token separately from the suffix rather
than removing element zero, and transfers the owned unambiguous string directly
to `CompactString`. `RunUnderSuffix` is the only new Arc-backed `Dupe` wrapper;
no new utility, cache, or interner is added.

No renderer/cache claim, full Java-String/lone-surrogate support, command
activation, runfiles/non-test trim, normalization, loader, checksum, wire, DICE,
or configured-cycle work is introduced. Exact RunUnder record renderer/cache
and full Java String remain **REPLAN**; configured-target cycles remain
user-deferred. Validation is 30/30 focused tests, crate test/check,
GNU-Windows tests check, formatting, archive, scope, cap, and diff gates green.
Formatted net is 180 production plus 450 test Rust, 630 Rust net.

Run next only `WP-6-m2-java-regex-route-source-closure-evidence`, docs/source
only. Inventory and pin `RegexFilter` (3), `ExecutionInfoModifier` (1),
`PerLabelOptions` (3), and `RunsPerTest` (1): Bazel 9.2 grammar/default/error/
order/duplicate/render/cache behavior and JDK 25 `Pattern`/`Matcher` dependence.
Decide bounded subsets or REPLAN without a JVM, runtime regex dependency, Rust,
Cargo, probes, or artifacts. Keep Host terminal Unsupported and command,
activation, loading, DICE, normalization, checksum, wire, and user-deferred
configured cycles excluded; only the three scheduling docs may change, at 260
documentation/total net lines.

### Java regex route source closure ACCEPT (2026-08-05)

`WP-6-m2-java-regex-route-source-closure-evidence` is **ACCEPT** as complete
source closure; every general explicit route is **REPLAN**. Within the 341-row
native registry, the inventory is `3 + 1 + 3 + 1 = 8`; its RegexFilter rows are
`toolchain_resolution_debug`, `archived_tree_artifact_mnemonics_filter`, and
`instrumentation_filter`; `ExecutionInfoModifier` owns
`modify_execution_info`; `PerLabelOptions` owns `host_per_file_copt`,
`per_file_copt`, and `per_file_ltobackendopt`; `RunsPerTest` owns `runs_per_test`.

Pinned Bazel 9.2 `8220c619` fixes the converter boundary. `RegexFilter.java`
54-93 splits with source literal `"(?<!\\\\),"`, rejects a leading `--`,
strips one leading sign, drops empty pieces without trimming, and wraps JDK
syntax errors. Lines 128-170 then sort/deduplicate inclusion and exclusion
strings, compile atomic-group unions, use exclusion-first `Matcher.find()`, and
render the generated union rather than the input; 173-213 retain the original
only for reversal while equality/hash use nullable generated pattern sources.
`ExecutionInfoModifier.java:38-100,103-152` splits on every literal comma and
requires every piece to match `^(?<pattern>.+)=(?<sign>[+-])(?<key>.+)$`, so
empty pieces or missing pattern/key fail. It validates
`internalToUnicode(pattern)` with DOTALL, later compiles without
DOTALL, and fully matches `internalToUnicode(mnemonic)`. Duplicate expressions
remain ordered; later matching same-key entries win. Additive occurrences apply
in command order, and nonadditive mode uses only the last occurrence.
AutoValue equality retains the raw option plus ordered expressions, while cache
text additionally depends on its unpinned generated renderer.
`PerLabelOptions.java:46-86,88-139` splits at the first `@`, uses the same regex
delimiter for option text, drops trim-empty options but otherwise preserves
ordered duplicates, delegates the filter, and renders the canonical filter plus
ordered options. Repeated occurrences remain ordered. Its three repeat defaults
and the ExecutionInfo default are special-null empty lists without converter
calls. Starlark reversal instead uses raw `toOriginalString()` plus the joined
ordered options at 72-79. `Converters.java:414-430` owns the DOTALL validator, and
`OptionsBase.java:90-116` routes every nonempty value through Java `toString`.
Diagnostics retain their source-owned text: RegexFilter distinguishes the
leading-flag-looking failure from `Failed to build valid regular expression: `
plus the JDK message; ExecutionInfo uses `malformed expression '<piece>'` or
`Not a valid regular expression: ` plus that message; PerLabel delegates; and
Runs embeds the raw input in its positive-count, multiplicity, and numeric
failures.

`TestConfiguration.java:534-577` first tries Java `Integer.parseInt`; positive
values retain their exact source spelling as the sole option, nonpositive
values fail, and only `NumberFormatException` falls back to the general
`PerLabelOptions` route. Consequently explicit `1`, `01`, and `+1` are not
equal/cache-identical even though their numeric values agree. The existing
default-only `runs_per_test="1"` seed remains exact and gains no explicit
route.

The actual-runtime nearest source remains `jdk25u` tag `jdk-25.0.2+10`, peeled
commit `405a5699`. `Pattern.java:1101-1182` owns compilation, flags, source
rendering, and matcher construction; its grammar summary at 140-401 exposes the
arbitrary Java-regex language that remains a JDK boundary.
`Matcher.java:742-785` distinguishes whole-region `matches()` from subsequence
`find()`. `String.java:3396-3424,3450-3513` routes the delimiter through
`Pattern` and removes trailing empty split results. `PatternSyntaxException.java`
102-118 makes diagnostics depend on the JDK description/index, platform line
separator, pattern, and caret. This packet authorizes no JDK-compatible engine,
Rust approximation, JVM/delegation, or runtime dependency for that surface.

Three annotated defaults are nevertheless finite exact seeds. Both `-.*`
defaults normalize/render as `-(?:(?>.*))` and reject every Java String because
`find()` admits the empty `.*` match. The instrumentation default retains raw
`-/javatests[/:],-/test/java[/:]`, normalizes/renders as
`-(?:(?>/javatests[/:])|(?>/test/java[/:]))`, and rejects any value containing
`/javatests/`, `/javatests:`, `/test/java/`, or `/test/java:`. This remains exact
over arbitrary UTF-16. Raw input is retained separately but does not participate
in equality. Coverage
dynamically replaces it only for TestCommand coverage without an explicit flag
(`TestCommand.java:161-165`). `AnalysisAndExecutionPhaseRunner.java:91-108` and
`AnalysisPhaseRunner.java:114-131` assign the package-derived heuristic from
`InstrumentationFilterSupport.java:55-121`; that activation remains REPLAN
with every explicit occurrence.

Run next only `WP-6-m2-fixed-regex-default-seed-implementation`. Add a private
owned-original plus finite semantic seed for exactly these three descriptor/
converter/default tuples, exact canonical cache rendering, and no occurrence
classification. Allowed Rust is only native `value.rs`, `defaults.rs`,
`cache_grammar.rs`, and `tests.rs`; terminal scheduling may change the three
plan documents. Add no registry/convert/Cargo/dependency, Pattern/Matcher,
dynamic coverage, public reversal/predicate activation, normalization,
configuration/checksum/DICE, Host, loader, wire, or configured-cycle work.
Keep the Runs seed and 287/8/5/41 partition unchanged. Caps are 150 production,
240 test, 100 documentation, and 490 total formatted net lines.

### Fixed RegexFilter default seed implementation ACCEPT (2026-08-05)

`WP-6-m2-fixed-regex-default-seed-implementation` is **ACCEPT**. Exactly the
two `-.*` descriptors and the one instrumentation descriptor materialize before
generic classification. The selector pins class, canonical name, field type,
converter, raw default, and nonrepeat shape; a mutated repeat descriptor falls
through to Unsupported. Every explicit occurrence remains Unsupported.

The private seed retains original text in `CompactString` but implements Eq/Ord
only through `ExcludeAll` versus `InstrumentationDefault`, matching normalized
RegexFilter identity. Its exact cache strings are `-(?:(?>.*))` and
`-(?:(?>/javatests[/:])|(?>/test/java[/:]))`. It derives `Allocative`, not
`Dupe`; no Arc, utility import, interner, map, cache storage, or dependency is
added. The existing Runs seed and 287/8/5/41 partition remain unchanged.

The four authorized Rust files add 89 production and 96 test lines, 185 total.
Focused validation passes 31/31 tests, crate check, GNU-Windows tests check,
formatting, archive, scope, cap, no-Cargo, and diff gates. Arbitrary explicit
regex, JDK Pattern/Matcher and diagnostics, reversal/predicate/coverage
activation, Host capture, exact RunUnder rendering/full Java Strings, and
configured-target cycles remain excluded or REPLAN.

M2 is now parked: no exact configuration equality/cache/checksum substrate can
omit the terminal Host and regex values, and another private schema would lack
a complete producer/consumer. Run next only the independent Stage 10 packet
`WP-10-m8-bazel-developer-graph-boundary-design`, docs-only. Inventory the live
Cargo workspace and pin the smallest Bazel 9.2/rules_rust developer graph for
the `slug_cli_v2` transitive closure before any Bazel metadata is written.

### Windows option-path short-name resolution design (2026-08-05)

`WP-6-m2-windows-option-path-short-name-resolution-design` closes the
filesystem-dependent part of Bazel 9.2 option-path conversion at pinned tag
`9.2.0` (`8220c6198837d5c13d53fea211cf3282aa12408a`).

`WindowsPathOperations.isShortPath` matches a complete UTF-16 path segment
against `^(.{1,6})~([0-9]{1,6})(\\..{0,3}){0,1}`, requires at most twelve
code units, and requires the two captured groups to total fewer than eight;
Java's default regex dot excludes line terminators. Backslashes, repeated
separators, and dot segments can request ordinary lexical normalization, but
`WindowsOsPathPolicy.needsToNormalize` promotes the level to
`NEEDS_SHORT_PATH_NORMALIZATION` only when a segment passes the short-path
predicate. At that level `normalize` first calls the default resolver on the
complete pre-normalization string, replaces the string on success, retains the
original on `IOException`, and only then normalizes separators, dot segments,
and drive-letter case. The Java/native chain is
`WindowsPathOperations.getLongPath` -> `nativeGetLongPath` -> `GetLongPath` ->
`GetLongPathNameW`; its boundary is lossless UTF-16, adds/removes the extended
prefix, and changes returned backslashes to slashes.

Pinned anchors are
`src/main/java/com/google/devtools/build/lib/windows/WindowsPathOperations.java:42-91`,
`src/main/java/com/google/devtools/build/lib/vfs/WindowsOsPathPolicy.java:43-56,77-163`,
`src/main/java/com/google/devtools/build/lib/vfs/PathFragment.java:124-143`,
`src/main/java/com/google/devtools/build/lib/vfs/OsPathPolicy.java:66-85`,
`src/main/native/windows/file-jni.cc:163-183`, and
`src/main/native/windows/file.cc:73-87,182-197`.

`OptionsUtils.convertOptionsPathFragment` expands only a literal leading
`~/`, uses Java `String.replace` to replace every `~` with `user.home`, and
then creates the `PathFragment`, so scanning happens after expansion and can
see a short-looking segment introduced by home. `platform_mappings` empty
input returns `PlatformMappingKey.DEFAULT` without conversion. Every nonempty
input converts before absolute rejection or construction of the explicit
workspace-relative key. Unix host policy never performs the Windows candidate
scan, even for a short-looking segment; Windows inputs without a matching
segment remain purely lexical. A Windows-host matching input that fails
Bazel's native absolute-normalized precondition after `asLongPath(input)` has
a deterministic caught-`IOException` fallback and needs no filesystem
observation; the candidate predicate and the native-eligibility predicate
remain distinct.

Those option anchors are
`src/main/java/com/google/devtools/build/lib/util/OptionsUtils.java:98-104,169-174`,
`src/main/java/com/google/devtools/build/lib/analysis/PlatformOptions.java:233-247`,
and `src/main/java/com/google/devtools/build/lib/vfs/UnixOsPathPolicy.java:20-108`.

The live `PathObservationDemand::windows_long_path` already provides a Host
namespace, raw `Arc<[u16]>` identity, an exact injected epoch, transient
self-unequal `Need`, outside-DICE observation, command retry, and A -> B -> A
replay. It is not the option-conversion fact: its result is only the final
lexically normalized `Arc<[u16]>`, so it collapses native success and fallback
and moves post-resolver normalization into the producer. Keep that accepted
repository-path operation and every existing consumer byte-for-byte semantic.
The live anchors are
`app/slug_workspace_v2/src/path_observation.rs:244-320,617-790,1329-1380`,
`app/slug_core_v2/src/runtime/path_observation.rs:135-223,862-1082,1688-1834`,
and `app/slug_core_v2/src/runtime/dice.rs:3613-3762,4088-4200`.

The option-specific operational fact is instead:

```text
WindowsOptionPathLongNameOutcome =
    Resolved(Arc<[u16]>)
  | IOExceptionFallback
```

Its dedicated Host demand retains the complete expanded, non-normalized raw
UTF-16 input plus a producer-derived normalized-absolute identity. `Resolved`
contains the exact long spelling after extended-prefix removal and
backslash-to-slash conversion but before Windows lexical normalization.
`IOExceptionFallback` is a distinct value and carries no diagnostic because
Bazel catches the exception; the consumer recovers the raw input from the
demand. The two variants remain unequal even if later lexical normalization
would yield identical paths. Demand identity is structural namespace + path +
operation + raw code units with exact `Eq`/`Ord`/`Hash`; the outcome is
structural `Eq`/`Ord`/`Hash` and `Allocative`, and only its Arc payload may use
`Dupe`.

`slug_workspace_v2` owns this producer-free DICE demand/outcome primitive and
`slug_core_v2` owns the direct native observation adapter. A later command
owner scans home-expanded raw occurrences only under Windows host policy,
requests only facts whose result can depend on filesystem state, retries
outside DICE, and projects a complete
raw-input-sorted/deduplicated `Arc<[WindowsOptionPathFact]>` into the pure
configuration converter. That future configuration schema owns no PathBuf,
OsString, DICE, IO, or workspace type; core is the sole future
workspace/configuration bridge. A missing fact in a supposedly complete
projection is a core assembly invariant failure. Duplicate demands or
operation/result mismatch remain epoch construction errors. Native failure is
the ordinary fallback value, never a configuration error.

Each one-shot command and each daemon request must begin with a fresh command
observation owner and exact epoch; a retained daemon DICE graph may not reuse
prior option facts merely because its process survives. Epoch equality and
the option outcome distinguish success/fallback and raw/payload changes.
Downstream configuration equality includes only the eventual converted path,
so equal normalized values may prune after the operational edge invalidates.
No lock is held across DICE compute or retry.

The design is `ACCEPT`. Implement next only the producer-free
`WP-6-m2-windows-option-path-long-name-observation-primitive` in the existing
workspace/core observation layers. The general Host snapshot, `user.home`
capture, option pre-scan, ordered configuration projection, contextual
conversion, core -> configuration dependency, command/request/wire ownership,
daemon activation, checksum, and configured targets remain later. That later
integration must `REPLAN` on a reverse dependency/new crate/cycle, stale
cross-request facts, hidden configuration IO, missing raw UTF-16 identity, or
an owner that holds a lock across DICE computation. Configured-target cycles
remain explicitly deferred.

### Host input observation contract design REPLAN (2026-08-05)

`WP-6-m2-host-input-observation-contract-design` stopped at its pinned-source
path-policy gate. Bazel 9.2's OS/CPU token table, process-lifetime lazy Host
resource capture, post-ceil integer CPU/RAM keyword inputs, and `user.home`
replacement behavior closed without a probe. The assumed finite lexical
`HostPathPolicy::{Unix, Windows}` did not.

On a Windows host, `PathFragment.create` selects
`WindowsOsPathPolicy.INSTANCE`. A segment matching Bazel's 8.3 short-name
predicate promotes normalization to `NEEDS_SHORT_PATH_NORMALIZATION`, whose
default resolver calls `WindowsPathOperations.getLongPath`; success substitutes
the observed long path and `IOException` falls back to the original. The exact
result therefore depends on filesystem state/access. Both
`shell_executable` and `platform_mappings` reach this call through
`OptionsUtils.convertOptionsPathFragment`; platform-mapping absolute rejection
happens only afterward.

The first source report incorrectly described this path as lexical-only. Root
rechecked pinned tag `9.2.0` (`8220c619...`), triggered the manifest stop, and
retained no proposed Host representation or owner. No plan design, Rust, test,
probe, Cargo/dependency, Host read, DICE, request, daemon, configuration, or
runtime edit remains. Schema-only implementation is not authorized while one
of its five routes has unresolved observation/invalidation ownership.

Run next only
`WP-6-m2-windows-option-path-short-name-resolution-design`, docs-only. Pin the
exact 8.3 predicate/GetLongPath/fallback/conversion-order contract and audit
Slug's existing lossless `WindowsLongPath` path-observation demand/result,
Host namespace, DICE ownership, and transaction lifecycle. Select the smallest
supplied observation seam that preserves conversion-before-normalization and
configuration identity, or record an unsupported boundary/`REPLAN`. Do not
implement Host inputs, path conversion, IO, DICE, configuration, or request
wiring. Configured-target cycles remain explicitly deferred.

### Java/Guava renderer authority evidence REPLAN (2026-08-04)

`WP-6-m2-java-guava-renderer-authority-evidence` bound Bazel 9.2's exact Zulu
25.0.2 runtime, Java SE 21 renderer contracts, and Guava `33.5.0-jre` source to
the pinned JAR SHA. Its temporary probes matched record punctuation, immutable
list/entry and map text, outer escaping, lowercase timeout keys, durations, and
the CompilationMode, StripMode, PlatformType, and TestTimeout overrides. The
terminal correction review nevertheless found the UTF-16 ordering evidence
non-discriminating: the purported sorted list inserted U+10000 before U+E000,
which was already Java UTF-16 order, rather than proving a reverse-order input
passed through the required `distinct().sorted()` path. The entire unaccepted
Stage 6 matrix was discarded and all temporary Java, JAR, and source artifacts
were deleted.

Run next only `WP-6-m2-java-guava-renderer-authority-evidence-retry`. Rebuild
the same documentation-only matrix at no more than 240 Stage 6/300 total net
lines, reusing the already observed runtime/source/hash facts but claiming only
freshly recorded evidence. Its UTF-16 probe must start with actual U+E000 then
U+10000 scalars, apply the production-equivalent distinct-then-natural-sort
path, and record code units plus exact bracketed cache bytes showing U+10000
then U+E000. Any disagreement, already-ordered/non-list shortcut, persistent
fixture need, second correction, or Java/JVM production implication is
`REPLAN`. Preserve 287/8/5/41; descriptor/family grammar, Rust, contextual and
regex conversion, normalization, checksum, wire, DICE, and configured-target
cycles remain deferred.

### Pure native converter source-closure ledger REPLAN (2026-08-04)

The combined `WP-6-m2-pure-native-converter-source-closure-ledger` produced a
correctly counted 287-row draft and one compact family table, then exhausted
its permitted correction without closing the byte contract. Its final review
still found incomplete concrete enum converter/value/renderer chains, no exact
EnvVar record strings or EnumMap/Duration bytes, ambiguous supplementary/BMP
UTF-16 input notation, and undefined default-route discriminator IDs. These are
material source-closure gaps, not formatting. The entire unaccepted Stage 6
diff was discarded with `apply_patch`; HEAD returned cleanly to `a9c05049`.

Run next only `WP-6-m2-pure-native-family-byte-contract-ledger`. Do not repeat
the 287 descriptor rows. At no more than 380 Stage 6/480 total documentation
lines, freeze every pure family’s complete converter, typed-value/equality, and
Java-renderer owner chain plus exact input/rejection and cache-byte
discriminators. In particular pin the versioned Java SE 21 `Enum#toString` and
`Duration#toString` API contracts versus Bazel's lowercase overrides; each
exact shared/custom converter class and value owner; full BoolOrEnum, sharding, fission,
and PlatformType branches; exact EnvVar records; exact default/mixed Duration
map text; executable UTF-16 ordering; and concrete null/empty/literal default
IDs. Any missing chain, semantic family split, live-JVM need, or new material
correction is `REPLAN`. Only after independent acceptance may a later packet
mechanically map the 287 already classified rows. Rust and every contextual,
regex, command, normalizer, checksum, wire, DICE, and configured-cycle path
remain deferred.

### Pure native family byte contract REPLAN (2026-08-04)

`WP-6-m2-pure-native-family-byte-contract-ledger` stopped under its explicit
missing-renderer-chain rule. Its compact draft still cited interfaces or vague
JDK behavior for Guava immutable lists/entries, record-generated EnvVar text,
and EnumMap/AbstractMap output. It also encoded timeout keys as uppercase, but
pinned `TestTimeout.java:159-162` overrides `toString()` to lowercase, so the
exact default map is `{short=PT1M, moderate=PT5M, long=PT15M, eternal=PT1H}`.
The unaccepted Stage 6 diff was discarded with `apply_patch`; HEAD returned
cleanly to `fd30a708`.

Run next only `WP-6-m2-java-guava-renderer-authority-evidence`. At no more than
240 Stage 6/300 total documentation lines, bind Bazel 9.2's actual Java runtime,
versioned Java API/spec renderers, pinned Guava `33.5.0-jre` plus JAR SHA-256
`1e301f0c52ac248b0b14fdc3d12283c77252d4d6f48521d572e7d8c4c2cc4ac7`,
and official Guava `v33.5.0` sources into one exact renderer matrix. A temporary
cleaned Java probe may pin record punctuation and standard collection/map/
duration bytes; it is evidence only and authorizes no Java/JVM dependency in
Slug. Freeze exact EnvVar, list/entry, enum, lowercase timeout map, duration,
UTF-16, and outer escaping bytes. Any source/JAR binding failure, runtime
disagreement, persistent fixture need, or production-JVM implication is
`REPLAN`. Descriptor/family conversion grammar, Rust, contextual/regex paths,
normalization, checksum, wire, DICE, and configured-cycle semantics remain
deferred.

### Rust-only semantic configuration identity direction reset (2026-08-08)

Explicit user direction supersedes the terminal exact-JDK/exact-hash scheduling
assumption. Slug will not embed or launch a JVM, ship or interpret Java bytecode
for its semantics, use a Java helper, or delegate production behavior to Bazel
or Java. Pinned Bazel remains an external oracle only. This prohibition is
permanent architecture, not a packet stop that a later agent may retry.

Bazel 9 remains the semantic reference, but affected surfaces now carry one of
three explicit compatibility classes: exact, Slug-native, or unsupported/
deferred. Rust-native OS, architecture, home, processor, memory, and container
observations replace bitwise HotSpot state. Valid-Unicode Rust strings and a
selected Rust regex contract replace Java UTF-16 lone-surrogate and `Pattern`
idiosyncrasies. Exact Bazel configuration checksum, configured-output-directory,
and ActionKey bytes move to M9 for later source-level algorithm analysis and a
Rust-only reproduction attempt.

The relaxation applies only to identity spelling and the named Host/regex edge
domain. Labels, targets, option behavior inside the admitted domain, structural
configuration partitioning, transitions, platforms/toolchains, providers,
actions, failures outside named divergences, artifact relative names/types/
modes/symlinks/content, lifecycle, and invalidation remain exact. REAPI/CAS,
content, repository, and lockfile digests remain exact for Slug's actual graph.
They are never normalized merely because they are hashes.

#### Semantic identity firewall

No provisional digest may become semantic truth. Complete typed admitted
configuration inputs—including native and Starlark values/scopes, repository/
platform/transition inputs, and relevant Rust-native Host facts—must own
structural equality and DICE invalidation. An unmodeled configuration-affecting
input fails before analysis; it may not alias the default. The current
caller-supplied `ConfigurationChecksum`/`first-build` carrier is quarantined to
existing bounded slices and gains no new caller.

Keep five domains separate:

1. structural Slug semantic configuration identity;
2. a collision-safe, domain/version-namespaced display and configured-path
   projection derived from that structure;
3. deferred Bazel `BuildOptions` checksum bytes;
4. deferred Bazel ActionKey bytes; and
5. exact REAPI/CAS content digests over Slug's action graph.

A display/path projection is never the sole DICE, action, AC, or cache key. It
must not use Rust `DefaultHasher`, truncate to a Bazel-looking seven-character
token, reuse an output path, or alias a REAPI digest. `cquery`/`aquery` comparison
may normalize only configuration IDs, configured-path configuration segments,
and ActionKey opaque identifiers to graph-scoped tokens while preserving their
equality/change/restoration relationships. It may not normalize arguments,
environment, mnemonics, owner labels, selected platforms, topology, relative
artifact names, contents, order, or failures.

Run next only `WP-6-m2-slug-native-configuration-identity-boundary-design`,
documentation/source-audit only. Inventory every live consumer of
`ConfigurationChecksum`/`ConfigurationKey` stable serialization,
`ConfiguredTargetKey`/DICE equality, configured layout/path construction,
action ownership, cquery/aquery output, and REAPI AC/CAS. Freeze the five-domain
ownership and no-new-placeholder-caller rules, every admitted structural input,
Host lifetime/injection, fail-closed unsupported-input behavior, projection
syntax/versioning/collision requirements, and semantic C0 -> C1 -> C0 evidence.

The design must schedule one observable successor,
`WP-6-m2-slug-native-default-configuration-vertical`, that replaces production
`first-build` end-to-end for the admitted no-argument/default configuration and
accepted root string transition, supplies one Rust-native process Host source,
uses structural typed defaults, and exposes an unmistakably Slug-native
display/path identity. It may not create dormant key/value substrate. Independent
identity/cache review precedes implementation.

This design authorizes no Rust, Cargo/dependency, hashing algorithm, new DICE
key, wire, configured output, aquery, action/cache activation, fixture, oracle,
JVM/Java artifact, or semantic relaxation beyond the explicit decision. Owner
documentation is capped at 260 changed lines; terminal scheduling may change
the current manifest and at most 15 canonical lines. Stop on digest-only
equality, silent flag omission, a Bazel-looking Slug token, content/graph
normalization, Java/JVM use, or an implementation successor that cannot remove
the production placeholder across one-shot and daemon paths.

### Slug-native configuration identity boundary design (2026-08-08)

`WP-6-m2-slug-native-configuration-identity-boundary-design` is `ACCEPT`.
The source audit found no existing production output-layout consumer to preserve:
`BazelLayout::{bazel_out,bin_dir,testlogs_dir}` is exercised only by identity
crate tests. Current execution instead materializes every configuration into
workspace `bazel-bin`. The first configured projection can therefore enter as
an explicitly Slug-native contract without rewriting a hidden Bazel checksum
consumer.

#### Live owner and consumer inventory

| Surface | Live owner/consumer | Present identity behavior | Required boundary |
|---|---|---|---|
| Configuration carrier | `slug_analysis_v2/src/key.rs:42-145` | `ConfigurationKey` derives structural `Eq`/`Hash` over kind, caller checksum, and optional root string; its display omits the root string | Replace the public opaque-checksum constructor in the production vertical; structural fields, not display bytes, remain semantic truth |
| Configured graph | `slug_analysis_v2/src/dice.rs:108-190,261-390,470-503,623-628,940-993,1098-1107` | Full `ConfiguredTargetKey` participates in DICE keys, dependency dedup/join, transitions, toolchain implementation analysis, and root-setting resolution | Preserve full structural equality through every recursive edge; never key joins or DICE by a projection |
| Build roots | `slug_core_v2/src/runtime/dice.rs:1519-1524,1626-1657,1938-2053,2056-2169` | `BuildCommandRootKey`, BFS seen/frontier sets, action closure, and action-result ownership retain the configuration/key structurally | Assemble one configuration before the root; action ownership remains target/configuration structural identity |
| Placeholder constructors | `slug_core_v2/src/runtime/dice.rs:2867-2874,2912-2923,3364-3373`; `slug_analysis_v2/src/dice.rs:1098-1107` | Three production entry paths and the accepted root-setting resolver manufacture `target:first-build` | The vertical removes all production `first-build`; no replacement caller may supply opaque identity text |
| Presentation | `slug_analysis_v2/src/key.rs:138-145,170-182`; `slug_core_v2/src/runtime/dice.rs:5417-5421` | Stable text is used by `Display` and one activation assertion; it is not a cache key | Display includes the Slug namespace and distinguishes every structural C0/C1 value |
| Layout/materialization | `slug_identity_v2/src/layout.rs:48-58`; `slug_cli_v2/src/commands/build.rs:183-204`; `slug_server_v2/src/reapi.rs:39-60` | Configured layout methods are test-only; one-shot and daemon materialize under workspace `bazel-bin` | Introduce one shared configured output owner; display and filesystem encodings remain separate typed projections of the same structure |
| cquery/aquery | `slug_core_v2/src/runtime/dice.rs:2889-2935`; `slug_cli_v2/src/commands/aquery.rs:12-15`; `slug_commands_v2/src/aquery.rs:18-34` | cquery retains a full configured key but prints only the requested label; aquery is still an explicit placeholder | Preserve accepted cquery bytes; use internal or separately admitted projection evidence, and leave aquery activation for later |
| Actions/platform | `slug_analysis_v2/src/result.rs:49-109`; `slug_analysis_v2/src/starlark_rule.rs:98-110`; `slug_analysis_v2/src/dice.rs:940-993`; `slug_build_api_v2/src/actions/spec.rs:126-220` | Analysis owns actions under a configured key, but `PreparedToolchain` drops selected execution platform before `ActionSpec` | Do not synthesize ActionKey or platform identity here; later aquery work must retain platform structurally before projecting it |
| REAPI/CAS/AC | `slug_reapi_v2/src/command.rs:119-170`; `slug_reapi_v2/src/executor.rs:93-125,181-210,240-258`; `slug_reapi_v2/src/action_cache.rs:92-146` | Command, input-root, Action proto, uploads, AC lookup, and results use exact SHA-256 content digests | Remains an independent exact domain; no configuration token enters AC/CAS except through actual action fields/paths |
| Host source | `slug_core_v2/src/runtime/process_host.rs:345-459`; `slug_core_v2/src/runtime/mod.rs:49-68`; `slug_server_v2/src/lib.rs:52-69` | A process owner has lazy OS/CPU/resource/home timing but all one-shot and daemon constructors install `UnsupportedSource`; `HostConversionInputs` is otherwise producer-free | Install one Rust source at process/daemon construction and project only demanded typed facts into configuration assembly |
| Command flags | `slug_commands_v2/src/build.rs:23-48`; `slug_commands_v2/src/common.rs:28-41,173-195,367-413` | All flags are retained, but unsupported/planned configuration flags can pass while production builds the same placeholder | Explicitly classify admitted nonconfiguration controls; reject every other configuration-affecting flag before root construction |

`ConfigurationChecksum` and `ConfigurationKey::target/exec/host_like` may remain
temporarily for tests and bounded nonproduction scaffolding, but production gets
no new caller. `stable_serialize` is presentation only. `ConfiguredTargetKey`,
DICE keys, `SmallSet`/`SmallMap` graph ownership, and transition restoration
continue to compare the complete structural value.

Dependency direction is fixed: public structural/default/projection types live
in `slug_configuration_v2`; `slug_analysis_v2` may depend downward on that crate
and embed the value; `slug_core_v2` supplies Host/request inputs. Configuration
must never depend on core, commands, workspace IO, or DICE. Existing private
default values are not yet a public hashable aggregate, so the successor must
close that representation rather than wrapping the old checksum string.

#### Five-domain ownership

1. `SlugConfiguration` is the sole semantic configuration value. For this
   vertical it contains target kind, the complete typed native default values
   admitted by the default kernel, and the optional typed root string setting.
   Equality/hash/ordering and DICE invalidation use this structure directly.
2. `SlugConfigurationProjection` is derived presentation. Display syntax is
   `slugcfg-v1:<opaque>` and the filesystem segment is
   `slugcfg-v1-<opaque>`. `<opaque>` is deterministic lowercase ASCII with at
   least 256-bit collision resistance over a tagged, length-delimited,
   versioned serialization. It is never truncated, parsed back as semantics,
   or accepted as caller input. Algorithm selection and implementation receive
   the required independent identity/cache review in the successor.
3. Bazel `BuildOptions` checksum text is unsupported/deferred to M9. Slug does
   not label its projection a Bazel checksum or imitate Bazel's short token.
4. Bazel ActionKey bytes are unsupported/deferred to M9. A later Slug action
   identity must include complete action/platform ownership and cannot reuse
   the configuration projection.
5. REAPI Command, Directory/input-root, Action, CAS, and AC digests remain exact
   hashes of their protobuf/blob bytes. Their existing owners do not consume
   `SlugConfigurationProjection` as a cache key.

The filesystem projection is collision-safe spelling, not semantic ownership.
Two unequal structures must not alias a configured directory; detection of an
impossible projection collision is a hard infrastructure failure. cquery/aquery
comparators may replace only display/path/ActionKey opaque IDs with graph-local
tokens after recording equality classes. They may not normalize any graph,
action, platform, path-relative, content, ordering, or failure field.

#### Admitted default vertical

The successor admits only target configuration constructed from the complete
typed default native-option set already supported by the native kernel, plus
the existing root string build setting and its one-output transition. Target
labels and Bzlmod command/environment/lockfile/registry inputs keep their
existing exact owners; they are not configuration merely because they share a
request. UI, `output_base`, BEP, and remote transport/execution controls are
likewise operational, not semantic configuration.

Every build/cquery request passes its complete parsed flag list to a single
configuration assembler. That assembler uses an explicit allowlist for the
above nonconfiguration controls and rejects any other explicit configuration-
affecting flag as unsupported before `BuildCommandRootKey` or cquery analysis
is constructed. In particular, `--config` and unknown/planned native options
must not silently select C0. No absent descriptor, converter failure, Host read
failure, unmodeled Starlark setting, transition output, platform, or repository
input may fall back to C0.

The existing cquery parser's explicit allowlist remains authoritative; the new
build classification must reach equivalent fail-closed behavior without
rejecting admitted Bzlmod/UI/output-base/BEP/remote operational controls merely
because the current broad disposition calls them parse-only or planned.

One Rust-native `ProcessHostSource` is installed when a one-shot process or
daemon is constructed. OS/architecture and container-aware available processor
and memory limits are process-latched; home is read at each eligible conversion
as required by the retained lifecycle. Configuration owns only the typed Host-
dependent default outcomes actually demanded by conversion (auto CPU, path
flavor, capacity, and eligible home/path outcomes), not a pointer to the source
or unused raw facts. Valid Rust Unicode replaces lone-surrogate semantics.
Source errors are explicit unsupported/infrastructure results, never identity
defaults. No lock is held across DICE computation.

#### Successor and acceptance evidence

Run next only `WP-6-m2-slug-native-default-configuration-vertical`. It is one
observable implementation because a schema-only or producer-only packet would
leave the placeholder authoritative. Before code, a reserved independent
identity/cache reviewer must accept the concrete tagged serialization,
collision behavior, dependency direction, and proof that REAPI digests remain
content-derived.

The implementation must, in one bounded change:

- make the typed structural configuration and projection V2-owned, with retained
  Arc-backed values and no `DefaultHasher`, path-as-key, or digest-only equality;
- install the Rust Host source in one-shot and daemon owners, assemble complete
  admitted defaults once per request, and pass the same structural value into
  build, cquery, recursive dependencies, transitions, toolchains, and actions;
- replace `UnsupportedSource` in every production runtime constructor, including
  legacy public wrappers, while preserving one Arc owner per process/daemon;
- delete every production `"first-build"` construction and make legacy
  `evaluate_workspace_targets*` either use the same assembler or fail as an
  unsupported legacy route—never manufacture another placeholder;
- route configured artifacts/materialization through one
  `bazel-out/slugcfg-v1-<opaque>/bin` owner while keeping relative action paths,
  contents, modes, symlinks, and REAPI digest computation unchanged;
- expose `slugcfg-v1:<opaque>` through internal/display assertions or a
  separately admitted new surface while preserving the accepted
  `str(target.label)` cquery output byte-for-byte; keep aquery/ActionKey
  activation out of scope; and
- reject unsupported explicit configuration inputs with stable diagnostics in
  both one-shot and daemon modes.

Evidence is structural C0 -> C1 -> C0 in one retained daemon and equivalent
one-shot commands. C0 is default; C1 changes only the accepted root string
setting. The proof must show C0 equality and projection restoration, C1
inequality, DICE recomputation/pruning at the correct configured nodes, distinct
configured output directories, unchanged labels/providers/action topology,
and changed action/REAPI bytes only when the configuration changes an actual
action field or configured path. It must also show identical one-shot/daemon
configuration projections for identical typed inputs, Host-source process
reuse versus per-eligible home reads, and pre-analysis rejection of `--config`
plus one unknown configuration flag.

Stop and `REPLAN` on a crate dependency cycle, partial default descriptor set,
new opaque-checksum caller, source/Host IO inside configuration conversion,
digest-only DICE identity, shortened or Bazel-looking token, configured-path
alias, ActionKey/aquery invention, REAPI digest normalization, Java/JVM artifact
or execution, or an implementation that leaves any production `first-build`.

### Slug-native default configuration vertical acceptance (2026-08-09)

`WP-6-m2-slug-native-default-configuration-vertical` is **ACCEPT**. The
implementation replaces production placeholder identity with the complete
341-option Rust-native structural target configuration, a frozen full-width
namespaced projection, Rust Host inputs, root string-setting defaults and the
admitted one-output transition. Structural keys remain authoritative; atomic
sidecars and an in-memory registry protect configured output spelling; REAPI
digest construction remains independent. Both native-demand and retained
snapshot analysis routes resolve and propagate the actual structural child
configuration. Build flags fail closed before mode routing.

Independent identity and semantic reviews accepted the final diff. Focused
configuration, analysis, command, server, CLI, Host, transition, collision, and
materialization tests pass, and `slug_cli_v2` rebuilds locally. The two broad
suite failures recorded in the packet manifest are independently reproduced
pre-existing failures outside this slice. BuildBuddy RC discovery/authentication
works without exposing the private user RC; the full Bazel developer target is
still stopped by the known missing `rules_rust` toolchain.

M2 structural identity is accepted. Run next only
`WP-6-m4-root-cquery-label-slug-projection-implementation`, the bounded Rust
implementation of the independently accepted public-format contract below.
Keep the Bazel seven-hex checksum and exact identity in M9, preserve the
accepted `str(target.label)` bytes, and do not broaden cquery beyond the frozen
one-root grammar.

### Root cquery Slug-projection public-format design result (2026-08-09)

`WP-6-m4-root-cquery-label-slug-projection-design` is **ACCEPTED**. The required
independent public-format/identity review returned `ACCEPT` with no blockers:
the grammar and public bytes are bounded, structural configuration remains the
DICE identity while its full-width projection is presentation only, one-shot
and daemon semantics agree, and comparison normalization is confined to the
opaque payload. The changed surface is explicitly **Slug-native** only where
the configuration token bytes diverge; the root apparent-label spelling and
already accepted terminal/error behavior remain exact for their named slices.
Exact Bazel checksum/short-ID bytes remain M9, and every other cquery form
remains unsupported.

#### Accepted command grammar and exact output

The successor admits exactly these formatter forms for one parsed root literal
target:

1. `slug cquery //pkg:target`
2. `slug cquery //pkg:target --output=label`
3. `slug cquery //pkg:target --output=starlark
   --starlark:expr=str(target.label)`

The first two forms select one `Label` mode and are byte-identical. Success is
exactly
`//pkg:target (slugcfg-v1:<64-lowercase-hex-bytes>)\n`, using the normalized
root apparent label retained from the parsed request, the literal parentheses
and space shown, the exact `slugcfg-v1:` namespace/version prefix, and the full
64-character projection payload. It never prints internal `@@` spelling in
this mode. The payload comes only from the returned `AnalysisResult`'s
structural `SlugConfiguration`; it is never caller input, parsed back,
truncated, or used as DICE/cache/action identity.

The third form remains a separate `StarlarkLabel` mode and preserves the
already accepted stdout byte-for-byte: `@@//pkg:target\n`. It never gains a
configuration suffix. `--starlark:expr` is legal only with
`--output=starlark` and only for the exact existing expression. Explicit
`--output=label` rejects every Starlark expression; omitted output plus a
Starlark expression is also rejected. Any other output, expression, duplicate
output/expression, passthrough, target cardinality/pattern/repository form, or
unknown flag fails during command parsing before one-shot/daemon selection.

Both modes additionally admit exactly `--//:setting=<Unicode>`, including the
empty string after `=`, with ordinary last-occurrence-wins flag semantics. The
form without `=` fails with the same `expected --//:setting=<Unicode>`
diagnostic as build. No other configuration flag is admitted. Existing
Bzlmod and `--output_base` controls retain their current nonconfiguration
owners and allowlist.

#### Structural and incremental ownership

No new evaluator or DICE key is introduced. The existing `CqueryCommandRoot`
continues to consume `RootConfiguredTargetAnalysisKey`. It constructs one
native base configuration, clones it with the optional command-line root
setting, and uses the existing root-setting request route whenever the flag is
present. Default resolution for a setting/consumer/transition continues
through the accepted DICE-owned self-routing path. The root retains the
canonical requested label separately so missing-target translation never calls
`configured_target()` on an unresolved setting request.

`CqueryCommandEvaluation` retains the successful analysis plus the normalized
root apparent display label and the full `SlugConfigurationProjection` derived
from the returned analysis key. Construction fails as infrastructure if a
production result contains an opaque/legacy configuration. Request-known
configurations are collision-claimed before DICE; any default or transition
configuration returned by DICE is claimed before publication. Formatting is a
pure choice over the accepted terminal and does not recompute analysis.

In one retained daemon, C0 -> explicit C1 -> C0 for the same target must show
distinct then exactly restored full label-mode bytes and structural projection,
zero source invalidations, cold C0/C1 analysis followed by warm restored-C0
reuse, and unchanged label/provider/action topology. The same typed C0 inputs
must produce byte-identical one-shot and daemon label output. Across C0/C1/C0,
Starlark-label stdout remains byte-identical because its contract observes only
the canonical label.

#### Wire, terminals, and comparison

The daemon request gains a required, serde-validated two-case discriminator
`label | starlark_label` and `root_string_setting: Option<String>`. The CLI
always sends the selected discriminator explicitly; default and explicit
`--output=label` both send `label`. There is no compatibility shim for the old
prototype request shape. Unknown discriminator values and malformed targets
fail before workspace observation or analysis with the existing structured
cquery request/transport exit-2 convention. The server never reparses a raw
formatter string after the enum is decoded.

Missing targets in either mode preserve exit 1, empty stdout, and the exact
accepted three diagnostic lines. Unsupported CLI forms remain the existing
`command_parse_error` exit 2 before mode routing. Non-missing analysis and
infrastructure failures retain the existing one-shot/daemon cquery JSON
families and runtime-mode/invalidated-file ownership; adding a formatter does
not translate or normalize those errors.

A graph comparator may replace only the 64 lowercase payload following an
exact `slugcfg-v1:` prefix with graph-local equality-class names. The prefix,
label, punctuation, graph, providers, actions, platforms, paths, content,
ordering, exit status, diagnostics, and all other fields remain literal. Equal
structural configurations must map to one class; unequal structures must never
be normalized together.

#### Bounded successor

The accepted design schedules only
`WP-6-m4-root-cquery-label-slug-projection-implementation`. Production edits
are limited to:

- `app/slug_commands_v2/src/cquery.rs`;
- `app/slug_core_v2/src/runtime/dice.rs` and `runtime/mod.rs`;
- `app/slug_cli_v2/src/commands/cquery.rs`;
- `app/slug_server_v2/src/lib.rs` and `src/server.rs`.

Tests are limited to `app/slug_commands_v2/tests/commands.rs`, focused runtime
tests in `app/slug_core_v2/src/runtime/dice.rs`,
`app/slug_cli_v2/tests/cli.rs`, and `app/slug_server_v2/src/tests.rs`. Reuse the
accepted Bazel 9.2 label-layout/missing/warm evidence; no new oracle fixture or
command is needed. Validate parser matrices, exact mode bytes, malformed wire,
direct setting/default/transition resolution, retained-daemon C0/C1/C0 and
recomputation, one-shot/daemon equality, and unchanged Starlark/missing
terminals. Build `slug_cli_v2` before daemon-sensitive CLI tests and clean stale
`slugd` processes before and after them.

The successor must stop and `REPLAN` on a second graph/key, any Bazel-looking
short-ID approximation, truncated/caller-supplied/projection-as-identity use,
changed Starlark bytes, general Starlark evaluation, aquery/ActionKey/platform
breadth, normalization outside the payload, or any JVM/Java artifact,
execution, helper, or delegation.

### Root cquery Slug-projection implementation ACCEPT (2026-08-09)

`WP-6-m4-root-cquery-label-slug-projection-implementation` is **ACCEPT**.
Default cquery and explicit `--output=label` now publish the apparent root
label followed by the full returned structural configuration projection as
`slugcfg-v1:<64-lowercase-hex>`. The existing exact
`--output=starlark --starlark:expr=str(target.label)` bytes remain unchanged.
The command parser admits only the frozen formatter matrix and the one root
Unicode string setting; the setting has last-occurrence-wins semantics and is
forwarded identically through one-shot and daemon execution.

The retained `RootConfiguredTargetAnalysisKey` remains the only analysis root.
Explicit settings use its existing root-setting request form, default and
transition resolution remain DICE-owned, and missing-target translation now
uses a separately retained canonical label rather than resolving an unfinished
request key. Successful evaluation derives and retains its display projection
only from the returned `AnalysisResult`; request-known and returned structural
configurations are collision-claimed before publication. The projection is
never parsed, truncated, supplied by the caller, or used as semantic, DICE,
cache, or action identity.

The daemon wire has a required serde discriminator `label | starlark_label`
and the optional root setting, with no old-request compatibility shim.
Malformed modes fail before observation; missing and runtime terminals retain
their prior ownership. Tests prove exact parser and wire matrices, full token
shape, direct setting/default/transition resolution, explicit-setting missing
translation, unchanged Starlark output, one-shot/daemon equality, and retained
C0 -> C1 -> C0 distinct/restored bytes, topology, warm reuse, and zero source
invalidations.

Validation passed: command parser `18/18`, focused core cquery `2/2`, server
`40/40`, focused CLI cquery `3/3`, `slug_cli_v2` build, formatting, and diff
checks. No stale daemon remained before or after CLI validation. An independent
final implementation review returned `ACCEPT` with no findings. All nine
changed Rust/test files stay within the packet allowlist. Exact Bazel
configuration/output/ActionKey bytes remain deferred to the Rust-only M9
analysis; no JVM or Java artifact, helper, execution, or delegation was added.

M4 is now **partial/parked**, not milestone-complete. This packet closes the
bounded one-root configured-label surface and proves that it consumes the
shared structural analysis graph, including default, explicit, and transitioned
configuration ownership. Configured query expressions, dependency-graph
traversal, provider projection, and broader functions/formats remain separate
M4 work after the canonical M3 query gate advances.

### Function-free configured set algebra packet (2026-08-09)

M3 is accepted by the retained 18-lane Bazel 9.2 `attr()` oracle and runtime
activation. Reserved architecture review accepted
`WP-6-m4-root-cquery-set-algebra-implementation` as the next M4 packet. It
admits only root literal `set`, `let`, `union`, `intersect`, and `except` in
label mode, reusing the existing parser and one minimal generic ordered-set
evaluator. One `CqueryCommandRoot` computes ordered distinct roots through
`RootConfiguredTargetAnalysisKey`, unions compatible Needs, selects the first
lexical terminal error after preparation completes, and deduplicates by full
`ConfiguredTargetKey`.

The prototype wire field becomes `expression` with no compatibility alias.
Existing single-literal label/Starlark output, missing diagnostics, and
C0/C1/C0 ownership remain unchanged. `deps` is not admitted because fresh
Bazel 9.2 evidence exposes host-platform and constraint nodes outside the
current retained dependency graph. Functions, patterns, external repositories,
providers, new formats, exact Bazel configuration hashes, new graph/DICE keys,
and JVM/Java work remain excluded.

Implementation reached terminal `REPLAN` after its one allowed correction.
The correction removed a duplicated recursive evaluator and all focused tests
passed, but independent review plus a fresh Bazel 9.2 command proved `set()`
must succeed with empty stdout. The draft instead required a first analysis and
returned a request error; `let x = set() in $x` failed for the same reason.
Review also found undefined variables escaped request validation and failed
only after workspace observation. Run next only
`WP-6-m4-root-cquery-set-algebra-empty-retry`: retain the shared evaluator and
all accepted ordering/DICE/wire work, support honest zero-root results, and
validate undefined variables before observation. No other breadth changes.

### Function-free configured set algebra retry ACCEPT (2026-08-09)

`WP-6-m4-root-cquery-set-algebra-empty-retry` is **ACCEPT**. Configured query
now reuses one Buck2-derived recursive expression fold for root-repository
literal `set`, `let`, `union`, `intersect`, and `except` expressions. Ordered
results deduplicate by complete `ConfiguredTargetKey`; every distinct root is
resolved through the existing `RootConfiguredTargetAnalysisKey` in one command
transaction. No configured graph, DICE key, parser, or evaluator was added.

The retry makes `set()` and `let x = set() in $x` successful zero-target
commands with empty output. Only cquery may seal an empty terminal activation
closure; every other native command remains fail-closed. Function-free request
validation now enforces lexical binding, RHS visibility, and shadow restoration
before workspace observation. Single-literal Starlark output remains an honest
singleton-only boundary, and the daemon wire uses required `expression` with no
compatibility alias.

Focused validation passed 99 query tests, 18 command tests, three core cquery
tests, four server cquery tests, four rebuilt-CLI cquery tests, archive status,
formatting, and diff checks. Stale `slugd` processes were cleaned before and
after CLI validation. Independent retry review returned `ACCEPT`. Fresh Bazel
9.2 evidence pins the admitted ordering and empty-result semantics; `deps`
remains excluded because Bazel also exposes host-platform and constraint nodes
not retained by the current analysis dependency surface.

Run next only `WP-6-m4-configured-query-successor-audit`: inspect the retained
configured graph/provider surfaces and the Buck2-derived query evaluator to
select one exact, bounded M4 behavior that can be implemented without a second
graph, invented platform nodes, new parser/evaluator, exact-hash approximation,
or JVM/Java work. The audit must end in a functional implementation packet; do
not commit documentation by itself.

### Configured-query successor audit ACCEPT (2026-08-09)

`WP-6-m4-configured-query-successor-audit` selected
`WP-6-m4-cquery-starlark-label-set-output-implementation`. Extend only the
already accepted exact `--output=starlark
--starlark:expr=str(target.label)` formatter from one configured target to the
ordered result of the accepted function-free set evaluator. Fresh Bazel 9.2
commands prove a deduplicated two-target `set` and a `let`/binary expression
emit `@@//pkg:bin\n@@//pkg:lib\n`, while `set()` exits zero with empty stdout.

This removes a singleton-only formatter gate; it needs no new Starlark
expression/file/provider runtime, configured state, graph, key, parser, or
evaluator. Provider projection is not selected because the retained collection
does not yet close Bazel's qualified provider dictionary and builtin value
semantics. Configured `kind`/`attr` lack retained configured metadata, and
`deps` remains blocked by observable host-platform and constraint nodes.

Production edits are limited to cquery command parsing, its core evaluation
formatter, CLI routing only if required, and the cquery server mode validation.
Tests are limited to the corresponding command/core/server/CLI cquery suites.
Stop on any new Starlark expression, `--starlark:file`, provider projection,
target pattern/external repository, dependency traversal, output/wire mode,
graph/key/parser/evaluator, configuration-hash, JVM, or Java requirement.

### Configured-query Starlark-label set output ACCEPT (2026-08-09)

`WP-6-m4-cquery-starlark-label-set-output-implementation` is **ACCEPT**. The
exact existing `str(target.label)` formatter now maps across the accepted
ordered configured-target set. Multi-root `set` and `let`/binary expressions
emit canonical labels in first-insertion order with full configured-key
deduplication; `set()` succeeds with empty stdout. Existing single-target bytes,
label mode, missing-target terminals, and C0/C1/C0 ownership are unchanged.

The implementation removes only singleton admission/formatting checks and the
retired `QueryExpression::single_literal()` helper. It adds no Starlark
expression/file/provider runtime, output or wire mode, graph, DICE key, parser,
evaluator, dependency traversal, exact hash, JVM, or Java surface. Fresh Bazel
9.2 evidence pins the two-target and empty bytes.

Validation passed 99 query tests, 18 command tests, three core cquery tests,
five server cquery tests, four rebuilt-CLI cquery tests, formatting, archive
status, and diff checks. Stale `slugd` processes were absent before and after
CLI validation. Independent review requested only removal of the unused
singleton helper; correction rereview returned `ACCEPT`.

Run next only `WP-6-m4-configured-query-successor-audit-2`, again selecting one
semantically closed functional slice from retained configured analysis state
and the Buck2-derived evaluator. Do not select provider projection without a
complete qualified-provider/builtin-value boundary, configured `kind`/`attr`
without retained configured metadata, or `deps` without observable platform
and constraint nodes. Carry the audit record with its functional successor.

### Configured-query successor audit 2 ACCEPT (2026-08-09)

`WP-6-m4-configured-query-successor-audit-2` selected
`WP-6-m4-cquery-filter-label-rust-native-implementation`. Admit only
`filter(regex, expression)` over the accepted configured root/set forms. The
predicate candidate is the retained configured target key's original apparent
root label (`//pkg:target`), while Starlark-label output remains canonical
`@@//pkg:target`. Grammar, arity, unanchored find, left-to-right operand/set
semantics, ordering, and full configured-key deduplication are exact. Regex
syntax, valid-Unicode behavior, resource limits, and diagnostics reuse the
accepted M3 Rust-native contract; Java regex/UTF-16 parity is excluded by user
direction.

The implementation must reuse the sole Buck2-derived recursive fold and the
existing bounded regex compiler/filter primitive. Fresh Bazel 9.2 evidence
corrected the audit's initial lazy-resolution premise: `filter('(',
//pkg:missing)` reports the missing target, not the malformed regex. Keep the
existing eager lexical `RootConfiguredTargetAnalysisKey` universe, compatible
Needs union/restart, and first typed root error unchanged. Only after that
universe completes does the shared evaluator compile the regex before folding
its already-resolved operand. Add no DICE key, graph, cache, evaluator, parser,
or regex identity.

Before Rust acceptance, add focused Bazel 9.2 cquery evidence for anchored root
label matching with canonical Starlark output, ordered composition/deduplication,
and empty nonmatch. Tests must additionally prove missing-before-malformed-regex
ordering, validator-time unsupported-function failures, existing set and
C0/C1/C0 behavior, one-shot/daemon equality, and missing recovery. `kind`,
`attr`, `deps`, providers, patterns, externals, new output/wire modes, exact
hashes, JVM, and Java remain excluded.

### Configured label filter ACCEPT (2026-08-09)

`WP-6-m4-cquery-filter-label-rust-native-implementation` is **ACCEPT**.
Configured query now admits only recursive `filter(regex, expression)` in
addition to the accepted ordered set forms. It reuses the sole Buck2-derived
expression fold and the same bounded regex compiler/filter invocation as
loading query. The predicate sees the apparent root label, while result identity
remains the full configured key and Starlark-label output remains canonical.

Fresh four-lane Bazel 9.2 evidence pins anchored matching, insertion order and
deduplication, empty nonmatch, and missing-target precedence over malformed
regex. The existing eager root/Needs/typed-error loop is unchanged; regex
compilation happens only after the root universe completes. The implementation
adds no graph, key, cache, parser, recursive evaluator, configured metadata,
provider/dependency traversal, exact hash, JVM, or Java surface.

Validation passed the Bazel fixture update and clean replay, three oracle
integrity tests, 100 query tests, 18 command tests, three core cquery tests,
five server cquery tests, five rebuilt-CLI cquery tests, formatting, archive
status, and diff checks. Stale `slugd` processes were absent after validation.
Independent review returned `ACCEPT` with no findings; the direct core `regex`
dependency is required by the typed configured-query environment boundary.

Run next only `WP-6-m4-configured-query-successor-audit-3`. Select one bounded
functional behavior supported by retained configured state and the shared
evaluator; carry its bookkeeping with implementation rather than committing
documentation alone. Keep `deps`, configured `kind`/`attr`, and provider
projection behind their already recorded missing-state boundaries.

### Configured-query successor audit 3 ACCEPT (2026-08-09)

`WP-6-m4-configured-query-successor-audit-3` selected
`WP-6-m4-cquery-some-selection-implementation`. Admit the complete existing
`some(expr[, count])` signature over the accepted configured literal/set/filter
language. Omitted count is one; the optional count uses the existing signed
`i32` validation seam. Positive counts return an arbitrary distinct subset up
to the requested size, counts at or above cardinality return all, and an empty
selection (empty input or nonpositive count) fails with `argument set is
empty`.

Reuse the static `some` function specification, `QuerySelectionCount`, sole
Buck2-derived recursive fold, and ordered configured `TargetSet`. Slug's
deterministic first-insertion subset is one valid witness of Bazel's explicitly
arbitrary choice; it must never be described as Bazel's selected label.
Validation errors precede literal preparation. Otherwise preserve the eager
root/Needs universe, so all valid operand literals resolve before selection.
Add no graph, key, parser, evaluator, traversal, metadata, provider, pattern,
external repository, output/wire mode, exact hash, JVM, or Java surface.

Fresh Bazel 9.2 cquery evidence must use fully anchored finite alternatives for
one- and two-member arbitrary subsets rather than pinning a winner. Cover
omitted/positive/at-cardinality/oversized counts, composition/dedup/filter,
zero/negative/empty failures, signed-i32 boundaries, invalid-count-before-
missing validation, and early-valid/later-missing eager resolution. Stop and
`REPLAN` on an eager-universe contradiction or a terminal requiring new wire
ownership. `executables` remains blocked because Bazel reads underlying Rule
capability, not retained `DefaultInfo` provider state.

### Configured `some` evaluation-terminal REPLAN (2026-08-09)

`WP-6-m4-cquery-some-selection-implementation` reached its explicit terminal
stop after fresh Bazel 9.2 evidence. Twelve oracle rows pass update and clean
replay, and the shared evaluator draft implements the complete optional-count
surface without graph or DICE changes. However `some(//pkg:alpha, '-1')` and
other valid expressions whose configured evaluation fails exit 1 in Bazel.
Slug currently maps every non-missing `CqueryCommandError` terminal to the
request/runtime JSON family with exit 2.

This is not an evaluator or parser correction. It requires a distinct typed
configured-evaluation terminal owned consistently by core, one-shot CLI, and
daemon publication. The initial packet is therefore **REPLAN** before accepting
its otherwise green draft. Preserve request-validation exit 2, missing-target
exit 1 with its accepted diagnostics, analysis/infrastructure ownership, empty
stdout, invalidated-file accounting, and one-shot/daemon JSON family unless
Bazel 9.2 evidence requires a narrower change.

Run next only `WP-6-m4-cquery-evaluation-terminal-ownership-design`. Review the
minimal new core variant/classification and both publishers, then schedule a
narrow retry using the retained `some` draft and 12-row fixture. Add no query
function, graph/key, output/wire request field, exact hash, JVM, or Java work.

### Configured evaluation terminal design ACCEPT (2026-08-09)

Reserved review accepted a single new
`CqueryCommandError::Evaluation(Arc<str>)` classification. A narrow public
`QueryError::is_evaluation_failure()` identifies only the existing evaluation
kind; post-preparation cquery evaluator errors use one constructor that maps
that kind to `Evaluation` and retains all other kinds, including bounded-regex
syntax, as `Request`. Core `exit_code()` returns 1 only for `MissingTarget` and
`Evaluation`, and 2 for `Request`, `Analysis`, and `Infrastructure`.

`missing_stderr()` remains exclusive to `MissingTarget`; Evaluation Display is
its message. One-shot and daemon keep `cquery_runtime_error`, empty stdout,
runtime mode, and invalidated-file accounting. Both publishers must use the
core exit-code accessor rather than infer classification. Analysis and
infrastructure remain exit 2 absent discriminating evidence.

Run next only `WP-6-m4-cquery-some-selection-evaluation-terminal-retry` using
the retained `some` draft and 12-row fixture. Add core classification tests,
one-shot/daemon evaluation-exit-1 coverage, pre-observation invalid-count exit
2, invalidation preservation, and unchanged missing diagnostics. Stop on wire
or JSON-kind changes, broad QueryError exit-1 mapping, analysis/infrastructure
reclassification, root/Needs changes, nonempty failure stdout, or function
breadth.

### Configured `some` selection retry ACCEPT (2026-08-09)

`WP-6-m4-cquery-some-selection-evaluation-terminal-retry` is **ACCEPT**.
Configured query now supports the complete `some(expr[, count])` signature
over its admitted literal/set/filter language through the shared recursive
fold. Signed-i32 validation, default count one, arbitrary distinct subset,
full-key deduplication, eager root preparation, and empty/nonpositive failure
semantics match the accepted source and 12-row Bazel 9.2 cquery fixture.

Only post-preparation `QueryError` evaluation failures become the new typed
`CqueryCommandError::Evaluation` and exit 1. Syntax/request, analysis, and
infrastructure remain exit 2; missing targets retain exact exit-1 diagnostics.
One-shot and daemon use the core classification while preserving empty stdout,
the `cquery_runtime_error` JSON family, runtime mode, request wire, and daemon
invalidated-file counts. No graph, key, parser, second evaluator, traversal,
metadata/provider, hash, JVM, or Java surface was added.

Validation passed 12/12 Bazel replay rows, 102 query tests, 18 command tests,
four core cquery tests, six server cquery tests, five rebuilt-CLI cquery tests,
three oracle integrity tests, formatting, archive status, and diff checks.
Stale `slugd` processes were absent before and after CLI validation.
Independent final review returned `ACCEPT` with no findings.

Run next only `WP-6-m4-configured-query-successor-audit-4`, selecting another
semantically closed behavior from retained configured state. Keep traversal,
configured metadata/provider projection, patterns/externals, exact hashes,
JVM, and Java behind their existing boundaries; bundle audit bookkeeping with
its functional successor.

### Configured-query successor audit 4 REPLAN (2026-08-09)

`WP-6-m4-configured-query-successor-audit-4` is **REPLAN**: no additional exact
function is closed over the current `CqueryResultTarget`/`AnalysisResult`
surface. Traversal lacks observable platform/constraint nodes; `kind` and
`attr` lack configured target metadata; provider projection lacks a complete
qualified dictionary/value runtime. A smaller substitute would preserve the
wrong end state.

The first truthful prerequisite is configured rule capability for
`executables(expr)`. Bazel uses the underlying Rule's executable/non-test
predicate, not `DefaultInfo`. Loading already owns the accepted immutable
`RuleCapability { rule_class: CompactString, executable, test_kind }` with
export identity and invalidation coverage. Attach `Option<RuleCapability>` to
`AnalysisResult` during analysis; cquery must borrow it from the retained result
and never reload packages or store a command-local duplicate. The complete
value participates in `AnalysisResult` equality and therefore existing DICE
reuse/invalidation. Use the existing compact Allocative value; add no interner,
owned string clone, map, graph, key, cache, or lock.

Run next only `WP-6-m4-configured-rule-capability-attachment-design` with
reserved review. Freeze construction ownership for every currently analyzable
rule/non-rule path, equality/restoration evidence, and the immediately following
complete `executables(expr)` implementation. `kind` remains later because its
target-kind domain includes non-rule/generated forms; `attr`, traversal, and
providers remain deferred.

### Configured rule-capability attachment design ACCEPT (2026-08-09)

Reserved review accepted retaining `slug_loading_v2::RuleCapability` in its
current owner; analysis already depends on loading, so no cycle or type move is
needed. Add `Option<RuleCapability>` directly to `AnalysisResult` and require it
in `AnalysisResult::new`, preventing production omission. Expose a borrowed
accessor. Existing derived Clone/Eq/Allocative covers the full compact
rule-class/executable/test-kind value; clone it once from the already located
`PackageTarget`, with no Arc, interner, owned string, or command-local copy.

`evaluate_loaded_rule` is the sole production construction boundary and passes
`target.rule_capability().cloned()`. Existing package dependencies invalidate
analysis on capability changes; full `AnalysisResult` equality owns reuse and
restoration without a new key. Unsupported non-rules remain unsupported;
constructor/unit paths may pass `None`.

Activate the complete existing `executables(expr)` signature through the sole
recursive fold. Share one invocation between loading and configured contexts;
configured filtering borrows `target.analysis.rule_capability()` and applies
exactly `executable && !rule_class.ends_with("_test")`. Do not substitute
`test_kind`; it remains equality state. Add a focused cquery fixture covering
positive/negative/executable `_test`/target-name `_test`, order/dedupe,
composition, empty, both outputs, missing, and arity. Same-daemon tests cover
false -> true -> exported `_test` -> restored false with recomputation/reuse.

Run next `WP-6-m4-cquery-executables-rule-capability-implementation`. Stop on a
dependency cycle/type relocation, optional production omission, package reload
or command-local metadata, test-kind predicate, native-analysis widening, new
graph/key/cache, patterns/externals, or failed equality/restoration evidence.

### Configured executables oracle REPLAN (2026-08-09)

**Status: REPLAN to a bounded analysis prerequisite.** Bazel 9.2 rejects a
positive executable Starlark rule unless it returns
`DefaultInfo(executable = <File>)`; Slug analysis currently admits only the
`files` field. The retained capability/query implementation passed focused
analysis, query, core, server, and CLI tests, but the required positive oracle
could not succeed without crossing the packet's provider-surface stop. No
query behavior is accepted from that draft.

Review accepted a serial non-test prerequisite using the existing build-api
`DefaultInfo` representation. Extend the analysis-only Starlark value with
optional `files` and `executable`; reject other fields and require executable
to be a declared file owned by the current evaluation. Explicit files override
the implicit singleton executable files set. Populate existing executable,
files-to-run executable, and default/data runfiles fields, and reject an
executable/test-capability rule that returns no executable. Retain the already
reviewed `RuleCapability` attachment because it overlaps this sole production
constructor and is independently exact.

Do not add general runfiles, `ctx.runfiles`, predeclared executables, foreign or
dependency files, execution/materialization, provider dictionary breadth, or a
new key/cache. Exact configured test-rule success remains deferred because
Bazel additionally requires test runfiles; the accepted loading-query oracle
continues to pin the exported `_test` predicate without claiming configured
test analysis.

Run next `WP-6-m4-default-info-executable-analysis-prerequisite`. Keep the
query/core activation draft out of its commit. After acceptance, resume the
non-test configured `executables(expr)` slice and decide separately whether a
runfiles prerequisite is warranted for live configured test-rule coverage.

### DefaultInfo executable analysis prerequisite ACCEPT (2026-08-09)

The bounded prerequisite is **ACCEPTED**. Analysis-only `DefaultInfo` now
accepts only optional `files` and `executable`, normalizes omitted/`None`, and
decodes only current-evaluation declared files. The existing build-api value
owns Bazel's implicit executable singleton, explicit-files override,
files-to-run executable, and default/data runfiles projection. Executable rule
capability without a returned executable fails with the pinned Bazel 9.2
diagnostic. General runfiles/provider fields remain unavailable.

`AnalysisResult` now requires and structurally retains the complete loading-
owned `Option<RuleCapability>` at its sole production rule-analysis boundary.
No new DICE key/cache, graph, interner, or query-time package lookup was added.
Direct equality and same-DICE edit/restoration tests cover capability and
provider changes. The corrected 12-row Bazel fixture update/replay, focused and
full build-api/loading/analysis suites, payload integrity, format, archive, and
diff checks pass. Independent review returned `ACCEPT`.

The fixture's missing-executable diagnostic is exact, but the existing cquery
publisher maps that analysis failure to exit 2 instead of Bazel's exit 1. This
was not widened into the prerequisite. Configured test-rule success also stays
deferred because Bazel requires test runfiles. Audit next only
`WP-6-m4-cquery-executables-nontest-successor-audit`, then implement the exact
non-test projection and bounded typed error mapping without claiming either
deferred surface.

### Non-test configured executables activation design ACCEPT (2026-08-09)

The audit accepted the existing query/core draft: it admits only
`executables`, shares one invocation through the sole recursive fold, evaluates
the operand once, borrows retained configured capability, and preserves ordered
full-key sets. The predicate remains exactly
`executable && !rule_class.ends_with("_test")`; neither target spelling nor
`test_kind` is classification state.

The remaining error correction is typed and narrow. Add a crate-private loaded-
rule error carrying the existing compact rule class, convert it to a dedicated
`AnalysisErrorKind`, and map only that kind to cquery exit 1. Generic analysis,
request, and infrastructure failures remain exit 2; missing-target and query
evaluation failures remain exit 1. Existing one-shot and daemon publishers
already consume the terminal exit code, so protocol/CLI/server production must
not change.

The checked-in 12-row fixture plus accepted M3 exported-class evidence is
sufficient; add no configured test-rule row. Required Rust evidence covers the
pure retained-capability predicate/order matrix, non-executable -> executable
-> warm -> restored daemon behavior, missing-executable recovery, and typed
exit discrimination. Implement next only
`WP-6-m4-cquery-executables-nontest-implementation`. Stop on message matching,
all-analysis exit remapping, test/runfiles breadth, package reload, new state,
protocol changes, or fixture expansion.

### Non-test configured executables implementation ACCEPT (2026-08-09)

The bounded activation is **ACCEPTED**. Configured validation admits the exact
existing `executables(expr)` signature, literal collection traverses its sole
operand, and loading/configured contexts share one invocation through the sole
recursive fold. The configured environment borrows retained
`AnalysisResult.rule_capability` and preserves ordered full configured-key set
identity under `executable && !rule_class.ends_with("_test")`.

Missing executable output is now a compact typed path from loaded-rule
evaluation through `AnalysisErrorKind` to a dedicated cquery terminal. Only
that analysis kind exits 1; generic analysis remains exit 2. No message match,
publisher/protocol change, package reload, key/cache, or duplicate retained
metadata was added.

The unchanged 12-row Bazel 9.2 and rebuilt-Slug replays pass. Query (39+56+9),
analysis (1+5+2+16+4), server (44), focused CLI cquery (5), payload integrity,
format, archive, and diff checks pass. Full core and CLI each retain one
unrelated pre-existing failure. Independent review accepted the pure suffix/
order matrix, configured non-test false -> executable -> warm -> restored
lifecycle, and missing-executable recovery.

Live configured `_test` lifecycle remains deferred: a valid exported `_test`
class is necessarily a test rule, which injects external `@bazel_tools` test
dependencies and later runfiles semantics. Accepted loading-query and loading
invalidation tests already cover that real capability transition; no fake
configured class or test-rule claim was introduced. Audit next only
`WP-6-m4-configured-query-successor-audit-5`.

### Configured-query successor audit 5 ACCEPT (2026-08-09)

The audit selected `kind(regex, expr)` as the only remaining default function
closed over current configured state. Every successful cquery root is presently
a Starlark rule, and retained `RuleCapability.rule_class` supplies the exact
Bazel target-kind candidate `"<exported class> rule"`. Pinned Bazel 9.2
`KindFunction`, `ConfiguredTargetAccessor`, `RuleClass#getTargetKind`, and the
accepted M3 kind strings close the evidence without a new fixture.

Share one kind invocation and the existing bounded regex compiler through the
sole recursive fold. Compile once after existing eager root preparation and
before operand folding; preserve unanchored find, order, and full configured-key
identity. Candidate formatting is request-local only. Missing capability fails
closed at exit 2.

Native, alias, source/generated, package-group, test, external, pattern, and
traversal roots remain unsupported during existing eager analysis; `kind` must
not convert them to empty or success. Add no retained kind field, package read,
key/cache, compiler, protocol change, or analysis widening. Implement next only
`WP-6-m4-cquery-kind-rule-class-implementation`.

### Configured kind implementation ACCEPT (2026-08-09)

Configured `kind(regex, expr)` is **ACCEPTED** over the existing successful
Starlark-rule root domain. Validation/literal traversal use only the expression
operand; loading and configured contexts share one invocation, bounded compile,
and recursive fold. Core borrows rule capability, forms only the request-local
compact `"<exported class> rule"` candidate, and preserves regex find, order,
and full configured-key identity. Missing capability fails closed at exit 2.

Tests cover exact/substring/anchored/nonmatch, both outputs, order/dedupe,
target-name independence, malformed-regex/missing-root precedence, unsupported
filegroup analysis, and exported-class edit/warm/restoration. Query (40+56+9),
core cquery (7), server cquery (9)/full (45), CLI cquery (5)/full (44), the
unchanged Bazel/Slug executable fixture, integrity, format, archive, and diff
checks pass. Independent review returned `ACCEPT`; the known unrelated core
failure remains unchanged.

No retained kind value, package read, key/cache, protocol, fixture, or analysis
widening was added. Audit next only
`WP-6-m4-configured-query-successor-audit-6`.

### Configured-query successor audit 6 ACCEPT (2026-08-09)

The audit selected exact post-analysis `siblings(expr)` terminal behavior.
Pinned Bazel 9.2 evaluates the sole operand once and calls
`getSiblingTargetsInPackage` only for delivered targets; its configured
post-analysis environment throws `siblings() not supported for post analysis
queries`. Therefore empty operands succeed empty, while every nonempty operand
is a query-evaluation failure with empty stdout and exit 1.

This slice needs only set emptiness. Share one invocation through the sole fold;
do not enumerate packages or add graph/metadata state. Existing eager root
preparation remains authoritative, so missing-target and analysis terminals
precede evaluation. Arity remains a request error, and both output modes render
the same empty success.

Traversal functions remain inexact because retained direct dependencies omit
toolchain/platform/constraint and other configured nodes. Attrs, labels,
providers, tests, visibility, file functions, and config identity retain their
existing prerequisites. Implement next only
`WP-6-m4-cquery-siblings-post-analysis-terminal-implementation`.

### Configured siblings terminal implementation ACCEPT (2026-08-09)

Configured `siblings(expr)` is **ACCEPTED** with exact Bazel post-analysis
behavior. The shared invocation evaluates its operand once; core only checks
set emptiness. Empty and filtered-empty operands succeed empty in both output
modes. Nonempty/nested operands emit the exact evaluation diagnostic, empty
stdout, and exit 1. Eager missing-target/analysis precedence and arity exit 2
remain unchanged.

Tests cover shared-fold order, pure empty/nonempty classification, one-shot/
daemon parity, and empty -> error -> restored-empty invalidation. Query (41+56+
9), core cquery (8), loading siblings (6), commands (18), server cquery (10)/
full (46), CLI cquery (6), integrity, format, archive, and diff checks pass.
Independent review returned `ACCEPT`; known unrelated core/CLI full failures
remain unchanged.

No package enumeration/read, graph/key/cache, retained value, protocol, or
fixture was added. Audit next only
`WP-6-m4-configured-query-successor-audit-7`.

### Configured-query successor audit 7 ACCEPT (2026-08-09)

The audit selected exact vacuous post-analysis `visible(callers, targets)`.
Bazel evaluates callers then targets, each once. Empty callers make the
universal visibility predicate vacuously true and return targets unchanged;
empty targets return empty. Only two nonempty sets reach configured visibility
and fail with `visible() is not supported on configured targets`, empty stdout,
and evaluation exit 1.

This requires set emptiness only. Share one invocation through the sole fold;
preserve operand errors, eager root preparation, order, full configured-key
identity, and both outputs. Do not read packages/package groups, add visibility
metadata, or widen configured analysis.

Traversal/attrs/labels/providers/tests/config/file functions retain their
existing prerequisites. Implement next only
`WP-6-m4-cquery-visible-vacuous-post-analysis-implementation`.

### Configured visible implementation ACCEPT (2026-08-09)

Configured `visible(callers, targets)` is **ACCEPTED** over the vacuous
post-analysis domain. The shared invocation evaluates/materializes callers once
then evaluates targets once, preserving loading-query behavior. Cquery checks
only set emptiness: empty callers return the ordered target set, empty targets
return empty, and two nonempty sets produce the exact configured-target
evaluation diagnostic, exit 1, and empty stdout.

Tests cover operand order, identity/order/dedupe, filtered-empty callers, empty
targets, nested errors, both outputs, one-shot/daemon parity, and vacuous success
-> failure/warm -> restored lifecycle. Query (42+56+9), core cquery (9),
commands (18), server cquery (11)/full (47), CLI cquery (7), integrity, format,
archive, and diff checks pass. Independent review returned `ACCEPT`; known
unrelated core/CLI full failures remain unchanged.

No package/visibility metadata, graph/key/cache, retained value, protocol, or
fixture was added. Audit next only
`WP-6-m4-configured-query-successor-audit-8`.

### Configured-query successor audit 8 ACCEPT (2026-08-09)

The audit grouped `buildfiles(expr)` and `loadfiles(expr)` into one exact
post-analysis family. Bazel gives both one-expression signatures and calls the
same configured helper before operand evaluation. That helper always fails
with `buildfiles() doesn't make sense for the configured target graph`; the
`loadfiles` distinction is unreachable.

Slug must retain lexical-root validation/collection and eager configured-root
preparation. After successful preparation, either function emits the shared
evaluation terminal without folding its operand. Empty operands still fail;
nested regex/some/siblings/visible runtime work is masked; missing-target or
analysis preparation failures still win. Arity remains exit 2.

Implement only a private cquery dispatcher helper. Do not extend the query
environment trait, call loading-file/package APIs, or add graph/state/protocol.
Run next
`WP-6-m4-cquery-loading-files-post-analysis-terminals-implementation`.

### Configured loading-file terminals implementation ACCEPT (2026-08-09)

Configured `buildfiles(expr)` and `loadfiles(expr)` are **ACCEPTED** as one
post-analysis family. Both validate and collect lexical roots, then—after eager
configured-root preparation—emit the exact shared `buildfiles()` evaluation
diagnostic without folding the operand. Empty/nonempty operands, nested runtime
errors, both outputs, missing/analysis precedence, arity, one-shot/daemon, and
delete/recreate recovery are covered.

Query (43+56), commands (18), server (48), core cquery (10), CLI cquery (8),
integrity, format, archive, and diff checks pass. Independent review returned
`ACCEPT`; known unrelated core/CLI full failures remain unchanged. No public
query trait, package/loading-file API, graph/state, protocol, or fixture changed.

No remaining default function is directly closed over current configured
state. Traversal needs the complete configured node universe, including
toolchain/platform/constraint and non-root nodes; attrs/labels/providers/tests
retain separate metadata/runtime prerequisites. Design next only
`WP-6-m4-configured-query-graph-ownership-design` under reserved review.

### Configured-query graph ownership design REPLAN (2026-08-09)

Reserved review returned **REPLAN**. The active configured key/result cannot
yet represent Bazel's traversal universe: null-configuration file/package-group
nodes are absent; active analysis accepts only Starlark rules; ordinary attrs
lose edge classification; selected toolchain analysis discards type,
declaration, platform, and constraint identities; target platform is unmodeled.
Bazel traversal instead follows and flattens classified Skyframe dependencies.
Choosing a Rust node enum now would guess topology or create a second graph.

Run evidence first: `WP-6-m4-configured-query-toolchain-topology-oracle` extends
only the existing first-platform fixture TOML/expected JSON with depth 0/1/2/
full configured `deps` plus implicit/tool controls. Then pin alias/output/file/
package-group delegation, design platform/configured-node identity, generalize
the existing DICE analysis result to own oracle-admitted nodes/ordered edges,
and activate `deps` before other traversal functions. No lock may cross a DICE
compute.

### Configured toolchain topology oracle REPLAN (2026-08-09)

The first oracle attempt is **REPLAN** and its fixture delta is discarded. Six
depth/control probes established deterministic node tiers, opaque target/exec
configuration relationships, `--noimplicit_deps` root-only behavior, and
`--notool_deps` preservation. The focused correction added graph output and
restored inherited records, but correction rereview found its ten independent
message-shape patterns did not pin graph order, shared configuration identity,
or graph-only selected-implementation edges.

The packet exhausted its correction budget. Design next only
`WP-6-m4-configured-query-toolchain-topology-oracle-retry-design`: freeze a
single anchored multiline graph discriminator with named configuration
backreferences and every claimed parent edge before regenerating evidence. No
fixture, Bazel, Rust, harness, or workspace edit belongs in the design packet;
bundle its bookkeeping with a later accepted oracle change.

### Configured topology oracle retry design ACCEPT (2026-08-09)

Pre-edit review accepted one anchored multiline unfactored-graph pattern. It
binds exact line order; root and selected-implementation edges; platform to
constraint-value edges; constraint-value to setting edges; common opaque target
and execution configuration classes; and rejects extra lines. Keep the six
depth/control probes, for seven new commands total. Generate next only
`WP-6-m4-configured-query-toolchain-topology-oracle-retry`.

### Configured toolchain topology oracle retry ACCEPT (2026-08-09)

The retry is **ACCEPTED**. Seven new Bazel 9.2 commands pin depth 0/1/2/full
label order, opaque target/exec configuration equivalence, implicit/tool option
behavior, and one exact unfactored graph. The anchored graph binds root and
selected-implementation toolchain edges, registered/host platforms, platform
to constraint values, constraint values to settings, line order, and no extras.

Generation/replay, integrity (3), archive, and diff checks pass. The six
inherited lifecycle records remain byte-identical. Independent pre-edit and
final reviews returned `ACCEPT`; only fixture TOML/generated JSON changed.

Pin delegation topology next in
`WP-6-m4-configured-query-delegation-topology-oracle`, then resume configured
platform/node ownership design. No Rust graph may precede that evidence.

### Configured delegation topology oracle design ACCEPT (2026-08-09)

The read-only audit selected a new isolated root-only
`cquery-delegation-topology` fixture. Seven commands cover filtered depth/full
labels, exact unfactored graphs, `rdeps` delegation unwinding, and alias-bypass
mutation/restoration. The workspace combines an ordinary and transitioned
edge, two-hop alias chain, explicit source, declared output/producer, and nested
visibility package groups without toolchain/action execution.

Every output is fully anchored; target/configuration bytes are opaque captures
and backreferences. Package groups are claimed only if the default Bazel graph
exposes the exact null-config top/leaf chain; absence triggers a split/replan.
Allow only six new fixture files, seven commands, and 850 added lines. Run next
`WP-6-m4-configured-query-delegation-topology-oracle-implementation` with no
Rust, harness, payload, Cargo, or plan edits by the worker.

### Configured delegation topology oracle REPLAN (2026-08-09)

The combined fixture is **REPLAN** and was removed before acceptance. Bazel's
default graph exposes `root(cfg) -> vis_top(null) -> vis_leaf(null)`, but also
adds host-platform, external platform, and transition-allowlist nodes.
`--noimplicit_deps` removes that noise and the nested package-group edge while
retaining only `root -> vis_top`. One fixture cannot claim both clean core
delegation and complete visibility topology under the accepted stops.

Split the evidence. Run next `WP-6-m4-configured-query-delegation-core-oracle`
with visibility removed and `--noimplicit_deps` throughout, covering only
transitioned ordinary edges, aliases, source/output targets, and `rdeps`
unwinding. Preserve the observed package-group/default-versus-noimplicit split
for a later oracle explicitly composed with accepted implicit/toolchain nodes.

### Configured delegation core oracle REPLAN (2026-08-09)

The noimplicit core retry is **REPLAN** and was discarded. Its topology was
clean, but `rdeps` multi-label output changed order across fresh roots. Pinned
Bazel source confirms cquery has no ordinary-query `order_output` path: label
formatting preserves callback iteration, while unfactored graph formatting
sorts nodes deterministically by label and configuration.

Retry as `WP-6-m4-configured-query-delegation-graph-oracle-retry`. Keep only a
singleton label row; use exact anchored unfactored DOT for every multi-node
deps/rdeps/mutation assertion. Explicitly model the default graph's visibility
and accepted implicit/external nodes rather than hiding them. Cap the new
fixture at six commands, six files, and 760 lines; stop on any cross-root graph
variation or missing logical alias/package-group chain.

### Configured delegation graph oracle retry ACCEPT (2026-08-09)

The graph-output retry is **ACCEPTED**. One isolated six-file, six-command
fixture uses a singleton label row and fully anchored unfactored DOT for all
multi-node assertions. It pins base/transitioned configurations, ordinary and
two-hop alias edges, source/null and output/producer nodes, reverse-delegation
membership, visibility package groups, every observed implicit host/platform/
transition-allowlist edge, and alias mutation/restoration.

Bazel 9.2 generation and distinct-root replay pass. Integrity (3), archive,
and diff checks pass. Independent review accepted exact graph patterns and a
focused provenance correction. No Rust, harness, payload, Cargo, or existing
fixture changed.

Toolchain and delegation topology evidence now closes the prerequisite for
`WP-6-m2-configured-node-platform-identity-owner-design`. Freeze platform and
node identity ownership before generalizing the existing DICE analysis result;
exact Bazel configuration hash bytes remain deferred.

### Configured-node/platform identity owner design ACCEPT (2026-08-09)

Reserved correction review returned **ACCEPT** for one recursive configured-node
DICE graph. The Need-aware production `RootConfiguredTargetAnalysisKey` is
renamed/generalized to the sole workspace-qualified `ConfiguredNodeAnalysisKey`;
the legacy parallel `ConfiguredTargetAnalysisKey` is migrated and deleted.
Root string-setting/default resolution moves into the existing command-root
preparation transaction, so the node key has no unresolved-request variant.

`ConfiguredNodeKey` is canonical label plus exactly one of structural
`ConfigurationKey` or `Null`. Production configuration identity admits only the
full `SlugConfiguration`; target, exec, and host-like roles come from its kind.
A transition is identified by its resulting structural configuration, never by
its origin, so equal outputs converge. Transition origin stays on the incoming
edge. Platform labels and toolchain selection are retained result facts unless
already part of the structural configuration; none are ad-hoc identity bytes.

The sole retained `ConfiguredNodeResult` owns node kind, ordered immutable
edges, providers, actions, outputs, diagnostics/capability, platform identity,
ordered candidate execution platforms, selected execution platform, and the
selected toolchain declaration/type/implementation. DICE equality includes all
of those facts. Need and complete error are invalid/non-equal; only complete
success is valid and structurally equal. Events never participate. Computation
uses existing package/module/repository/Host DICE owners and holds no lock
across a compute.

Each edge owns its target, semantic kind, and exact `implicit`/`tool` bits.
Explicit ordinary/transitioned attributes, alias actual, generated-by, source,
and declaring-visibility edges are `false/false`. Package-group includes,
toolchain requirement/selected implementation, candidate execution platforms,
host platform, platform constraints, constraint settings, and the function
transition allowlist are `true/false` for the admitted evidence. Therefore
`--noimplicit_deps` removes the latter edges while `--notool_deps` preserves the
entire admitted topology; no `tool=true` edge activates without a new oracle.
Transitioned attributes retain attribute/index/origin while convergent target
nodes remain shared.

Storage stays Buck2-derived and compact: `Arc<[T]>`, `CompactString`,
`SmallMap`/`SmallSet`, `Dupe`, and `Allocative`; no retained standard-map graph,
global interner, query cache, or filesystem bypass is added. BUILD, `.bzl`, and
MODULE parsing/evaluation remains on vendored `starlark-rust` with Slug-owned
Bazel globals and effects. Exact Bazel configuration/output/ActionKey bytes,
JVM/Java, general transitions/aspects, multi-root label ordering, and unproven
host/tool edges remain hard stops.

The serial route is: (1) consolidate the production analysis owner and
structural configuration boundary; (2) introduce structural/null node, edge,
and result substrate; (3) add fixture-admitted root/external delegating/native
nodes with verbatim `@bazel_tools`; (4) retain toolchain/platform topology; and
(5) activate only evidence-backed `deps`, singleton-anchored `rdeps`, label,
and unfactored graph output. Run next
`WP-6-m2-configured-analysis-single-owner-implementation`; no documentation-only
commit is permitted.

The implementation audit split that first step without changing the accepted
architecture. `WP-6-m2a-analysis-key-consolidation` first renames the Need-aware
root key to `ConfiguredNodeAnalysisKey`, deletes the legacy parallel key, and
preserves its temporary resolved/request input enum. Only then may
`WP-6-m2b-command-root-setting-preparation` move default/explicit setting
resolution into Build/Cquery command-root preparation, remove the request
variant, and enforce the structural-only production configuration boundary.
Combining both refactors would cross two command roots and conceal whether the
single-owner invariant was independently achieved. Run M2a next.

### Configured analysis key consolidation ACCEPT (2026-08-09)

`WP-6-m2a-analysis-key-consolidation` is **ACCEPTED**. The legacy
`ConfiguredTargetAnalysisKey` implementation and export are deleted, and the
Need-aware production key is now the sole `ConfiguredNodeAnalysisKey` used by
recursive analysis, build action closure, cquery, the retained legacy
workspace adapter, and focused tests. The temporary resolved/root-setting
request enum remains unchanged for M2b. The retained legacy adapter converts
its workspace to the normalized identity and fails explicitly if its already
committed observations cannot satisfy a Need; it does not reconstruct the old
graph.

Full analysis passes 28 tests; server passes 48; core passes 154/155 with only
the unchanged direct-external visibility mismatch; the rebuilt CLI passes
47/48 with only the unchanged `bzl_cycle` unavailable-root-node failure.
Formatting, diff, archive integrity, and a zero-old-symbol grep pass. The
functional delta is net-negative in production and uses the existing compact
keys/results unchanged. Independent review returned `ACCEPT`; parser/evaluator,
configuration representation, node kinds, edges, query behavior, and public
bytes did not change.

Run next only `WP-6-m2b-command-root-setting-preparation`: move root-setting
default/explicit resolution into both command-root preparation paths, remove
the unresolved key mode, and make opaque/legacy configuration unrepresentable
at the production key boundary. No configured-node breadth precedes it.

### Command-root setting preparation ACCEPT (2026-08-09)

`WP-6-m2b-command-root-setting-preparation` is **ACCEPTED**. Build, Cquery, and
recursive declared dependencies now call one analysis-owned Need-aware
preparation path before constructing the sole `ConfiguredNodeAnalysisKey`.
Default, explicit, and carried string settings resolve to the final structural
configuration; explicit setting Needs precede target errors, mismatched carried
labels fail closed, and equal transition outputs converge. The unresolved
request variant is deleted and the key constructor rejects legacy/opaque
configuration.

Full analysis passes 28 tests and server passes 48. Core passes 154/155 with
only the unchanged external-query visibility diagnostic mismatch; the rebuilt
CLI passes 47/48 with only the unchanged `bzl_cycle` unavailable-root failure.
Rich activation evidence retains parent-to-child DICE node provenance, format
and diff checks pass, and independent correction review returned `ACCEPT`.
No second key/cache/graph, parser change, JVM/Java, CI, or public hash-byte work
landed.

Run next only `WP-6-m2c-configured-node-result-substrate`: introduce the
reviewed structural/null node identity, classified immutable edges, and the
single retained configured-node result shape before adding native/delegating
topology or cquery traversal breadth.

### Configured-node result substrate ACCEPT (2026-08-09)

`WP-6-m2c-configured-node-result-substrate` is **ACCEPTED**. One structural/null
`ConfiguredNodeKey`, one immutable `ConfiguredNodeResult`, and classified
ordered edges replace the configured-only result/dependency substrate without a
second DICE key, graph, cache, or retained adjacency. Transitioned edges point
at the resolved child node while retaining attribute-local origin; equal
transition outputs converge. Every admitted edge kind has its exact fixed
implicit/tool bits, and only complete successes are DICE-valid or equal.

Analysis passes 32 tests. A later clean detached validation corrected the
command-side baseline after success-only error validity: core passes 153/155
with missing-executable activation selection and the external-visibility
diagnostic mismatch; server passes 45/48 with the same activation-selection
owner obscuring three semantic error terminals; the rebuilt CLI passes 47/48
with the unchanged `bzl_cycle` unavailable-root failure. The functional
delta is 152 net production and 216 net test lines across nine production and
three test files; the production-file allowance expanded only for required
core/server/CLI migration after deleting the old public result shape. Format,
diff, archive, old-symbol, and independent correction review gates pass. The
vendored Buck2 `starlark-rust` parser/evaluator is unchanged.

Run next only `WP-6-m2d-root-delegating-native-node-activation`: activate root
alias, referenced-source/null, generated-output, and package-group nodes
through the same key/result and accepted edge kinds. Filegroup and
`exports_files`-specific activation remain unproven by this fixture. Literal external
`@bazel_tools` remains stopped until apparent-repository routing, external glob,
and the `@platforms` dependency can be owned without synthetic content.

### Root delegating native-node activation ACCEPT (2026-08-09)

`WP-6-m2d-root-delegating-native-node-activation` is **ACCEPTED** for exactly
the Bazel 9.2 fixture-proven root slice: two-hop alias forwarding, generic
referenced source/null identity, generated-output-to-producer identity, rule
declaring visibility, and ordered package-group includes. Filegroup and
`exports_files`-specific behavior were removed from scope because the accepted
fixture does not exercise them. External delegating references fail closed;
literal `@bazel_tools` remains behind its real repository route, external glob,
and `@platforms` dependencies.

The sole `ConfiguredNodeAnalysisKey` now admits configured and null nodes, and
the sole `ConfiguredNodeResult` retains their classified ordered edges. Source
existence uses the resolved-path DICE owner, propagates Needs, rejects missing
or wrong-kind paths, and proves create/delete/recreate invalidation. Aliases
forward custom providers across both layers and restore structurally after an
actual-edge edit. The build action closure traverses alias and generated nodes
to the producer action while filtering source and visibility null nodes from
public analyses. Vendored Buck2 `starlark-rust` is unchanged.

Analysis passes all 34 tests and identity passes all 21 tests. Core passes
154/156: the new downstream closure regression passes, while the two clean-M2c
baseline failures remain missing-executable activation selection and the
external-visibility diagnostic mismatch. Server remains 45/48 on the same
clean-M2c error-sidecar baseline; the rebuilt CLI remains 47/48 on the existing
query-cycle activation baseline. Formatting, diff, cap, scope, source-path
invalidation, and independent review gates pass.

Run next only `WP-6-m2e-analysis-error-activation-sidecar-prerequisite`:
preserve complete semantic analysis errors for command terminal publication
without making them DICE-valid/equal or adding another analysis owner. Restore
the three server error terminals and focused core missing-executable result
before configured-query traversal breadth.

### Analysis-error activation sidecar prerequisite ACCEPT (2026-08-09)

`WP-6-m2e-analysis-error-activation-sidecar-prerequisite` is **ACCEPTED**.
Configured analysis errors remain DICE-invalid and non-equal. The command
sidecar selector now permits only build `Analysis` and cquery
`MissingTarget`/`ExecutableRuleMissingExecutable`/`Analysis` terminals to prune
exact `UnavailableRoot` nodes before selecting the surviving activation
closure. Dirty, unverified, foreign, unavailable-dependency, and every
non-analysis terminal remain strict failures; no second analysis key, cache,
or graph was added.

Focused evidence proves strict default rejection, opted-in transient-root
pruning, cquery missing-executable publication, build alias-to-native analysis
error publication and edit recovery, and preservation of a successful sibling
cquery root's event sidecars beside an analysis error. The pruned transient
error root's own event batch has no accepted oracle requirement in this packet
and is not claimed. Core passes 157/158 with only the unchanged external-query
visibility diagnostic mismatch; server passes all 48 tests; the rebuilt CLI
passes 47/48 with only the unchanged query-cycle unavailable-root baseline.
Formatting, diff, archive, scope, and independent review gates pass. Vendored
Buck2 `starlark-rust` is unchanged.

Run next only `WP-6-m2f-root-toolchain-platform-topology-activation-audit`:
reconcile the accepted Bazel 9.2 topology evidence with live root owners and
freeze the smallest non-synthetic platform/constraint/toolchain activation
slice before configured-query traversal breadth. The audit is read-only.

### Root toolchain/platform topology activation audit ACCEPT (2026-08-09)

`WP-6-m2f-root-toolchain-platform-topology-activation-audit` is **ACCEPTED**.
The live loading owner already retains root native platform, constraint,
toolchain-type, declaration, and implementation labels, and the analysis
resolver already validates ordered registrations and first-compatible
selection. Missing state was confined to a Host-free Target-to-Exec structural
role projection, native configured-node results, ordered implicit edges, and
compact selection facts on the sole configured result.

The exact implementable subgraph is requester-to-type/selected-implementation/
root-candidate-platforms plus same-package registered implementation-to-root-
candidate-platforms and platform-to-constraint-value-to-setting. Toolchain
declarations are selection facts, not nodes. Literal `@bazel_tools`,
`@platforms`, host-platform tails, and cross-package reverse implementation
topology remain stopped; no synthetic content or second graph is permitted.
Run next only `WP-6-m2g-root-toolchain-platform-topology-activation`.

### Root toolchain/platform topology activation ACCEPT (2026-08-09)

`WP-6-m2g-root-toolchain-platform-topology-activation` is **ACCEPTED**. A
narrow Slug-native Target-to-Exec projection reuses the complete structural
option records and root setting without Host observation. The sole configured
analysis key now activates root toolchain-type, platform, constraint-value,
and constraint-setting nodes in their fixture-proven Target/Exec roles. The
sole configured result retains one compact ordered candidate/optional-selection
value and the accepted requirement, selected-implementation, candidate,
platform-constraint, and setting edges with `implicit=true`, `tool=false`.

Direct same-package registered implementations intrinsically retain candidate
platform edges before or after requester analysis. Unregistered zero-toolchain
rules do not observe the MODULE anchor. Cross-package toolchain selection still
succeeds but retains no unproven reverse topology; direct toolchain declaration,
wrong-role native, and admitted external-registration paths fail closed.
Configuration passes 37 tests and analysis passes 36. Core remains 157/158 on
the unchanged external-visibility diagnostic mismatch, server passes 48/48,
and the rebuilt CLI remains 47/48 on the unchanged `bzl_cycle` unavailable-root
baseline. Format, diff, archive, cap, parser-scope, and independent correction
review gates pass.

Run next only `WP-6-m2h-configured-query-root-traversal-activation-audit`:
reconcile the retained root configured graph with the existing cquery
expression/evaluator owners and freeze the smallest evidence-backed `deps`
slice without inventing the stopped external host-platform tail. The audit is
read-only; `rdeps`, label/graph formatting breadth, parser changes, JVM/Java,
and exact Bazel hash bytes remain excluded.

### Configured-query root traversal audit and noimplicit activation ACCEPT (2026-08-09)

`WP-6-m2h-configured-query-root-traversal-activation-audit` selected and
`WP-6-m2i-configured-query-root-noimplicit-deps-activation` **ACCEPTED** the
smallest exact forward traversal: top-level
`deps(//root[, nonnegative-depth]) --noimplicit_deps`. The shared
Buck2-derived query expression fold now dispatches that one form; vendored
`starlark-rust` and the query parser are unchanged. Set, let, variable,
multi-root, nested, negative-depth, `rdeps`, graph-output, and external-label
forms remain rejected.

The command root activates each breadth-first frontier through the sole
`ConfiguredNodeAnalysisKey`, unions all Needs before the first stable child
analysis error, and retains only ordered `Arc<ConfiguredNodeResult>` handles
plus a request-local full-`ConfiguredNodeKey` index. Evaluation reads the
authoritative ordered `ConfiguredNodeResult::edges()` directly; no adjacency,
DICE key, graph, or cache is copied. Depth zero does not traverse an edge.
Null source/package-group identities survive to label output as `(null)`,
configured-output claims skip them, and Starlark-label output remains
canonical. The existing label projection and multi-node callback order are
explicitly Slug-native rather than claims about deferred Bazel hash/order
bytes.

Default implicit traversal remains unsupported because Bazel adds the stopped
`@bazel_tools`/`@platforms` host and transition-allowlist tail.
`--noimplicit_deps --notool_deps` is accepted and identical because every
currently admitted edge is `tool=false`; any future `tool=true` edge fails
closed pending evidence. One-shot and daemon requests carry the two primitive
filter booleans symmetrically.

Query passes 111 tests, commands 19, server 48, and the focused core and
rebuilt-CLI one-shot/daemon traversal regressions pass. Full core remains at
its unchanged external-visibility diagnostic mismatch and full CLI at its
unchanged `bzl_cycle` unavailable-root baseline. Formatting, diff, archive,
parser-scope, stale-daemon cleanup, and independent design/final review gates
pass.

Run next only `WP-6-m2j-configured-query-forward-traversal-successor-audit`.
Prefer exact noimplicit unfactored graph output or a smaller forward consumer;
keep default external topology, multi-root label-order claims, reverse
traversal, parser replacement, JVM/Java, exact hashes, and CI stopped.

### Configured-query forward successor audit ACCEPT (2026-08-09)

`WP-6-m2j-configured-query-forward-traversal-successor-audit` selected one
bounded successor: top-level unbounded
`deps(<single-root>) --output=graph --nograph:factored --noimplicit_deps`.
Accepted Bazel 9.2 delegation evidence pins structural membership, edges, and
unfactored node-then-edge layout. Exact Bazel seven-character configuration
tokens remain deferred; rendered configuration spelling and any resulting
same-label ordering are explicitly Slug-native.

Run next `WP-6-m2k-configured-query-unfactored-graph-output`. Generalize only
the existing unfactored renderer around a successor callback over selected
`ConfiguredNodeResult::edges()`. Retain no second graph or copied adjacency.
Reject depth, factored/default-implicit, nested, multi-root, reverse, and
external-tail forms. Parser/evaluator/loading-query graph ownership, vendored
`starlark-rust`, JVM/Java, exact hashes, and CI remain stopped.

### Configured-query unfactored graph output ACCEPT (2026-08-09)

`WP-6-m2k-configured-query-unfactored-graph-output` is **ACCEPTED**. The
bounded graph mode admits only top-level unbounded single-root
`deps --noimplicit_deps --output=graph --nograph:factored`. It renders the
selected full configured/null identities by cursor-rescanning authoritative
`ConfiguredNodeResult::edges()`; it retains no copied adjacency, second graph,
cache, or DICE key. The shared unfactored writer now operates over that cursor
and preserves existing loading-query bytes, duplicate suppression, and cycle
termination. Configuration tokens and their ordering remain explicitly
Slug-native.

Query passes 112 tests, commands 19, and server 48. Focused core and rebuilt
one-shot/daemon CLI graph regressions pass; the scoped core suite passes 159
library and 12 integration tests with the two unrelated baselines filtered,
and full CLI remains 49/50 on the unchanged `bzl_cycle` unavailable-root
baseline. Formatting, diff, archive, parser-scope, stale-daemon, cap, and
independent final review gates pass. The Rust delta is 216 production and 194
test lines, 410 total, within 260/450/710 caps.

Run next only `WP-6-m2l-configured-query-forward-successor-audit`. Prefer an
evidence-backed depth-limited graph/output extension or a smaller forward
consumer. Keep factored/default-implicit/external topology, reverse traversal,
parser replacement, JVM/Java, exact hashes, and CI stopped.

### Configured-query depth-graph successor audit ACCEPT (2026-08-09)

`WP-6-m2l-configured-query-forward-successor-audit` selected depths 0, 1, and
2 for the existing noimplicit unfactored graph mode. Accepted depth label rows
and full delegation DOT combine with pinned Bazel 9.2 `DepsFunction` bounded
selection and `GraphOutputFormatterCallback` selected-membership edge filtering
to prove the exact induced topology. Configuration spelling and same-label
ordering remain Slug-native.

Run next `WP-6-m2m-depth-limited-unfactored-cquery-graph-output`. Replace only
the command and raw-server unbounded-only guards with an explicit 0-through-2
gate. Core production, query parser/evaluator/traversal, renderer, vendor,
fixtures, oracle, JVM/Java, exact hashes, and CI remain stopped.

### Configured-query depth-limited graph output ACCEPT (2026-08-09)

`WP-6-m2m-depth-limited-unfactored-cquery-graph-output` is **ACCEPTED**. The
existing noimplicit unfactored graph mode now admits explicit depths 0, 1, and
2. Only the command and raw-server guards changed; configured closure,
evaluation, and rendering production code were already the exact selected
induced-subgraph owner. Depth 3 and above remain rejected.

Commands pass 19 tests, server 48, and the focused core and rebuilt CLI graph
tests pass. Core coverage pins root-only depth 0, seven nodes/six root edges at
depth 1, and all nine nodes/nine induced edges at depth 2, equal to the full
fixture. Formatting and diff gates plus independent final review pass. The
Rust delta is 4 production and 94 test lines, 98 total, within 10/180/190 caps.

Run next only `WP-6-m2n-configured-query-forward-successor-audit`. Determine
whether a source-backed larger-depth boundary or a smaller forward consumer is
the next closed slice. Keep factored/default-implicit/external topology,
reverse traversal, parser replacement, JVM/Java, exact hashes, and CI stopped.

### Configured-query remaining-depth audit ACCEPT (2026-08-09)

`WP-6-m2n-configured-query-forward-successor-audit` proved the temporary
depth-2 graph cap is unnecessary. Pinned Bazel 9.2 applies bounded BFS to every
parser-valid nonnegative Java `int`, then emits the selected induced graph.
Slug's existing closure, evaluator, and renderer already own those generic
semantics, including `i32::MAX`; only two admission guards differ.

Run next `WP-6-m2o-remove-cquery-graph-depth-cap`. Delete only the command and
raw-server `> 2` guards and mechanically remove unused bindings. Core/query/
renderer production, parser, vendor, fixture/oracle, JVM/Java, exact hashes,
and CI remain stopped.

### Configured-query all-depth graph admission ACCEPT (2026-08-09)

`WP-6-m2o-remove-cquery-graph-depth-cap` is **ACCEPTED**. Commands and the raw
server now admit every parser-valid nonnegative Java-`int` depth for the
existing noimplicit unfactored configured `deps` graph path. The parser still
rejects negative and above-`i32::MAX` values. Depth 3 proves induced-subgraph
truncation on a deeper chain; `i32::MAX` equals unbounded topology and bytes in
one-shot and retained-daemon execution.

Production removed 10 net lines across the two admission owners; tests added
44 net lines, for +34 total Rust. Commands passed 19, focused core passed 2,
server passed 48, the rebuilt CLI graph regression passed, and independent
review returned ACCEPT. `starlark-rust`, parser/evaluator/traversal/renderer,
JVM/Java, CI, fixtures/oracles, and exact configuration hashes were untouched.

Run next only `WP-6-m2p-configured-query-forward-successor-audit`. Select the
smallest exact forward consumer that reuses the sole configured result graph;
keep factored/default-implicit/external topology, reverse/multi-root forms,
parser replacement, JVM/Java, exact hashes, and CI stopped.

### Configured-query label-kind successor audit ACCEPT (2026-08-09)

`WP-6-m2p-configured-query-forward-successor-audit` selected Bazel 9.2's
registered `label_kind` formatter. Pinned
`LabelAndConfigurationOutputFormatterCallback` prepends
`Target.getTargetKind()` to the same label/configuration row in the same
callback order. Slug already retains the exact rule class or structural
source/generated/package-group kind beside each selected configured/null node.
No graph, adjacency, key, cache, parser, evaluator, or retained representation
is required.

The formatter applies to the entire already-admitted configured-query
expression subset, including noimplicit top-level `deps` at every admitted
depth; a deps-only output guard would invent an unnecessary restriction.
Configuration tokens and configuration-sensitive order remain Slug-native.
The public daemon-wire variant requires fallible, fail-closed formatting and
downstream command/server/CLI coverage. Independent reserved review returned
ACCEPT.

Run next `WP-6-m2q-configured-query-label-kind-output`. Keep nested deps,
factored/default-implicit/external/reverse graph breadth, other output modes,
parser/evaluator/analysis/vendor/fixture/oracle changes, JVM/Java, exact hashes,
and CI stopped.

### Configured-query label-kind output ACCEPT (2026-08-09)

`WP-6-m2q-configured-query-label-kind-output` is **ACCEPTED**. The complete
already-admitted configured-query expression subset now supports
`--output=label_kind`. Formatting preserves the selected TargetSet order and
existing label/configuration/null bytes while prepending Bazel's exact target
kind: retained rule class, source file, generated file, or package group.
Capability-free rule-like nodes fail closed as infrastructure errors rather
than guessing or panicking.

The public command and serde-wire variants route identically through one-shot
and retained-daemon presentation after semantic terminal acceptance. Existing
graph wire round-trip coverage remains intact. Commands pass 19, focused core
passes 14, server passes 48, the CLI rebuild and one-shot/daemon/max-depth
regression pass, and independent final review returned ACCEPT. Production
added 53 net Rust lines, tests 115, and total Rust 168, within 90/180/270 caps.

Query parsing/evaluation, configured analysis, DICE identity, retained
representation, vendored `starlark-rust`, fixtures/oracles, JVM/Java, CI, and
exact configuration hashes were untouched. Run next only
`WP-6-m2r-configured-query-forward-successor-audit`; compare a meaningful
forward composition with exact singleton-root expression breadth before
selecting one bounded successor.

### Configured-query forward composition audit ACCEPT (2026-08-09)

`WP-6-m2r-configured-query-forward-successor-audit` selected exactly
`executables(deps(<one concrete root>[, <nonnegative i32 depth>]))`. Unlike
singleton `deps(set(...))` syntax, this meaningfully composes the accepted
configured closure with the retained executable/non-test rule predicate while
requiring no new graph, key, cache, state, representation, output, or wire mode.

Pinned Bazel 9.2 `DepsFunction`, aggregate evaluation, `ExecutablesFunction`,
configured lookup-key identity, and graph callback sources establish complete
closure before filtering, full configured-key deduplication, stable relative
delivery order, and selected-induced graph edges without path contraction.
Accepted delegation-topology and executable-capability evidence is sufficient;
no new fixture or oracle is required. Configuration tokens and any ordering
derived from them remain Slug-native.

Run next `WP-6-m2s-configured-query-executables-deps-composition`. Keep the
direct `deps` fast-path accessor distinct from a new wrapper-aware preactivation
accessor, route the exact wrapper through the existing shared expression fold,
and reuse the existing closure, executable predicate, and formatters. Every
other nested `deps` form, default implicit/tool/external/factored/reverse
topology, parser replacement, JVM/Java, exact hashes, and CI remain stopped.

### Configured-query executable-deps composition ACCEPT (2026-08-09)

`WP-6-m2s-configured-query-executables-deps-composition` is **ACCEPTED**.
Configured query now admits exactly
`executables(deps(<one concrete root>[, <nonnegative i32 depth>]))` under
`--noimplicit_deps`. A wrapper-aware accessor preactivates the same complete
depth-bounded closure while the existing direct-`deps` accessor preserves its
fast path; the shared fold then applies the retained executable/non-test rule
predicate exactly once.

Full configured-key identity preserves transitioned duplicates. Text outputs
retain filtered closure order, and unfactored graph output keeps only direct
authoritative edges whose endpoints survive; it does not contract paths through
filtered non-executable nodes. All four retained output modes route identically
through one-shot and daemon execution. No graph, key, cache, DICE identity,
representation, output/wire family, fixture, oracle, or vendored
`starlark-rust` parser/evaluator changed.

Query passes 113 tests, commands 19, server 48, focused core composition and
Need-before-error regressions pass, the V2 CLI rebuild passes, and two focused
CLI one-shot/daemon regressions pass. Formatting, diff, stale-daemon, cap, and
independent final review gates pass. The net Rust delta is 58 production and
261 test lines, 319 total, within 100/300/400 caps.

Run next only `WP-6-m2t-configured-query-forward-successor-audit`. Keep general
nested `deps`, factored/default-implicit/tool/external/reverse topology, other
outputs, parser replacement, JVM/Java, exact hashes, and CI stopped.

### Configured-query filter-deps successor audit ACCEPT (2026-08-09)

`WP-6-m2t-configured-query-forward-successor-audit` selected exactly
`filter(<word regex>, deps(<one concrete root>[, <nonnegative i32 depth>]))`.
This meaningfully filters the complete configured closure by original label
while safely covering configured rules and retained null source/generated/
package-group nodes. `kind(deps(...))` crosses a capability-free-node boundary,
`some(deps(...))` adds cancellation and arbitrary-selection ownership, and
`siblings(deps(...))` is only an error terminal for nonempty configured sets.

Pinned Bazel 9.2 `RegexFilterExpression`, `FilterFunction`, configured-target
label access, and `DepsFunction` sources establish unanchored label matching,
operand-relative order, full-key retention, and closure-before-filter
composition. Accepted filter-label and delegation-topology evidence is
sufficient; no new fixture or oracle is required. Regex language and errors
remain the approved Slug-native Rust Unicode boundary.

Run next `WP-6-m2u-cquery-filter-deps-forward-composition`. Generalize only the
existing wrapper validator and preactivation accessor, then reuse M2s's nested
`deps` dispatch, complete closure, shared filter fold, authoritative edges, and
formatters. Every other nested `deps` form, default implicit/tool/external/
factored/reverse topology, parser replacement, JVM/Java, exact hashes, and CI
remain stopped.

### Configured-query filter-deps composition ACCEPT (2026-08-09)

`WP-6-m2u-cquery-filter-deps-forward-composition` is **ACCEPTED**. Configured
query now admits exactly
`filter(<word regex>, deps(<one concrete root>[, <nonnegative i32 depth>]))`
under `--noimplicit_deps`. Only expression admission and the existing
wrapper-aware preactivation accessor changed; M2s's shared fold, complete
closure, original-label predicate, full configured/null identity, authoritative
edges, and terminal formatters are reused unchanged.

Filtering preserves transitioned duplicates and operand-relative order. Graph
output is selected-induced, so an admitted descendant behind a filtered bridge
is isolated rather than connected by a synthetic edge. Empty filters succeed,
closure Need/error precedes filtering, and all four retained outputs route
through command/server and one-shot/daemon paths. Regex language and errors
remain the approved Slug-native Rust Unicode boundary.

Query passes 114 tests, commands 19, server 48, focused core topology and
lifecycle regressions pass, the V2 CLI rebuild passes, and two focused CLI
one-shot/daemon regressions pass. Formatting, diff, stale-daemon, cap, and
independent final review gates pass. The net Rust delta is 11 production and
145 test lines, 156 total, within 70/260/330 caps. No fixture, oracle, graph,
key, state, representation, wire/output family, or vendored `starlark-rust`
parser/evaluator changed.

Run next only `WP-6-m2v-configured-query-forward-successor-audit`. Keep general
nested `deps`, factored/default-implicit/tool/external/reverse topology, other
outputs, exact hashes, JVM/Java, and CI stopped.

### Configured-query kind-deps successor audit ACCEPT (2026-08-09)

`WP-6-m2v-configured-query-forward-successor-audit` selected exactly
`kind(<word regex>, deps(<one concrete root>[, <nonnegative i32 depth>]))`.
The exact target-kind mapping added for configured label-kind output closes the
former capability-free boundary: retained rule class yields `<class> rule`, and
retained structural nodes yield `source file`, `generated file`, or
`package group`. Other capability-free kinds remain unsupported and fail closed.

Pinned Bazel 9.2 `KindFunction`, configured target access, target-kind owners,
regex filtering, and `DepsFunction` establish the composition. Accepted
kind/label-kind and delegation-topology evidence covers the required strings,
full-key duplicates, null nodes, depths, and induced edges; no new oracle is
required. `some(deps(...))` remains stopped because Bazel's callback-level
arbitrary selection and cancellation conflict with complete preactivation.

Run next `WP-6-m2w-cquery-kind-deps-forward-composition`. Extend only exact
wrapper admission/preactivation and route configured kind through the existing
request-local target-kind mapping. Keep all other nested `deps`, stopped
topology, parser/vendor changes, JVM/Java, exact hashes, and CI excluded.

The shared mapping intentionally corrects already-admitted direct configured
`kind` for represented source, generated, and package-group targets as the same
Bazel-exact target-kind behavior. Unsupported capability-free nodes must retain
the current request-terminal class: convert the shared formatter failure to
`QueryError::syntax`, never an exit-1 evaluation error. Direct structural
success and unsupported-kind failure are required regressions.

### Configured-query kind-deps composition ACCEPT (2026-08-09)

`WP-6-m2w-cquery-kind-deps-forward-composition` is **ACCEPTED**. Configured
query admits exactly
`kind(<word regex>, deps(<one concrete root>[, <nonnegative i32 depth>]))`
under `--noimplicit_deps`. Wrapper admission reuses the complete closure, and
configured kind now shares the exact request-local target-kind mapping used by
label-kind: retained rule class, source file, generated file, or package group.

This also corrects the same already-admitted configured kind operation over
represented structural sets. Other capability-free native nodes remain
fail-closed as request errors through `QueryError::syntax`; direct structural
source roots retain their pre-existing analysis boundary. Full configured/null
identity, transitioned duplicates, closure order, and selected-induced graph
edges are preserved across every depth and output.

Query passes 114 tests, commands 19, server 48, four focused core regressions,
the V2 CLI rebuild, and two focused CLI one-shot/daemon regressions. Formatting,
diff, stale-daemon, cap, and independent final review gates pass. The net Rust
delta is 5 production and 150 test lines, 155 total, within 70/300/370 caps. No
fixture, oracle, graph, key, state, representation, output/wire family, or
vendored parser/evaluator changed.

Run next only `WP-6-m2x-configured-query-forward-successor-audit`. Keep general
nested `deps`, stopped topology, exact hashes, JVM/Java, and CI excluded.

### Configured-query chained-filter successor audit ACCEPT (2026-08-09)

`WP-6-m2x-configured-query-forward-successor-audit` selected exactly
`filter(<word regex>, executables(deps(<one concrete root>[, <nonnegative i32 depth>])))`.
Independent adjudication preferred this practical named-executable query over
kind/filter chains and syntax-only breadth. Pinned Bazel 9.2 and accepted
delegation, executable, and filter evidence establish complete closure followed
by executable/non-test and original-label filtering, with full-key order and
selected-induced edges.

`some(deps(...))` remains stopped because Bazel's streaming graceful
cancellation conflicts with complete closure preactivation and Need-before-error.
Run next `WP-6-m2y-cquery-filter-executables-deps-forward-composition`, changing
only fixed-shape admission/preactivation and reusing the existing shared fold.
Every other chain, stopped topology, parser/vendor change, JVM/Java, exact
hashes, and CI remain excluded.

### Configured-query named-executable chain ACCEPT (2026-08-09)

`WP-6-m2y-cquery-filter-executables-deps-forward-composition` is **ACCEPTED**.
Configured query admits exactly
`filter(<word regex>, executables(deps(<one concrete root>[, <nonnegative i32 depth>])))`
under `--noimplicit_deps`. Fixed-shape admission is the only production change;
the shared fold evaluates the complete closure, executable/non-test predicate,
then original-label filter while preserving full-key order and selected-induced
edges.

Query passes 114 tests, commands 19, server 48, focused core topology and Need
regressions, the V2 CLI rebuild, and two CLI one-shot/daemon regressions.
Formatting, diff, stale-daemon, cap, and independent final review gates pass.
The net Rust delta is 25 production and 149 test lines, 174 total, within
70/300/370 caps. No fixture, oracle, graph, key, state, representation,
output/wire mode, or vendored parser/evaluator changed.

Run next only `WP-6-m2z-configured-query-forward-successor-audit`. Keep general
nested `deps`, `some(deps)`, stopped topology, exact hashes, JVM/Java, and CI
excluded.

### Configured-query named-kind successor audit ACCEPT (2026-08-09)

`WP-6-m2z-configured-query-forward-successor-audit` selected exactly
`filter(<label regex>, kind(<kind regex>, deps(<one concrete root>[, <nonnegative i32 depth>])))`.
It provides a distinct two-dimensional named-kind query while reusing accepted
closure, target-kind, label, full-key, and induced-edge owners. Pinned source
and accepted kind/filter/delegation evidence are sufficient; regex remains the
approved Slug-native boundary.

The audit considered `rdeps(deps(...), label)` but design review initially
rejected implementation from forward edges alone. M31 later corrected that
reading: the accepted oracle explicitly retains both alias nodes, while
Bazel's separate delegation unwinding contracts raw Skyframe requester keys
before producing the same semantic configured-node edges Slug already retains.
`some` remains stopped on callback cancellation.

Run next `WP-6-m30-cquery-filter-kind-deps-forward-composition`, changing only
fixed-shape admission/preactivation. Keep all other chains, reverse traversal,
stopped topology, parser/vendor changes, exact hashes, JVM/Java, and CI excluded.

### Configured-query named-kind chain ACCEPT (2026-08-09)

`WP-6-m30-cquery-filter-kind-deps-forward-composition` is **ACCEPTED**.
Configured query admits exactly
`filter(<label regex>, kind(<kind regex>, deps(<one concrete root>[, <nonnegative i32 depth>])))`
under `--noimplicit_deps`. Fixed-shape admission is the only production change;
the shared fold evaluates closure, target kind, then label while retaining
structural/null kinds, full-key duplicates, order, and induced edges.

Query passes 115 tests, commands 19, server 48, three focused core regressions,
the V2 CLI rebuild, and two CLI one-shot/daemon regressions. Formatting, diff,
stale-daemon, cap, and independent final review gates pass. The net Rust delta
is 18 production and 209 test lines, 227 total, within 40/240/280 caps. No
fixture, oracle, graph, key, state, representation, output/wire mode, or
vendored parser/evaluator changed.

The forward-filter lane is now closed. Run next only
`WP-6-m31-cquery-reverse-delegation-normalization-design`: pin the retained
Bazel delegation/value-key semantics required before exact `rdeps` can reverse
the accepted configured graph. Do not infer them from alias edges or implement
reverse traversal in the design packet.

### Configured reverse-delegation normalization design ACCEPT (2026-08-09)

`WP-6-m31-cquery-reverse-delegation-normalization-design` is **ACCEPTED** after
correcting its premise. The accepted Bazel payload includes `alias_outer` and
`alias_inner`; bypass removes them only because they leave `deps(//:root)`, and
restore returns the complete chain. Ordinary aliases are not the delegation
that `PostAnalysisQueryEnvironment#skipDelegatingAncestors` unwinds.

Pinned Bazel 9.2 source shows that forward query traversal targetifies each raw
Skyframe dependency to the configured value's lookup key. Reverse traversal
first contracts requester keys whose values are owned by the current child,
then targetifies to that same semantic-key domain. Slug already performs this
normalization at analysis time: every authoritative edge names the computed
child result key, aliases retain their own keys, transitioned edges retain the
final structural child key plus transition origin, and source/package-group
nodes retain final null identity. No admitted noimplicit edge retains a raw
requester key.

Therefore no delegation relation, retained reverse adjacency, graph, cache,
DICE key, interner, or representation change is needed. Exact reverse BFS may
rescan the existing request-local closure's normalized forward edges and
deduplicate with full `ConfiguredNodeKey` identity. This keeps existing
`Arc`/`SmallMap`/`SmallSet`/`Allocative` ownership unchanged and adds no lock or
invalidation boundary.

Run next `WP-6-m31-cquery-rdeps-deps-normalized-reverse-traversal`. Admit only
unbounded `rdeps(deps(<one concrete root-repository root>), <one concrete
root-repository label>) --noimplicit_deps` across existing outputs. Resolve the
second label against every matching full key in the completed universe, retain
aliases and selected-induced edges, and prove bypass/edit/restore plus
Need/error and daemon behavior. Keep depth arguments, general reverse syntax,
implicit/tool/external/factored topology, parser/vendor changes, exact hashes,
JVM/Java, and CI stopped. No standalone documentation commit is permitted.

### Configured-query normalized reverse traversal ACCEPT (2026-08-09)

`WP-6-m31-cquery-rdeps-deps-normalized-reverse-traversal` is **ACCEPTED**.
Configured query now admits exactly unbounded
`rdeps(deps(<one concrete root-repository root>), <one concrete
root-repository seed>) --noimplicit_deps` across the existing label,
Starlark-label, label-kind, and unfactored graph outputs.

The evaluator completes the universe first, then validates the seed through
the existing root package-loading key without configuring or analyzing it.
Declared seeds resolve against every matching full configured/null key in the
completed universe; missing seeds report `MissingTarget`, while declared but
unreachable or default-configuration-failing seeds do not trigger outside-
universe analysis. Reverse BFS rescans authoritative normalized forward edges,
retains ordinary aliases, deduplicates by full `ConfiguredNodeKey`, and adds no
reverse graph, DICE key/cache, interner, lock, or retained representation.

The query crate passes its 50 library, 56 loading, and 10 query tests; commands
passes 19 and server passes 48. Focused core topology, universe-Need ordering,
missing/unreachable/transitioned seed, bypass/edit/restore, rebuilt CLI, and two
one-shot/daemon regressions pass. Formatting, archive, stale-daemon, diff, and
independent final review gates pass. The net Rust delta is 195 production and
234 test lines, 429 total, within the correction-adjusted 200/240/440 caps. The
larger production allowance covers the required universe-first loading-only
seed-validation seam; no vendored parser/evaluator or retained hot-path utility
changed.

Run next `WP-6-m32-cquery-rdeps-reverse-depth-admission`. Add only the optional
signed Java-`int` reverse depth to the accepted unbounded-universe shape: zero
returns matching seed keys, positive values add that many reverse BFS layers,
negative values return empty, and omission remains unbounded. Keep bounded
universe `deps`, general reverse/path expressions, wrappers, default
implicit/tool/external/factored topology, new reverse state, parser/vendor
changes, exact hashes, JVM/Java, and CI stopped.

### Configured-query reverse-depth admission ACCEPT (2026-08-09)

`WP-6-m32-cquery-rdeps-reverse-depth-admission` is **ACCEPTED**. The accepted
M31 shape now admits Bazel's optional signed Java-`int` reverse depth while its
`deps()` universe remains unbounded. Omission is unbounded, negative depth
returns empty after ordinary universe/seed validation, zero returns every
matching full configured/null seed key, and positive values add exactly that
many normalized reverse BFS layers.

The existing parser's Java-integer path accepts quoted signed bounds and rejects
both overflow directions. Core evidence covers base and transitioned duplicate
seed identities, alias boundaries at depths zero/one/two, maximum-depth equality
with unbounded traversal, empty negative output in all four output families,
and selected-induced graph edges. Universe-first Need ordering, loading-only
seed validation, and M31 lifecycle regressions remain green.

Query, commands, server, focused core, rebuilt CLI, and one-shot/daemon tests
pass serially, as do formatting, archive, stale-daemon, diff, and independent
review gates. The net Rust delta is 23 production and 128 test lines, 151 total,
within 60/220/280 caps. No retained representation, reverse state, DICE owner,
utility, parser/vendor content, output, or wire mode changed.

Run next `WP-6-m33-cquery-rdeps-bounded-universe-syntax-normalization`. Admit
the already-supported optional nonnegative Java-`int` depth on inner `deps()`,
but reproduce Bazel's subsequent unbounded transitive re-closure. Keep general
universe expressions, multi-root/set/variable forms, wrappers, other reverse or
path functions, default implicit/tool/external/factored topology, new reverse
state, exact hashes, JVM/Java, and CI stopped.

### Configured-query inner-depth normalization ACCEPT (2026-08-09)

`WP-6-m33-cquery-rdeps-bounded-universe-syntax-normalization` is **ACCEPTED**.
Bazel accepts the optional nonnegative inner `deps` depth, then cquery
`RdepsFunction` calls `getTransitiveClosure` on that result; because the root is
present at every admitted depth, the effective universe is the full unbounded
root closure. Slug now validates the syntax but explicitly clears the inner
depth for preactivation and generic universe evaluation.

Oracle-discriminating core coverage proves inner depths zero, one, two, maximum,
and omitted produce identical DOT topology and full configured-key vectors;
outer reverse depth remains independent. Query, focused core, rebuilt CLI and
one-shot/daemon checks pass with formatting, archive, daemon, diff, and review
gates. The packet changes only seven Rust files and adds no retained state,
graph, key, cache, or vendor change.

Run next `WP-6-m34-cquery-reverse-successor-audit` to select the next exact
bounded reverse-query shape from pinned Bazel 9.2 behavior. Keep general
universes, wrappers, path functions, stopped topology, exact hashes, JVM/Java,
and CI excluded.

### Configured reverse successor audit ACCEPT (2026-08-10)

`WP-6-m34-cquery-reverse-successor-audit` is **ACCEPTED** and selects direct
single-label universe spelling. Pinned Bazel 9.2 `RdepsFunction` evaluates the
first expression and then builds its unbounded transitive closure before
reverse traversal, so `rdeps(//:root, seed[, depth])` is equivalent to the
accepted single-root `deps` spelling. The local delegation oracle confirms
identical unfactored topology.

Run next `WP-6-m35-cquery-rdeps-direct-universe-normalization`. Normalize one
concrete root-repository universe label to the existing unbounded
`CqueryDepsSpec`, retaining all seed validation, full-key traversal, reverse
depth, output, and daemon behavior. Keep set/multi-root/variable/external
universes, wrappers, same-package reverse activation, paths, stopped topology,
exact hashes, JVM/Java, and CI excluded.

### Configured-query direct reverse universe ACCEPT (2026-08-10)

`WP-6-m35-cquery-rdeps-direct-universe-normalization` is **ACCEPTED**. Direct
`rdeps(root, seed[, depth])` structurally normalizes to the existing unbounded
single-root universe. Direct and `deps(root)` spellings are byte-identical for
all four outputs and omitted, negative, zero, positive, and maximum reverse
depths, including full configured-key vectors, aliases, transitioned seeds,
missing/unreachable seeds, and universe-first Need ordering.

Query passes 116 tests, commands 19, two focused core regressions, focused
server and rebuilt CLI daemon symmetry. Formatting, archive, daemon, diff, and
independent review gates pass. Production changes only `expr.rs` by six net
lines; tests add 77 net lines, 83 total, within 25/170/195 caps. No state, key,
cache, graph, vendor, output, or wire ownership changed.

Run next `WP-6-m36-cquery-filter-rdeps-direct`. Admit only
`filter(<word regex>, rdeps(<direct root>, <seed>[, depth]))` under
`--noimplicit_deps`. Compile the regex before any configured/source activation,
then evaluate reverse traversal and apply the existing label filter, preserving
selected-induced output. Keep every other wrapper,
non-direct universe, multiple seed, same-package reverse, path, stopped
topology, exact-hash, JVM/Java, and CI surface excluded.

### Configured-query filtered direct reverse ACCEPT (2026-08-10)

`WP-6-m36-cquery-filter-rdeps-direct` is **ACCEPTED** after one source-backed
precedence correction. The fixed outer filter compiles with the established
Slug-native regex contract before any command-root DICE, universe, or seed
work, matching Bazel's compile-before-operand ordering. Valid patterns then run
the accepted direct reverse traversal and existing label filter. Compilation is
deterministically repeated in the evaluator and never retained.

Query passes 117 tests, commands 19, two focused core regressions, focused
server, and rebuilt CLI daemon symmetry. Invalid regex masks universe Need and
missing universe/seed errors; valid traversal retains duplicate configured
keys, aliases, reverse depths, empty success, and selected-induced graphs.
Formatting, archive, daemon, diff, and independent design/final review gates
pass. The 295-net-line Rust packet remains within 130/320/450 caps and adds no
semantic state, DICE owner, graph, cache, or vendor change.

Run next `WP-6-m37-cquery-executables-rdeps-direct`. Admit only
`executables(rdeps(<direct root>, <seed>[, depth])) --noimplicit_deps`, run the
accepted reverse traversal first, then apply the existing executable-non-test
predicate. Keep every other wrapper and stopped reverse surface excluded.

### Configured-query executable direct reverse ACCEPT (2026-08-10)

`WP-6-m37-cquery-executables-rdeps-direct` is **ACCEPTED**. The fixed wrapper
runs the accepted direct reverse traversal first and then the existing
executable-non-test predicate; reverse errors prevent filtering. Full configured
identity, transitioned duplicates, depths, empty success, and selected-induced
graph output are retained without changing M36 regex preflight or any semantic
state.

Query passes 118 tests, commands 19, focused core/server, and rebuilt CLI daemon
symmetry; formatting, archive, daemon, diff, and independent review gates pass.
The net Rust delta is 37 production and 141 test lines, 178 total, within
80/260/340 caps. No DICE, graph, cache, vendor, output, or wire owner changed.

Run next `WP-6-m38-cquery-kind-rdeps-direct`. Admit only
`kind(<word regex>, rdeps(<direct root>, <seed>[, depth]))`, extend M36's
compile-before-activation preflight to this one wrapper, then apply the existing
configured target-kind projection after reverse traversal. Keep all other
wrappers and stopped reverse surfaces excluded.

### Configured-query kind direct reverse ACCEPT (2026-08-10)

`WP-6-m38-cquery-kind-rdeps-direct` is **ACCEPTED**. The fixed wrapper shares
M36's regex-before-activation preflight, then runs the accepted direct reverse
traversal and existing fail-closed configured-kind projection. Reverse errors
prevent projection; ordinary-rule versus alias selection, transitioned duplicate
keys, reverse depths, empty success, selected-induced graphs, and the existing
unsupported-Platform boundary remain proven.

Query passes 119 tests, commands 19, focused core including Platform, focused
server, and rebuilt CLI daemon symmetry. Formatting, archive, daemon, diff, and
independent review gates pass. The net Rust delta is 21 production and 173 test
lines, 194 total, within 100/300/400 caps. No DICE, state, graph, cache, vendor,
output, or wire owner changed.

Run next `WP-6-m39-cquery-milestone-close-aquery-entry-audit`. Independently
test the canonical M4 exit claim against the complete provider/transition/graph
evidence and, only if it holds, select a read-only M5 aquery design/evidence
entry packet. Do not add further cquery breadth or any M5 production in the
audit.

### Configured-query milestone close ACCEPT (2026-08-10)

`WP-6-m39-cquery-milestone-close-aquery-entry-audit` is **ACCEPT**. Cquery
computes the same Need-aware `ConfiguredNodeAnalysisKey` as analysis and retains
the resulting provider/action/edge-bearing `ConfiguredNodeResult`; it creates no
shadow configured graph. Forward and reverse traversal read those authoritative
classified edges with full configured/null identity, preserving transitions,
aliases, toolchain/delegation topology, error ordering, and daemon restoration.
The public projection is derived from structural `SlugConfiguration` and is
explicitly Slug-native. Exact Bazel checksum, output-path, and ActionKey bytes
remain M9 work. Independent milestone review accepts the gate; unsupported
expression and topology shapes are later breadth rather than M4 blockers.

M4 is accepted. Run next `WP-6-m5-aquery-opaque-token-entry-design`, read-only.
Prove whether one Bazel 9.2 formatter shape can consume the retained action
closure and its owning configured results without re-analysis or a second graph.
Freeze separate structural configuration, configured artifact/path, per-action
execution-platform/exec-group, and Slug action-identity domains; formatter IDs
may only be explicit opaque Slug-native graph-scoped tokens. Keep vendored Buck2
`starlark-rust` unchanged for source semantics and reuse the existing
Buck2-derived query parser for aquery syntax. Add no parser, Rust, tests, wire,
execution, DICE state, exact-byte work, JVM/Java, CI, or vendor changes.

### Aquery opaque-token entry design REPLAN (2026-08-10)

`WP-6-m5-aquery-opaque-token-entry-design` is **REPLAN**. The retained action
closure is the correct sole aquery input, but no Bazel 9.2 formatter can yet
render it truthfully. `ActionSpec`, inputs, outputs, param files, and argv retain
raw strings without typed configured-artifact provenance; a formatter cannot
distinguish an artifact path from an identical user literal. The selected
platform is not retained per action, and a missing topology is not equivalent
to Bazel's default execution platform. No collision-safe Slug action identity
exists, and an exact REAPI digest is a separate protocol/content domain.

Independent review confirms the stop. Run next only
`WP-6-m5-aquery-action-owner-artifact-identity-design`, read-only. Freeze one
compact structural representation for configured target ownership, typed
artifact/argument provenance, default and named-exec-group platform selection,
and action-specific identity material before selecting any formatter. Continue
to reuse unchanged vendored Buck2 `starlark-rust` for BUILD/`.bzl` and the
existing Buck2-derived `QueryExpression` parser for aquery syntax. Add no parser,
Rust, tests, fixture/oracle reruns, formatter, wire, DICE state, execution,
REAPI identity reuse, exact Bazel bytes, JVM/Java, CI, or vendor change.

### Aquery action-owner/artifact identity design REPLAN (2026-08-10)

`WP-6-m5-aquery-action-owner-artifact-identity-design` is **REPLAN**. Intrinsic
action data belongs below analysis, while configured target and platform
identity belong to the configured result; putting `ConfiguredTargetKey` into
`ActionSpec` would invert the crate dependency. A future analysis-owned retained
action wrapper is viable, but current no-toolchain rules do not resolve a
default execution platform, and `run_shell` stringifies declared-file arguments.
Approving the full multi-kind representation now would therefore guess at
unimplemented provenance and named-exec-group semantics.

Independent review accepts a narrower read-only successor. Run next
`WP-6-m5-action-provenance-and-default-exec-platform-design`. Freeze only a
FileWrite vertical: configured-result owner, typed declared output,
content/executable identity material, toolchain-selected or explicit Slug
default execution-platform identity, equality/invalidation, and existing
build/REAPI consumer projection. Every other action kind and named exec group
must remain fail-closed for aquery. Preserve unchanged Buck2 `starlark-rust` and
the existing query parser; add no parser, Rust, tests, oracle rerun, formatter,
wire, DICE state, execution, exact Bazel bytes, JVM/Java, CI, or vendor change.

### FileWrite provenance/default-platform design ACCEPT (2026-08-10)

`WP-6-m5-action-provenance-and-default-exec-platform-design` is **ACCEPT** for
one narrower structural vertical. The accepted `f00e99db` FileWrite runs in the
default exec group but its rule requires `//:demo_type`, so the existing
`ToolchainTopology::selection` already co-retains exact P0/P1 platform identity
with the configured result. Its owner key, intrinsic Write content/executable
bit, and typed file output are likewise present. Ordinary no-toolchain actions
still lack a platform and remain unsupported rather than guessed.

Run next `WP-6-m5-toolchain-filewrite-configured-action-view-implementation`.
Add only an analysis-owned borrowed configured-action view over the existing
result. Admit configured-owner `ActionKind::Write` with exactly one file output,
default exec group, selected toolchain platform, and empty argv/input/tool/param
surfaces; reject every other shape, named group, or missing selection. Prove
structural C0/C1/C0, P0/P1/P0, content, and output-path change/restoration from
the retained evidence. Add no aquery command/formatter, token/hash/ActionKey,
path projection, parser/vendor change, retained state, DICE key, execution,
wire, JVM/Java, or CI. Caps are 110 production, 140 tests, 250 total; the final
review corrected and accepted the fixture's toolchain-backed scope.

### Toolchain-backed configured FileWrite view ACCEPT (2026-08-10)

`WP-6-m5-toolchain-filewrite-configured-action-view-implementation` is
**ACCEPT** at 84 production and 136 test lines. `ConfiguredNodeResult` now
exposes an allocation-free borrowed view of its configured owner, intrinsic
Write spec, single typed file output, default exec group, and selected toolchain
execution platform. It rejects non-Write and ambiguous execution surfaces,
named groups, missing owners/platforms, and unsupported populated fields.
Zero-action closure nodes correctly yield an empty iterator without requiring a
platform. No action or graph state is copied or added.

Structural tests mirror the accepted C0/C1/C0, P0/P1/P0, content A/B/A, and
output-path A/B/A relationships and exercise fail-closed shapes. All 38
`slug_analysis_v2` tests pass; formatting, diff checks, caps, and independent
final review pass. Vendored Buck2 `starlark-rust` and the Buck2-derived query
parser remain unchanged.

Run next only `WP-6-m5-toolchain-filewrite-text-formatter-design`, read-only.
Freeze one text formatter's exact non-identity fields and explicitly Slug-native
configuration/path/action-token projections over this view before any command,
wire, or formatter Rust. Preserve the accepted identity change/restoration
relations, exact REAPI digest separation, ordinary no-toolchain/non-Write
fail-closed boundaries, and the no-new-parser/JVM/CI stops.

### Toolchain FileWrite text formatter design REPLAN (2026-08-10)

`WP-6-m5-toolchain-filewrite-text-formatter-design` is **REPLAN** before Rust.
The existing Slug configuration projection and configured-output owner are
valid formatter inputs, and the proposed names correctly avoided claiming a
Bazel checksum or `ActionKey`. However, a FileWrite token over content,
executable bit, and platform label deliberately omitted configured owner and
typed output to mimic Bazel ActionKey change relations. It therefore cannot be
called a collision-safe Slug action identity: distinct owned actions can alias.
The canonical platform label also cannot detect a same-label semantic platform
mutation because that structure is not retained in the current view.

Run next only `WP-6-m5-filewrite-semantic-identity-and-formatter-token-design`,
read-only. Separate complete owner/output/platform-bearing Slug semantic action
identity from any graph-local, explicitly non-identity formatter token. Freeze
the retained structural platform fact needed to fail closed on same-label
changes, or select one bounded prerequisite. Reuse the existing configuration
projection/configured-output owner and unchanged Buck2 parser stacks; preserve
REAPI/CAS separation. Add no formatter, command/wire, Rust, tests, oracle rerun,
execution, DICE state, exact Bazel bytes, JVM/Java, CI, or vendor change.

### FileWrite semantic identity/token separation design REPLAN (2026-08-10)

`WP-6-m5-filewrite-semantic-identity-and-formatter-token-design` is **REPLAN**
for one structural prerequisite. The existing action closure already contains
every candidate platform result through configured `CandidateExecutionPlatform`
edges. A selected result therefore supplies its full configured key and ordered
constraint edges without a second graph. But Platform analysis currently drops
coerced `exec_properties`; a same-label property mutation is invisible after
loading, so neither a complete semantic identity nor an honest fail-closed
formatter can proceed.

Run next `WP-6-m5-platform-semantic-fact-retention-and-resolved-filewrite-view`.
Project the existing normalized ordered native `exec_properties` value into the
Platform result, reject nondefault unrepresented parents/legacy properties/
flags/required-settings/toolchain-type controls, and expose a borrowed core view
that resolves each admitted FileWrite selected key to exactly one exec-configured
Platform result in the existing action closure. Keep semantic identity and any
graph-local formatter token separate and deferred. Add no hash, token, formatter,
command/wire, second graph, DICE key, execution, REAPI reuse, parser/vendor,
JVM/Java, or CI change. Reserved independent review accepts 220 production / 220
test caps and requires the complete platform-field fail-closed classification.

### Platform semantic fact and resolved FileWrite view ACCEPT (2026-08-10)

`WP-6-m5-platform-semantic-fact-retention-and-resolved-filewrite-view` is
**ACCEPT** at 206 production and 215 test net lines. Platform analysis now
retains normalized key-ordered `exec_properties` in the existing configured
result and rejects nondefault unrepresented platform-semantic fields. The core
view resolves each admitted FileWrite's exact selected platform from the
existing action closure and exposes the ordered
Platform-to-ConstraintValue-to-ConstraintSetting chain without another graph
or retained index.

Focused analysis and core tests cover property reordering, mutation and
restoration, nondefault legacy-property rejection, exact unique closure
resolution, malformed chain shapes, and a resolved setting A/B/A lifecycle.
Formatting, diff and archive checks pass, and independent final review accepts
the corrected full-chain regression. The Buck2-derived Starlark and query
parsers remain unchanged; no hash, identity token, formatter, command, wire,
execution, REAPI reuse, DICE state, JVM/Java, or CI surface was added.

Run next only `WP-6-m5-filewrite-semantic-identity-design-retry`, read-only.
Freeze one collision-safe, tagged structural FileWrite identity over configured
owner, typed output, Write material, default exec group, selected platform key,
normalized exec properties, and the complete ordered constraint chain. Keep
this semantic identity distinct from any graph-local formatter token, Bazel
ActionKey, and REAPI digest. Select at most one bounded implementation successor
or `REPLAN`; add no Rust, tests, hashing, formatter, command/wire, execution,
DICE state, parser/vendor, exact Bazel-byte work, JVM/Java, or CI change.

### FileWrite semantic identity design retry ACCEPT (2026-08-10)

`WP-6-m5-filewrite-semantic-identity-design-retry` is **ACCEPT**. The first
semantic identity is the complete Slug-owned canonical byte sequence itself,
not a checksum or digest. Its grammar starts with `slugact\0` and a big-endian
schema version, then uses tagged, u64-length-framed fields for the configured
owner; File output kind and relative path; FileWrite mnemonic, content, and
executable bit; explicit default exec group; selected platform configured key;
key-ordered exec-property pairs; and the index-ordered configured
ConstraintValue/ConstraintSetting pairs.

Every configured key encodes its canonical label including repository-mapping
identity plus the existing structural `SlugConfiguration::canonical_bytes`.
Legacy/projected configuration strings fail closed. Equality is exact canonical
byte equality; no 32-byte digest may stand in for semantic equality. Any future
domain-separated digest or graph-local formatter token is only a presentation
projection and remains distinct from this identity, Bazel ActionKey/checksum,
configured output-root spelling, and REAPI/CAS digests.

Run next `WP-6-m5-filewrite-semantic-identity-implementation`. Add only the
framed encoder and immutable identity over the accepted resolved FileWrite
view, rejecting every non-structural configured key. Reuse the configuration
canonical-byte pattern and immutable compact ownership; record the utility
decision in Stage 9. Prove framing, structural configuration, owner/output,
Write material, platform property, and constraint-chain change/restoration.
Add no formatter token/projection digest, aquery command/root/wire, execution,
DICE state, parser/vendor, exact Bazel bytes, JVM/Java, or CI. Caps: 220
production / 180 tests / 420 total net lines, including bookkeeping.

### FileWrite semantic identity implementation ACCEPT (2026-08-10)

`WP-6-m5-filewrite-semantic-identity-implementation` is **ACCEPT** at 212
production and 165 test net lines (377 Rust, 412 including bookkeeping). The
new immutable `FileWriteSemanticIdentity` is exact `Arc<[u8]>` equality over
the accepted `slugact\0` versioned, tagged, length-framed grammar. Configured
keys use canonical labels with mapping provenance and complete structural Slug
configuration bytes; legacy configurations fail closed.

Resolved-view fields are private behind read-only accessors, leaving the
validated closure resolver as their sole constructor. Focused tests distinguish
framing, owner, structural configuration, typed output, content, executable
bit, selected platform P0/P1/P0, normalized platform properties, and constraint
setting A/B/A. Formatting, focused identity and resolver tests, diff and archive
checks, caps, and independent final review pass. No digest, formatter token,
command/wire, execution, DICE state, parser/vendor, JVM/Java, or CI surface was
added.

Run next only `WP-6-m5-filewrite-aquery-text-formatter-design-retry`, read-only.
Reconcile the pinned Bazel 9.2 FileWrite text evidence with the accepted
configured action view and exact Slug semantic identity. Freeze formatter field
order and exact-versus-Slug-native classifications, and define any necessary
short graph-local display token as an explicitly non-identity projection over
the complete canonical identity. Select at most one bounded implementation
successor or `REPLAN`; add no Rust, tests, oracle rerun, command/root/wire,
execution, DICE state, parser/vendor, exact Bazel identity bytes, JVM/Java, or CI.

### FileWrite aquery text formatter design retry ACCEPT (2026-08-11)

`WP-6-m5-filewrite-aquery-text-formatter-design-retry` is **ACCEPT** for one
per-action block over an already resolved FileWrite semantic view. In order,
the frozen block is:

```text
action 'Writing file {declared-output}'
  Mnemonic: FileWrite
  Target: {aquery-label}
  Configuration: slugcfg-v1:{64 lowercase hex}
  Execution platform: {aquery-label}
  SlugActionToken: slugact-display-v1:{64 lowercase hex}
  Inputs: []
  Outputs: [bazel-out/slugcfg-v1-{64 lowercase hex}/bin/{declared-output}]
  IsExecutable: false
```

The header, two-space indentation, field order, punctuation, `FileWrite`
mnemonic, empty inputs, lowercase boolean, declared-output suffix, and aquery
label spelling are exact Bazel-shaped text. Aquery labels spell the main repo
as `//...` and canonical external repos as `@@repo//...`; repository-mapping
provenance remains in semantic identity but is not formatter text. The
configuration value and configured output root are the existing explicitly
Slug-native full configuration projections, not Bazel mnemonic/checksum or
`bazel-out` identity. `SlugActionToken` deliberately replaces `ActionKey`.

`slugact-display-v1:` is a full 32-byte lowercase-hex BLAKE3 derive-key
projection over the complete `FileWriteSemanticIdentity` canonical bytes with
context `slug.v2.filewrite.aquery-display.v1`. It is not truncated, retained,
or admitted for equality, DICE/cache keys, configured paths, Bazel checksum or
ActionKey, or REAPI/CAS identity. Exact canonical bytes remain the only action
semantic identity; the token is presentation only.

Run next `WP-6-m5-filewrite-aquery-text-formatter-implementation`. Add only the
projection and per-action formatter in core over the existing resolved view.
Admit exact mnemonic `FileWrite`, a main-repository configured owner, one
already validated relative File output, default exec group, and
`is_executable = false`; fail closed otherwise. Prove exact baseline text and
C0/C1/C0, P0/P1/P0, content, output-path, platform-property, and constraint
change/restoration token relations. Reuse the accepted oracle without rerun or
growth and record the hash-utility decision in Stage 9. Add no container
ordering/join/final newline, `--include_file_write_contents`, executable Write,
external owner, aquery parser/command/root/wire, retained state, DICE key,
execution, REAPI reuse, exact Bazel identity bytes, JVM/Java, vendor, or CI.
Caps: 150 production / 210 tests / 400 total Rust net lines, plus the bundled
design and scheduling bookkeeping.

### FileWrite aquery text formatter implementation ACCEPT (2026-08-11)

`WP-6-m5-filewrite-aquery-text-formatter-implementation` is **ACCEPT**.
Core now formats one already resolved default-exec-group FileWrite action as
the frozen Bazel-shaped block. Main-repository target and platform labels use
aquery spelling, the existing full Slug configuration projection owns the
configuration and configured-output components, and
`slugact-display-v1:{64 lowercase hex}` is a full BLAKE3 derive-key projection
over the canonical FileWrite identity. It remains presentation-only and is not
retained or reused for semantic equality, DICE/cache keys, Bazel checksums or
ActionKey, configured paths, or REAPI/CAS identity.

The formatter fails closed for legacy configuration, external owner,
non-`FileWrite` mnemonic, non-Write action, executable Write, named exec
group, unsafe output text, unresolved platform facts, and other shapes already
rejected by the resolved-view producer. Focused tests freeze the exact text and
prove configuration, owner, output, content, platform, platform-property, and
constraint token change/restoration. AI cleanup removed a one-call test helper,
tightened delimiter/control rejection, and kept all display allocations
request-local.

Validation accepted:

- `cargo test -p slug_core_v2 aquery`: 4 passed;
- the focused semantic-identity lifecycle test passed;
- `cargo check -p slug_cli_v2 -p slug_server_v2` passed;
- formatting and `git diff --check` passed;
- raw Rust net additions are 105 production / 120 tests / 225 total, within
  150 / 210 / 400;
- full core unit tests were 171 passed / 1 unrelated documented external
  visibility diagnostic-wording baseline failure;
- the runtime integration target was 12 passed / 1 unrelated legacy
  `PathObservationEpochKey` injected-compute failure; and
- the archive script passed every V2 layout check but this checkout lacks the
  historical `slug-v1-archive` tag/branch and recorded archive commit.

The accepted Bazel 9.2 evidence was reused without oracle rerun or fixture
growth. Independent design and final review returned `ACCEPT` after the one
allowed correction added explicit named-exec-group rejection. No container,
command/root/wire, parser/vendor, execution, retained DICE state, REAPI reuse,
exact Bazel identity-byte work, JVM/Java, or CI entered the packet.

Run next `WP-8-m5-filewrite-aquery-command-root-design`, owned by Stage 8.
Reconcile the accepted core formatter and action closure with the existing
aquery command/parser/protocol scaffolding, then freeze at most one bounded
single-root text consumer or return `REPLAN`. The packet is read-only: add no
Rust, tests, fixture growth, oracle rerun, command/wire implementation,
execution, retained state, parser/vendor, exact Bazel identity bytes, JVM/Java,
or CI.

### Embedded default-test-toolchain closure audit REPLAN (2026-08-12)

Pinned `@bazel_tools//tools/test:all` expands to 20 rules: eight filegroups,
one sh_binary, two aliases, three toolchains, one toolchain type, one empty
ToolchainInfo rule, one bool build setting, and three config settings. Its
loaded labels reach `src/conditions:windows`, `platforms//os`, rules_shell's
toolchain/runfiles targets, and the generated remote-coverage repository.

Slug analysis currently canonicalizes and loads only root registrations,
rejects every external registration, and resolves only root toolchain types
and packages. No bounded Stage 6 correction can precede the hidden
`bazel_tools` module/mapping owner and Stage 4 contextual package load.
Configured Test toolchain and TestRunner semantics remain deferred.


The module-injection audit further pins that Bazel's registered toolchain and
execution-platform consumers iterate modules from the selected dependency
graph using each module's full contextual mapping. A callerless embedded MODULE
value may preserve registration order, but it cannot activate Stage 6
registration or selection before the one combined graph exists.

### M7A bootstrap action-owner context audit accepted (2026-08-18)

The read-only audit from `35e84646` selects one uniquely smaller just-in-time
Bazel 9.2 evidence prerequisite before the immutable Rust owner design. M1 is
complete; no lower observation/publication owner is missing.

Live `ActionSpec` retains intrinsic action fields and an optional exec-group
string, while `ConfiguredNodeResult` separately retains the action slice and
one `ToolchainTopology`. The borrowed FileWrite view reconstructs a single
topology-selected platform, exposes only the default group, and rejects named
groups and per-action execution fields. Identity, aquery and REAPI consume
that reconstruction. The natural future owner is therefore an analysis-owned
immutable configured-action row, not a new DICE key or a configured identity
embedded into build-API `ActionSpec`.

Existing evidence cannot freeze that row. `actions-api-basic` supplies action
summaries, `toolchain-resolution-first-platform` has no actions, and the
dedicated `exec-groups-action-platform` fixture is an ungenerated one-action
scaffold with an empty expected command list.

Run only `WP-1-6-7A-exec-group-action-owner-context-evidence`. Replace that
fixture with one Bazel 9.2 single-owner, two-action default/named-group
discriminator over distinct platforms/toolchains/properties and ordered named
platform A/B/A restoration. Use a deterministic non-summary aquery shape.
No Rust, harness, other fixture, rules_rust breadth, applied aspect, execution,
M8, M7B or M9 work.

### M7A exec-group action-owner evidence accepted (2026-08-19)

The Bazel 9.2 fixture is now generated and cleanly replayed. One owner creates
default and named compile actions in declaration order. Default remains on its
distinct platform/key through every row. The compile action is cold/warm
stable, changes only its opaque key for a same-platform property edit, restores
that key, moves to the second compatible platform after registration reorder,
and restores its original platform/key. Exact key bytes remain M9.

Pinned `RuleContext#getActionOwner(execGroup)` source ties the selected group
to configuration, aspect descriptors, merged properties and execution
platform. The accepted fixture has explicit absent aspect provenance; it does
not admit applied aspects, broader action kinds, rules_rust or execution.

Run next only
`WP-6-7A-immutable-configured-action-owner-context-design`, docs-only. Freeze
one analysis-owned configured-action row from the accepted evidence before any
Rust implementation or public named-group activation.

### Immutable configured-action owner-context design accepted (2026-08-19)

The natural owner is analysis finalization, not lower `ActionSpec`, a DICE key
or a later command projection. `CtxActions` continues to register intrinsic
specs in declaration order. The mode-aware toolchain preparation already owns
platform/toolchain selection and the native package facts; it will additionally
compute the selected Platform analysis through the matching family and prepare
one compact action context before Starlark evaluation. The evaluator moves its
validated specs through a pure finalizer before returning `ConfiguredNodeResult`.
Platform analysis occurs immediately after raw package selection and before
selected toolchain-implementation analysis; any Platform terminal suppresses
both the implementation and rule children.

Replace the retained raw spec slice with one configured-action slice. Each row
owns one intrinsic spec and shares an `Arc<ConfiguredActionOwnerContext>` for
its owner/group. The context structurally retains configured owner, explicit
default/named group, selected exec-configured platform, normalized merged
properties, ordered constraint value/setting keys, exact toolchain selection
plus selected `ToolchainInfo` marker/provider projection, and explicit absent
aspect provenance. Preparation, Starlark and final rows share that compact
context; no full selected implementation/provider Result Arc is retained. The
admitted production path supplies
one default context; named contexts and platform/target/group merge precedence
are private representation/finalizer proof only. Public `rule(exec_groups=...)`,
action `exec_group=`, target/group property ingestion and applied aspects remain
unsupported.

Selected Platform analysis provides the exact property Arc and ordered value
edges; already loaded native packages provide each setting identity. The
prepared Platform result, registry and lookup scratch are compute-local. The
existing `ToolchainTopology` remains its independent analysis/edge fact, but no
action consumer reads it and no second candidate/topology collection is added.
Same-group actions share one context Arc; there is no retained context map,
platform result Arc or second action graph.

Core's resolved FileWrite view becomes a direct borrow of this row. Remove its
projection-time platform/constraint closure lookup and temporary constraint
vector. Semantic identity, text aquery and unchanged `FileWriteReapiPlan` read
the identical retained group/platform/property/constraint data. Dependency
platform/toolchain/constraint nodes remain exactly once in the recursive action
closure through the accepted configured edges; public FileWrite bytes/order and
REAPI wire semantics do not change. Named-group identity is structurally tagged
but remains nonactivated by Starlark, commands, aquery, REAPI and execution.

Run next only
`WP-6-7A-immutable-configured-action-owner-context-implementation`. Exact Rust
authority is the four analysis production files and two existing analysis proof
files; core `runtime/{dice.rs,file_write_identity.rs,mod.rs}` and its build proof;
and the existing REAPI proof only. Caps are 788 production, 860 test, 1,648
aggregate and 25,835 combined physical lines. Prove declaration order and
same-context pointer sharing; default/named distinct contexts; configuration,
platform, property and toolchain marker/provider A/B/A; Platform-before-
implementation suppression; failure-before-retention and diagnostic
precedence; matching-family selected-Platform Need/outer/error; removal of
projection reconstruction; identical identity/aquery/REAPI consumption; exact
default action behavior; and bounded retention/cap accounting.

### Immutable action-owner absence correction REPLAN (2026-08-19)

The first implementation cannot be accepted under `460dea72`. Full analysis
validation exposed six existing action tests failing at finalization. Ordinary
action-producing rules without a required toolchain have no prepared context,
and a selected toolchain implementation can likewise register actions while
its own analysis has no requirement. The frozen mandatory concrete
`ToolchainSelection` plus marker is therefore not total. Inventing either value
would corrupt semantic identity; rejecting the actions would regress exact
intrinsic `ActionSpec` behavior.

Run only `WP-6-7A-action-owner-context-absence-correction-design`, docs-only.
Retain the eleven-file Rust candidate non-writable. Freeze one explicit
execution-state distinction inside the immutable owner context:

- selected toolchain: selected Platform fact/constraints plus exact compact
  selection and marker;
- selected platform only: the same Platform projection with explicit absent
  toolchain, admitted only for the existing unique-candidate topology; and
- unresolved default: owner/group/aspect only, with no guessed platform,
  properties, constraints, selection or marker.

Production always supplies exactly one Default context, selected or explicitly
unresolved. Intrinsic actions remain exact and ordered. Configured FileWrite
projection continues to require a selected platform: the existing sole-
candidate case remains usable and an unresolved/ambiguous action remains
intrinsically retained but unprojectable. Named contexts remain private proof.
Platform analysis stays matching-family and terminal-before-rule wherever a
selected platform exists.

The retry must also split the oversized root-toolchain routine into bounded
Platform, implementation and orchestration helpers below 200 lines without a
new key or retained state. All other accepted ownership, memory, identity,
aquery and REAPI contracts remain unchanged. After independent design ACCEPT,
schedule only
`WP-6-7A-immutable-configured-action-owner-context-implementation-retry` with
the same eleven files. Raise only `result.rs` to +300/730 physical and aggregate
to 848 production, 860 tests, 1,708 semantic and 25,895 physical. No Rust,
oracle, public named-group, M7A/M8/M7B or M9 activation is authorized now.

### Action-owner absence correction accepted (2026-08-19)

The three-state correction is implementation-ready. `SelectedToolchain`
preserves the accepted full platform/toolchain context. The existing unique-
candidate topology may produce `SelectedPlatformOnly` after matching-family
Platform analysis, with explicit absent toolchain. Rules with no selected
candidate receive `UnresolvedDefault`, retaining owner/group/aspect and exact
intrinsic action order without a fabricated platform, property, constraint,
selection or marker. Configured FileWrite projection still requires a selected
platform, so the former sole-candidate success and unresolved rejection remain
unchanged.

Run only
`WP-6-7A-immutable-configured-action-owner-context-implementation-retry` from
Rust base `51127df8`, semantic design `460dea72` and correction `11934418`.
Authority remains the same eleven Rust files. Raise only `result.rs` to
+300/730 physical and aggregate caps to 848 production, 860 tests, 1,708
semantic and 25,895 physical. Split the root-toolchain owner into Platform,
implementation and orchestration helpers below 200 lines. Preserve every other
owner/order/memory/identity/aquery/REAPI/proof boundary and schedule only a
docs-only M7A next-owner audit after ACCEPT.

### Immutable configured-action owner context accepted (2026-08-19)

Implementation `cb5073e0` completes the corrected owner. Analysis now retains
one configured-action slice and one shared compact context per group, with
explicit `SelectedToolchain`, `SelectedPlatformOnly` and `UnresolvedDefault`
states. Selected Platform analysis uses the matching key family before any
selected implementation or rule evaluation. FileWrite identity, text aquery
and REAPI consume the retained row without topology reconstruction. Intrinsic
zero-toolchain actions remain ordered and exact; sole-candidate actions remain
projectable; unresolved actions retain no fabricated platform or toolchain.

Accounting against `51127df8` is +397 production, +538 test, +935 aggregate
semantic and 24,807 physical lines across the exact eleven files. Full analysis
passes 4+11+10+21+4 tests. Focused core/REAPI, workspace check, fmt, diff-check,
cap accounting and independent ownership/retention/cleanup review pass; only
the already recorded inherited core baselines remain.

Run only `WP-6-7A-post-owner-context-bootstrap-closure-owner-audit`, docs-only.
Trace the exact remaining Stage 10 bootstrap closure across repository sources,
rules_rust/provider/toolchain semantics, action kinds/Args/paramfiles/tools/
runfiles/input trees, normalized aquery and REAPI execution/cache/
materialization. Rank natural owners and accepted evidence without assuming
that one umbrella owner or implementation is next.

Return exactly one bounded owner design, one uniquely smaller just-in-time
Bazel 9.2 evidence prerequisite, or formal REPLAN. Write authority is only the
canonical plan, current manifest, this Stage 6 plan and the routing log. Rust,
tests, fixtures, oracle generation, Cargo/BUILD, public named groups, applied
aspects, bootstrap-only paths, M7A closure, M8, M7B and M9 are stopped. Preserve
M7A -> M8 -> M7B and require an independently accepted design before code.

### Post-owner-context bootstrap closure audit accepted (2026-08-19)

The read-only audit from `86d23ca8` selects one uniquely smaller just-in-time
Bazel 9.2 evidence prerequisite before the external rules_rust toolchain owner
design. Live analysis still rejects external topology registrations, native
references and registered toolchains. The accepted direct nonroot-registration,
root first-platform and immutable action-owner fixtures do not expose
rules_rust 0.73's module-extension-generated `@rust_toolchains//:all`
expansion/mapping or the selected Rust provider/action context. The only
rules_rust fixture is Bazel 9.1.1/rules_rust 0.71.1 message-shape evidence and
cannot freeze the Stage 10 owner.

Run only `WP-1-6-7A-rules-rust-0.73-toolchain-action-owner-evidence`. Add one
isolated Bazel 9.2/rules_rust 0.73 analysis-only fixture matching Stage 10's
edition-2024 pinned nightly. Pin generated registration expansion, configured
toolchain/provider edges and the Rustc/runfiles action-owner projection, with
unchanged warm reuse and edition 2024 -> 2021 -> 2024 restoration. Use anchored
query/cquery/text-aquery output; message-shape counts are insufficient.

Authority is only canonical/current/this Stage plus the eight new fixture
files named in the manifest. Handwritten fixture content is <=350 physical
lines, generated JSON <=3,000 lines/200 KiB, and aggregate physical growth
<=3,750 lines. Do not edit the harness, `rules-rust-basic`, Stage 10, Rust,
Cargo or nonfixture BUILD metadata. Do not execute actions or claim REAPI,
cache, run/test, sysroot-closure or exact ActionKey bytes. Generate once, shut
down Bazel, replay from no server, and REPLAN if exact bounded output cannot be
anchored. After evidence ACCEPT, design only
`WP-6-7A-external-rules-rust-toolchain-owner-design`.

### rules_rust 0.73 toolchain/action-owner evidence accepted (2026-08-19)

Evidence commit `b7390392` adds exactly the isolated eight-file Bazel 9.2 /
rules_rust 0.73 fixture. It pins 78 generated registration labels, the selected
canonical nightly tools implementation, 25 configured edges, exact CrateInfo
owner/type/edition/root/output/dependency projection, and the restricted
Rustc/SymlinkTree/RunfilesTree owner relationship. Edition 2024 -> 2021 ->
2024 changes and restores the provider projection and opaque Rustc ActionKey.

Fresh generation and no-server no-update replay pass. Authored content is 228
physical lines; generated oracle is 430 lines and 196,649 bytes; aggregate is
658 lines. The fixture contains no home path, stale-server diagnostic or
credential pattern. It claims exact generated mapping/provider/action
relationships but not opaque identity bytes, execution, cache, REAPI, run/test
or full sysroot breadth.

### External rules_rust owner formal REPLAN (2026-08-19)

Do not freeze the analysis owner yet. Live root apparent repository mapping
still crosses carrierless selected extension/module graph owners, and the
accepted observed package loader starts only from direct-local/builtin root
routes. Extension-generated `@rust_toolchains` and canonical
`@@rules_rust++rust+rust_toolchains` therefore have no complete reusable
mapping -> definition -> route/source -> package epoch. Private core apparent-
route keys cannot be called from lower analysis/loading without dependency
inversion; a mapping-only sibling would leave the same route/package gap.

Run only `WP-6-7A-root-generated-repository-observation-frontier-design`,
docs-only. Freeze the minimal matching-family bzlmod/loading sibling set that
carries the generated apparent mapping through canonical repository definition,
route/source and generated BUILD package loading. Each sibling may retain only
one natural Result Arc plus one compact epoch; parents retain no child carrier,
map/frontier/event scratch, cache, store, interner, lock or task. Union Complete
epochs left-first before semantic inspection, preserve exact first Arcs, and
freeze outer > compatible Need > semantic ordering with explicit REPLAN for
incompatible Need kinds.

The design audit must also trace the carrierless prepared-input, pure-extension
invocation/event and repository-instantiation owners in `bzl_module.rs`,
`module_extension.rs` and `module_extension_repository_instantiation.rs`; the
validation key already computes them and cannot bypass them without duplicating
evaluation/event semantics. The ten existing candidate files total 26,768
physical lines. Freeze the exact necessary subset and measured per-file caps,
or REPLAN if another owner/file is required. The provisional whole-candidate
envelope is +1,200 production, +900 tests, +2,100 aggregate and <=29,200
combined physical lines; it is not implementation authority.

Prove the accepted rules_rust mapping and selected tools implementation, exact
legacy parity, every Need/outer/semantic prefix, joined full-batch order, exact
epoch Result-Arc identity, both family directions, child-only events/warm
suppression, cancellation/recovery and root/registry/extension A/B/A. STOP Rust,
analysis/toolchain/action activation, core ownership inversion, a partial
mapping carrier, duplicate retained state, Stage 10 and M7A/M8/M7B/M9 closure.
After design and implementation ACCEPT, return directly to the docs-only
`WP-6-7A-external-rules-rust-toolchain-owner-design`.

### Generated-repository frontier first-owner REPLAN (2026-08-19)

The design audit at `d1755008` traced past selected graph into the actual root
MODULE-files owner. `HostRootModuleFileObservationKey` already owns exact root
MODULE/include path observations and the sole local MODULE event batch, but its
private semantic value drops the evaluated `extension_usages`. The legacy
`RootModuleFilesKey` separately computes `VisibleLockfileKey`, whose completed
workspace read has no retained `PathObservationEpoch`. Consequently neither an
observed selected graph nor extension input request can start from an exact
root MODULE + lockfile prefix without reconstructing or rereading state.

Run only `WP-6-7A-root-module-files-observation-completion-design`, docs-only.
Freeze a two-file matching Legacy/Observed `RootModuleFiles` driver in
`host_module.rs` and `module_eval.rs`: retain the already-evaluated extension
usages in the private root-module value; observed visible-lockfile handling
must preserve legacy mode-first semantics, use the accepted Host FileBytes
observation only when the mode reads the file, and union root then lockfile
epochs left-first before semantic inspection. The parent remains eventless and
retains exactly one local `RootModuleFiles` Result Arc plus one compact epoch.

Provisional implementation caps from 4,531/5,451 physical baselines are +80
production/+120 tests for `host_module.rs`, +180 production/+220 tests for
`module_eval.rs`, <=600 aggregate semantic lines and <=10,590 combined physical
lines. STOP any third Rust file, selected-graph/registry/extension/package or
analysis activation, direct Host read, duplicate collection/state/event owner,
or parity drift. After design and implementation ACCEPT, schedule only the
docs-only selected-module-graph observation-frontier design.

### Root MODULE-files observation design accepted (2026-08-19)

Design `335cfa45` freezes `RootModuleFilesKey` as the first complete aggregate
owner. The implementation may edit only `host_module.rs` and `module_eval.rs`.
It must move the already-evaluated extension-usage Arc into the private root
child value, add the private observed RootModuleFiles sibling/carrier, preserve
legacy evaluation -> visible-lockfile order, and make the observed lockfile
projection mode-first with no file activation in Off mode. Complete epochs are
merged root then lockfile left-first before semantic inspection; Need/typed
outer is immediate and carrierless; the parent remains eventless.

Caps are host-module +80 production/+120 tests/4,740 physical, module-eval
+180 production/+220 tests/5,850 physical, <=600 aggregate semantic and
<=10,590 combined physical. Retain only one local RootModuleFiles Result Arc
plus compact epoch; no third file, selected graph/registry/extension/package or
analysis activation, direct Host read, extra collection/state/event owner, or
cap excess. After ACCEPT, schedule only the docs-only selected-module-graph
observation-frontier design.

### Root MODULE-files implementation cap/proof REPLAN (2026-08-19)

The retained two-file candidate compiles and full `slug_bzlmod_v2 --lib`
passes 426/426. Ownership, matching-family order, union-before-semantic,
child-only events and compact Result-Arc+epoch retention are sound. Measured
against `335cfa45`, `host_module.rs` is +4 production at 4,535 physical lines;
`module_eval.rs` is 315/30, +285 production at 5,736 physical lines and exceeds
its frozen +180 production cap. Its approximately 66-line observed-lockfile
helper and 128-line shared driver are cohesive and already below the 200-line
helper gate. A 105-line forced reduction would require macro compression,
ownership duplication or abandoning the accepted shared driver.

The retry must also replace the candidate's Debug-derived
`HostRootModuleFileError` and lockfile `HostFileError` strings with explicit
semantic projections. Equivalent command-policy, validation, evaluation,
lockfile-mode/read/parse terminals preserve exact legacy messages. Slug-native
Need/typed outer and Host-only source-kind/path errors remain structurally
distinct and must use stable explicit messages. Real legacy/observed terminal
comparisons discriminate this boundary.

Run only
`WP-6-7A-root-module-files-observation-proof-cap-correction-design`, docs-only.
Retain the Rust candidate non-writable. The corrected retry keeps
`host_module.rs` at +80 production/+120 tests/4,740 physical and raises only
`module_eval.rs` to +340 production/+300 tests/6,100 physical; aggregate caps
are +840 semantic and 10,840 physical. Preserve every owner, algebra, event,
memory, family and deferred-boundary requirement from `335cfa45`. STOP Rust,
another file/owner/caller, Debug error projection, direct Host reads, selected
graph/extension/package/analysis activation, cap excess and milestone closure.
After independent design ACCEPT, resume exactly one implementation retry; after
implementation ACCEPT, design only the selected-module-graph frontier.

### Root MODULE-files cap/proof correction accepted (2026-08-19)

Correction `47746115` accepts the measured two-file envelope without changing
the owner. Run only
`WP-6-7A-root-module-files-observation-completion-implementation-retry` from
Rust/design base `335cfa45`. Authority remains exactly `host_module.rs` and
`module_eval.rs`; caps are +80 production/+120 tests/4,740 physical and +340
production/+300 tests/6,100 physical, <=840 aggregate semantic and <=10,840
combined physical.

Preserve the matching Legacy/Observed driver, extension-usage transfer,
mode-first lockfile selection, root-then-lockfile exact-Arc epoch algebra,
child-only events and one Result Arc+epoch retention. Replace Debug-derived
root/lockfile semantic strings with explicit exact legacy-equivalent
projections and stable Slug-native Host-only path/source-kind messages; prove
both with real terminal comparisons. No third file/caller/owner, direct Host
read, selected-graph/extension/package/analysis activation or milestone close.
After ACCEPT, design only the selected-module-graph observation frontier.

### Root MODULE-files observation implementation accepted (2026-08-19)

Implementation `a3efa1b7` completes the first aggregate root prefix. The private
matching-family driver retains the already-evaluated extension-usage Arc, reads
the visible lockfile mode-first through the observed Host FileBytes owner, and
associates root then lockfile exact Arcs before semantic inspection. Equivalent
legacy errors remain exact; Host-only terminals have stable explicit messages.
The parent is eventless and retains only one local Result Arc plus compact
epoch.

Final accounting against `335cfa45` is +76 production/+119 tests in
`host_module.rs`, +303 production/+298 tests in `module_eval.rs`, +796
aggregate semantic lines and 10,778 physical lines. Focused 2/2 and full bzlmod
428/428 pass. Loading 138/138 and query 53/53 remain green; core retains only
the accepted inherited visibility-wording baseline. Formatting, diff hygiene,
archive disposition, retention/cleanup and independent terminal review pass.

### Selected-graph frontier first-child REPLAN (2026-08-19)

Do not freeze the selected graph sibling yet. `HostSelectedModuleGraphKey`
starts from the now-complete `RootModuleFilesKey` but repeatedly computes
carrierless `HostEffectiveModuleOverrideKey` before every discovered-module
horizon. That same effective-override owner is the first shared child of module
source preparation, nonregistry preflight and selected repository-definition
projection. Bypassing it would duplicate command/root override precedence and
lose the accepted root epoch.

`HostDiscoveredModuleKey` also still crosses carrierless registry preparation
and nonregistry closure branches. Those remain later frontier work; folding
them into this first child would widen ownership and event/lifetime proof before
the shared root prefix is reusable.

Run only `WP-6-7A-effective-module-override-observation-design`, docs-only.
Freeze one private structural observed sibling/carrier in `module_eval.rs`
with one local effective-override Result Arc plus the unchanged root-files
epoch. One Legacy/Observed driver must select only the matching root-files key,
then compute the shared command policy and run the existing pure precedence
projection. Root Need/typed outer is immediate and carrierless; root compute
failure has empty prefix; root semantic, policy compute failure and every
command/root/None terminal retain the root prefix. The parent remains eventless
and retains no child carrier or collection.

Future implementation authority after design ACCEPT is exactly
`module_eval.rs` from the 6,052-line `a3efa1b7` baseline: <=160 production,
<=240 tests, <=400 aggregate semantic and <=6,500 physical lines. Keep every
helper below 200 lines. Prove exact legacy Arc/value/error parity, exact epoch
Arc identity, all terminal prefixes, both family directions, later-owner
nonactivation, eventlessness, warm/cancellation recovery and override A/B/A.

Exact behavior is effective override values/errors/order, normalized command
paths and legacy Results. The sibling/carrier/epoch/typed outer association is
Slug-native. Selected graph/discovered registry/nonregistry modules, extension
evaluation/instantiation, generated repository loading, external rules_rust
analysis/actions, M8/M7B and identity bytes remain deferred.

STOP Rust during design and stop every other file/caller/export, selected graph
or discovered/preparation/repository activation, direct Host read, extra state,
family/order/error/event/retention drift, cap excess or milestone closure.
After design ACCEPT schedule only the bounded effective-override implementation;
after implementation ACCEPT return only to the selected-module-graph frontier
design.

### Effective-module-override observation design accepted (2026-08-19)

Design `c2d1f893` freezes the uniquely smaller shared child. One crate-private
observed key/carrier in `module_eval.rs` forwards the exact accepted
root-files epoch beside one local effective-override Result Arc. A shared
Legacy/Observed driver selects only the matching root-files family, then
preserves command-policy order and the existing root-name/command/root/None
projection. Root Need/typed outer is immediate and carrierless; root compute
failure is empty-prefix and every later semantic terminal retains the root
prefix. The parent remains eventless and compact.

Run only
`WP-6-7A-effective-module-override-observation-implementation` from Rust base
`a3efa1b7` and accepted design `c2d1f893`. Authority is exactly
`module_eval.rs`: <=160 production, <=240 tests, <=400 aggregate semantic and
<=6,500 physical lines from 6,052. Keep helpers below 200 lines. Preserve exact
legacy Result/value/error/command-path behavior, family isolation, epoch Arc
identity, eventlessness, override lifecycle and the full proof/STOP contract.
No caller/export or selected graph/discovered/preparation/repository activation
is authorized. After ACCEPT return only to the docs-only selected-module-graph
frontier design.

### Effective-module-override proof/cap REPLAN (2026-08-19)

The retained one-file candidate preserves the accepted effective-override owner, matching-family root-files selection, command-policy order, exact precedence projection, eventlessness and compact Result-Arc+epoch retention. Focused 2/2 and full bzlmod validation pass. Against `a3efa1b7`, it is +170 production/+236 tests/+406 aggregate at 6,458 physical lines.

Independent proof review found that the four remaining test lines cannot discriminate the parent dependency row, production root Need/typed-outer reduction, command-policy error prefix, exact legacy Result Arc, cold child events, real poll-drop recovery and command-override lifecycle. The smallest production-used reducer also exceeds the +160 production ceiling; synthetic proof or dense macros would violate the proof and cleanup gates.

Run only `WP-6-7A-effective-module-override-observation-proof-cap-correction-design`, docs-only. Retain `module_eval.rs` non-writable. Keep the same one-file retry and raise only its limits to +200 production/+320 tests/+520 aggregate and 6,700 physical lines. The increase may fund only the existing live reducer, legacy Arc projection seam and missing discriminators; it authorizes no semantic/event/retention change, caller or later owner.

STOP Rust, every other file/key/caller/export, direct Host read, new retained state, selected graph/discovery/preparation/repository activation and milestone closure. After independent correction ACCEPT, resume exactly one bounded implementation retry; after its ACCEPT, return only to the selected-module-graph frontier design.

### Effective-module-override implementation retry scheduled (2026-08-19)

Correction `5ebc274a` accepts the measured proof-cap increase without changing
the owner, semantics, event authority or retention contract. Run only
`WP-6-7A-effective-module-override-observation-implementation-retry` from Rust
base `a3efa1b7`, semantic design `c2d1f893` and correction `5ebc274a`.
Authority remains exactly `module_eval.rs`: <=200 production, <=320 tests,
<=520 aggregate semantic and <=6,700 physical lines. The added room may fund
only the production-used pure root reducer, exact legacy projection seam and
the frozen dependency-row, prefix, Arc, event, cancellation and lifecycle
proof. Preserve every original STOP boundary. After independent Rust ACCEPT,
schedule only the docs-only selected-module-graph frontier design.

### Effective-module-override second proof-cap REPLAN (2026-08-19)

The honest corrected proof passes focused 2/2 and preserves the accepted
effective-override semantics, but measures +175 production/+390 tests/+565
aggregate at 6,617 physical lines versus the +200/+320/+520/6,700 limits.
Independent review found no bounded 70-line test reduction without weakening
the live dependency-row, cancellation, event, exact-Arc and lifecycle proof.

Run only `WP-6-7A-effective-module-override-observation-proof-cap-correction-2-design`,
docs-only. Retain `module_eval.rs` non-writable; keep production <=200 and
raise only tests to <=420, aggregate to <=620 and physical to <=6,750. After
independent ACCEPT, retry the same one-file implementation, then return only to
`WP-6-7A-selected-module-graph-observation-frontier-design`, docs-only.

### Effective-module-override implementation retry-2 scheduled (2026-08-19)

Correction `b832736d` accepts the second measured proof-cap increase while
freezing all production semantics and the +200 production limit. Run only
`WP-6-7A-effective-module-override-observation-implementation-retry-2` from
Rust base `a3efa1b7`, semantic design `c2d1f893` and corrections
`5ebc274a`/`b832736d`. Authority remains exactly `module_eval.rs`: <=200
production, <=420 tests, <=620 aggregate and <=6,750 physical lines. Preserve
the complete root/command, dependency-row, reducer, Arc, event, cancellation,
family and lifecycle proof with no semantic/event/retention change. After
independent Rust ACCEPT, schedule only the docs-only selected-module-graph
frontier design, `WP-6-7A-selected-module-graph-observation-frontier-design`.

### Effective-module-override observation implementation accepted (2026-08-19)

Implementation `3d174006` completes the first shared consumer of the accepted
root MODULE-files epoch. Its private matching-family key forwards the exact
root epoch, preserves command-policy precedence and exact legacy Result Arc
projection, remains eventless and retains one local Result Arc plus compact
epoch. Final accounting is +175 production/+420 tests/+595 aggregate at 6,647
physical lines. Focused and full affected validation, the inherited core
visibility-wording baseline, formatting, cleanup/retention and independent
review pass.

### Selected-module-graph frontier materialization-request REPLAN (2026-08-19)

Do not freeze the selected graph sibling yet. Registry discovery uses
`ModuleSourcePreparationKey`, while the nonregistry closure/source path
crosses `RepositoryMaterializationKey` and its carrierless
`RepositoryMaterializationRequestKey`. A complete observed preparation
sibling must preserve its existing nonregistry branch, which would otherwise
duplicate or bypass that request projection. The request key is therefore the
smallest reusable owner of normalized workspace, effective override, canonical
repository and local/immutable request-kind semantics. The builtin discovery
branch adds no Host path epoch.

Run only `WP-6-7A-repository-materialization-request-observation-design`,
docs-only. Freeze one private request sibling/carrier and a matching
Legacy/Observed driver in `source_preparation.rs`. Invalid workspace and
effective compute failure have empty prefixes; effective Need/outer is
carrierless; completed effective semantic and every request projection terminal
retain the exact effective prefix. The parent is eventless and retains one
local request Result Arc plus compact epoch.

Future implementation authority after independent design ACCEPT is exactly
`source_preparation.rs` from 13,747 physical lines, with <=180 production,
<=320 tests, <=500 aggregate semantic and <=14,300 physical lines. STOP every
other Rust file, caller/export, materialization/source/closure/preparation/
discovery/selected-graph activation, direct Host read, retained collection or
state, event drift and milestone closure. After implementation ACCEPT, return
only to the docs-only selected-module-graph observation-frontier design.

### Materialization-request observation design accepted (2026-08-19)

Design `e606e1b2` freezes one private request sibling/carrier in
`source_preparation.rs` from Rust base `3d174006`. A shared Legacy/Observed
driver selects only the matching effective-override family and reuses one pure
workspace/override/canonical-repository/request-kind projection. Invalid
workspace and effective compute failure are empty-prefix; completed effective
semantic and all request terminals retain the unchanged child epoch; Need and
typed outer are carrierless. The parent is eventless and retains one local
request Result Arc plus compact epoch.

Run only
`WP-6-7A-repository-materialization-request-observation-implementation`.
Authority is exactly `source_preparation.rs`: <=180 production, <=320 tests,
<=500 aggregate semantic and <=14,300 physical lines from 13,747. Preserve the
complete identity, Arc, prefix, family, event, lifecycle, retention,
compatibility and STOP contract. No materialization/source/closure/preparation/
discovery/selected-graph caller is activated. After independent ACCEPT, return
only to the docs-only selected-module-graph observation-frontier design.

### Materialization-request observation proof-cap REPLAN (2026-08-19)

The retained one-file candidate implements the accepted private request owner
without a semantic, ownership, event or retention defect. Against
`3d174006`, it measures +161 production/+319 tests/+480 aggregate at 14,227
physical lines and passes focused 2/2, full bzlmod 431/431 and loading 138/138.
The parent selects only the matching effective-override family, accepts the
complete child epoch before semantic inspection, moves the exact legacy Result
Arc, remains eventless and retains only one request Result Arc plus the compact
epoch.

Independent proof review found that the single remaining test line cannot
discriminate the frozen empty invalid-workspace/effective-compute prefixes,
full missing/unsupported/canonical/request-kind/spec terminals,
command-absolute and HTTP/Git immutable behavior, or command/request-kind
A-B-A with held Result and epoch Arcs. Existing legacy request-kind tests do
not prove the observed parent's prefix or retained-Arc behavior. Compaction
cannot add that matrix without removing the accepted live dependency-row,
family, event, warm, cancellation, Need/outer, legacy-Arc and root-local
lifecycle proof.

Run only
`WP-6-7A-repository-materialization-request-observation-proof-cap-correction-design`,
docs-only, from scheduling base `ba04cde7`, Rust base `3d174006` and
semantic design `e606e1b2`. Retain `source_preparation.rs` non-writable.
Keep production <=180; raise tests only to <=480, aggregate to <=660 and
physical to <=14,480 from the 13,747-line base. The added room may fund only
proof restructuring and the missing observed terminal/lifecycle matrix.

STOP every Rust/Cargo/BUILD/fixture/oracle/public write, semantic or event
change, new key/caller/state, later-owner activation, proof deletion, cap
excess and milestone closure. After independent correction ACCEPT, schedule
exactly the same one-file implementation retry. After its independent ACCEPT,
return only to
`WP-6-7A-selected-module-graph-observation-frontier-design`, docs-only.
Exactly one immediate successor is authorized.

### Materialization-request observation implementation retry scheduled (2026-08-19)

Correction `7592334b` accepts only the measured proof-cap increase. Run
`WP-6-7A-repository-materialization-request-observation-implementation-retry`
from Rust base `3d174006`, semantic design `e606e1b2` and correction
`7592334b`. Authority remains exactly `source_preparation.rs`: <=180
production, <=480 tests, <=660 aggregate semantic and <=14,480 physical lines.

Preserve the matching-family driver, exact Result-Arc projection, effective
epoch, empty/full prefix algebra, eventless parent, compact retention,
compatibility and every existing discriminator. Added room may fund only proof
restructuring and the missing observed terminal/projection and command/
request-kind lifecycle cases. No caller, second file, semantic/event/state
change or later-owner activation is authorized. After independent ACCEPT,
schedule only the docs-only selected-module-graph observation-frontier design.

### Materialization-request observation implementation accepted (2026-08-19)

Implementation `cc847c98` completes the private request owner from Rust base
`3d174006`, design `e606e1b2` and proof correction `7592334b`. The
matching-family driver accepts the complete effective-override epoch before
semantic inspection, preserves exact legacy request Results and every
local/command/immutable projection, remains eventless, and retains only one
request Result Arc plus the compact epoch.

Final one-file accounting is +161 production/+471 tests/+632 aggregate at
14,379 physical lines. Focused 4/4, full bzlmod 433/433, loading 138/138 and
full query pass. Core remains 245/246 only on the inherited stale visibility
wording assertion. Formatting, diff, cleanup/retention and independent review
pass.

### Selected-module-graph frontier materialization REPLAN (2026-08-19)

Do not freeze the selected graph sibling yet. Nonregistry discovery computes
`HostNonregistryModuleClosureKey`, which consumes
`RepositoryMaterializationKey` before repository source reads.
`RepositorySourceFileKey` independently consumes the same materialization
owner. The accepted request sibling closes that key's only observed path child;
its neutral `RepositoryMaterializationResultKey` adds no Host/path observation
and owns no event. This makes `RepositoryMaterializationKey` the uniquely
smallest complete next carrier boundary.

Run only `WP-6-7A-repository-materialization-observation-design`, docs-only,
from scheduling/Rust base `cc847c98`. Freeze one private structural sibling
and a `Dupe`/`Allocative` carrier containing exactly one materialization
Result Arc plus the unchanged request epoch. A shared Legacy/Observed driver
selects only the matching request family, then the same neutral result key.
Request compute failure is empty-prefix; request semantic, result compute,
result semantic and success retain the request prefix. Request Need/outer and
result Need are immediate and carrierless; no epoch union or joined Need exists.
Legacy moves the exact materialization Result Arc.

Both parent and result stay eventless. Retain no request/result child carrier,
collection, cache, store, interner, lock, task, Host read, revision or
certificate. Registry `ModuleSourcePreparationKey`/`RegistryFileKey`,
repository source, nonregistry closure, discovery, selected graph, extension
and generated repository activation remain deferred.

Future implementation authority after independent design ACCEPT is exactly
`source_preparation.rs` from 14,379 physical lines: <=180 production, <=400
tests, <=580 aggregate semantic and <=15,000 physical lines. Keep touched
helpers below 200 lines. Proof must discriminate exact Result and epoch Arcs,
all request/result terminal prefixes and Needs, family rows, child events,
parent silence, cancellation/recovery, result/request lifecycle, and zero later
activation.

Exact behavior is existing materialization values/errors/order, request/result/
generation semantics, legacy Result Arc and child events. The sibling,
carrier, typed outer and epoch association are Slug-native. Registry/source/
closure/discovery/selected graph, extensions, generated repositories,
rules_rust analysis/actions, M8/M7B and exact identity bytes remain deferred.

STOP Rust during design and stop every other file/key/caller/export, later
activation, direct Host read, event/family/order/error drift, retained state,
cap excess and milestone closure. After independent design ACCEPT schedule only
`WP-6-7A-repository-materialization-observation-implementation`; after
implementation ACCEPT return only to the docs-only selected-module-graph
frontier design.

### Repository-materialization observation design accepted (2026-08-19)

Design `b2fd01e7` freezes the uniquely smallest complete carrier after accepted
request implementation `cc847c98`. One private structural sibling and carrier
retain exactly one materialization Result Arc plus the unchanged request epoch.
A matching-family driver preserves request-then-neutral-result order, exact
legacy Result-Arc projection, empty request-compute and full later prefixes,
immediate carrierless Need/outer, eventlessness and compact retention.

Run only
`WP-6-7A-repository-materialization-observation-implementation`. Authority is
exactly `source_preparation.rs` from the 14,379-line `cc847c98` baseline:
<=180 production, <=400 tests, <=580 aggregate semantic and <=15,000 physical
lines. Keep touched helpers below 200 lines.

Preserve exact request/result/generation values, errors, order, legacy Arc and
child events. The sibling/carrier/typed outer/epoch association is Slug-native.
Registry preparation, repository source/nonregistry closure, discovery,
selected graph, extensions, generated repositories, external rules_rust
analysis/actions, M8/M7B and exact identity bytes remain deferred.

Proof must cover every request/result terminal prefix and Need/outer position,
exact Result/epoch Arcs, family rows, eventlessness, warm/cancel/recovery,
request/result A-B-A and zero later activation. STOP every second file/key/
caller/export, semantic/event/family/state drift, direct Host read, cap excess
or milestone closure. After independent implementation ACCEPT, schedule only
the docs-only selected-module-graph observation-frontier design.

### Repository-materialization observation implementation accepted (2026-08-19)

Implementation `ae8aa35e` completes the private materialization owner from
Rust base `cc847c98` and design `b2fd01e7`. Its matching-family driver preserves
request-then-neutral-result order, accepts the request epoch before semantics,
forwards it unchanged through every completed result terminal, keeps Need/outer
carrierless, remains eventless and retains only one local Result Arc plus the
compact epoch.

Final one-file accounting is +168 production/+393 tests/+561 aggregate at
14,940 physical lines. Focused 3/3, full bzlmod 436/436, loading 138/138 and
query 121/121 pass. Core remains 245/246 only on the inherited stale visibility
wording assertion. Formatting, diff, Clippy/archive disposition, cleanup,
retention and independent review pass.

### Selected-module-graph frontier repository-source REPLAN (2026-08-19)

Do not freeze the selected graph sibling yet. Nonregistry module-source
preparation, closure roots/fragments, package preflight, REPO projection and
repository-ignore handling all reuse `RepositorySourceFileKey`. It is the first
complete carrierless owner after accepted materialization: relative-path
validation -> materialization -> resolved path -> FileBytes. The route-based
Host source observation sibling cannot substitute without reconstructing route
identity, and a detached path sibling would have no independent consumer.

Registry preparation separately crosses `RegistryFileKey` and patch-path
resolution. It remains a later frontier. Run only
`WP-6-7A-repository-source-file-observation-design`, docs-only, from
scheduling/Rust base `ae8aa35e`.

Freeze a crate-private structural sibling and a `Dupe`/`Allocative` carrier
containing exactly one local repository-source Result Arc plus the cumulative
epoch. A shared Legacy/Observed driver selects only the matching materialization
and resolution families, followed by neutral FileBytes. It preserves exact
legacy values/errors and nested bytes Arcs; neither sibling computes the other.

Invalid relative path and materialization-compute errors are empty-prefix.
Materialization semantic error retains its epoch. Invalid materialized path and
resolution compute error retain the materialization prefix; resolution
semantic/Absent/WrongKind retains the merged prefix. File compute retains the
resolved prefix; completed Present/Missing/Error retains the full prefix. Merge
the materialization prefix left-first with the resolution epoch, then append
FileBytes before semantic inspection. Preserve the earliest equal Arc and
return typed outer on conflict/mismatch. Need/outer is immediate and
carrierless; this sequential owner performs no Need union.

Parent, resolution and FileBytes owners remain eventless. Existing request/root
children keep their events. Retain no child carrier, path scratch, collection,
cache/store/interner/lock/task, direct Host read, revision, certificate or event
state. Do not activate preflight/REPO/ignore/preparation/closure/discovery/
selected-graph/registry callers.

Future implementation authority after independent design ACCEPT is exactly
`source_preparation.rs` from 14,940 physical lines (<=300 production, <=30
colocated proof, <=15,320 physical) and
`source_preparation_observation_tests.rs` from 2,470 lines (<=500 tests,
<=3,020 physical). Aggregate semantic growth is <=830 and combined physical
size <=18,340; helpers stay below 200 lines.

Proof must discriminate every materialization/resolution/FileBytes terminal
and prefix, first-Arc/conflict/mismatch algebra, exact local/immutable bytes and
epoch Arcs, family rows, parent silence/warm behavior, cancellation/recovery,
local+immutable lifecycle/restoration and zero upper activation.

Exact behavior is current path/materialization/source values, errors, order,
bytes and legacy semantics. The sibling, Result Arc, epoch and typed outer are
Slug-native. Registry/preparation/closure/discovery/selected graph, extensions,
generated repositories, rules_rust analysis/actions, M8/M7B and exact identity
bytes remain deferred.

STOP Rust during design and stop a third future file, caller/export, upper or
registry activation, direct Host read, event/family/error/order drift, retained
state, cap excess and milestone closure. After independent design ACCEPT,
schedule only `WP-6-7A-repository-source-file-observation-implementation`;
after implementation ACCEPT return only to the docs-only selected-graph
frontier design.

### Repository-source-file observation design accepted (2026-08-19)

Design `9040e168` freezes the uniquely smallest reusable source owner from Rust
base `ae8aa35e`. One crate-private structural sibling and carrier retain exactly
one local repository-source Result Arc plus the cumulative materialization ->
resolution -> FileBytes epoch. A shared Legacy/Observed driver selects only the
matching materialization and resolution families and the same neutral FileBytes
key, preserving exact legacy values/errors and nested bytes Arcs.

The accepted algebra keeps the materialization prefix left-first when merging
the resolution epoch, then appends FileBytes, preserving the earliest equal Arc
and returning typed outer on conflict/mismatch. All Need/outer terminals are
carrierless; semantic terminals retain their exact reached prefix. Parent and
path owners remain eventless and retained state is one local Result Arc plus
the compact epoch.

Run only `WP-6-7A-repository-source-file-observation-implementation` in exactly
`source_preparation.rs` (<=300 production, <=30 colocated proof, <=15,320
physical from 14,940) and `source_preparation_observation_tests.rs` (<=500
tests, <=3,020 physical from 2,470). Aggregate semantic growth is <=830 and
combined physical size <=18,340; helpers stay below 200 lines.

Proof every child terminal/prefix, first-Arc/conflict/mismatch, exact local and
immutable bytes/epoch Arcs, family rows, events/warm/cancellation, lifecycle and
zero upper activation. STOP every third file, caller/export, registry or upper
activation, direct Host read, semantic/event/family/state drift, cap excess or
milestone closure. After independent implementation ACCEPT return only to the
docs-only selected-module-graph observation frontier.

### Repository-source-file observation proof-cap correction REPLAN (2026-08-19)


`WP-6-7A-repository-source-file-observation-implementation` is **REPLAN**
before acceptance. Retain the exact two-file Rust candidate from scheduling
base `178dec27`, Rust base `ae8aa35e` and accepted semantic design `9040e168`;
it is non-writable during this docs-only design.

The candidate is ownership- and retention-sound. `source_preparation.rs` is
+298 net at 15,238 physical; the proof file is exactly +500 at 2,970 physical;
aggregate is +798 and 18,208. The 179-line driver selects matching
materialization/resolution families, merges the materialization prefix
left-first, appends FileBytes before semantics, and retains only one local
Result Arc plus the epoch. Focused 3/3, bzlmod 439/439, loading 138/138 and
query 53/53 pass; core remains the inherited 245/246 visibility-text baseline.

The old proof ceiling is exhausted before the parent-specific matrix is
complete. The retry must discriminate the production-used materialization and
resolution reducers for Need, typed outer, compute/semantic prefix and later
suppression; exact epoch iteration order; equal-first Arc, conflicting value
and operation mismatch through merge/FileBytes append; and exact carrier
validity/equality. It must assert exact direct dependency rows including
neutral FileBytes, phase-separate cold child events from parent silence and
warm suppression, and keep every upper family—including repository-ignore and
module preparation—unactivated.

Lifecycle proof must poll/drop and recover the identical request on the same
DICE engine. Both local and immutable namespaces must run
A -> B -> absent -> directory -> A, assert A equals restoration, and hold the
semantic Result, bytes and epoch Arcs through churn before comparing restored
per-demand Arcs. Preserve the existing invalid-relative-path, source terminal,
legacy nested-bytes parity and zero-upper proof. Accepted lower-key tests may
support but cannot replace the new parent's branch/prefix decisions.

Run only
`WP-6-7A-repository-source-file-observation-proof-cap-correction-design`,
docs-only. Keep `source_preparation.rs` at <=300 production, <=30 colocated
proof and <=15,320 physical from 14,940. Raise only
`source_preparation_observation_tests.rs` from <=500 tests/3,020 physical to
<=700 tests/3,250 physical from 2,470. Aggregate becomes <=1,030 semantic and
<=18,570 physical. The +200 semantic/+230 physical proof margin authorizes no
production semantic, event, retention, family or owner change.

The accepted exact behavior remains relative-path/materialization/source
order, Host versus Materialization namespace, symlink/path/FileBytes
semantics, values/errors/nested bytes Arc and all legacy behavior. The
sibling, local Result Arc, epoch and typed outer remain Slug-native. Registry,
preparation/closure/discovery/selected graph, extensions/generated
repositories, rules_rust actions, M8/M7B and exact identity bytes remain
deferred.

STOP Rust/Cargo/BUILD/fixture/oracle writes during design. STOP a third retry
file, caller/export, upper or registry activation, changed production
semantics/events/memory/families, proof deletion, cap excess and milestone
closure. REPLAN again if the full matrix cannot fit. After independent design
ACCEPT schedule only
`WP-6-7A-repository-source-file-observation-implementation-retry`; after its
independent ACCEPT return only to
`WP-6-7A-selected-module-graph-observation-frontier-design`.

### Repository-source-file proof-cap correction accepted (2026-08-19)

Correction `edc533ff` accepts the proof-only REPLAN for the retained source
candidate from Rust base `ae8aa35e` and semantic design `9040e168`. Production
authority remains exactly `source_preparation.rs` at <=300 production, <=30
colocated proof and <=15,320 physical. Proof authority remains exactly
`source_preparation_observation_tests.rs`, raised to <=700 tests and <=3,250
physical. Aggregate caps are <=1,030 semantic and <=18,570 physical.

The retry preserves the accepted key/carrier, matching-family driver,
materialization -> resolution -> FileBytes order, materialization-first exact
Arc algebra, carrierless Need/outer, eventlessness and compact retention. Only
line-neutral production-called terminal projectors may be extracted; no
production semantics, owner, family, event, state or caller may change.

The corrected proof must cover exact epoch iteration order, every terminal
prefix, duplicate/conflict/operation mismatch, neutral FileBytes dependency
rows, phase-separated cold child events and warm silence, identical-request
same-DICE cancellation recovery, and local+immutable
A -> B -> absent -> directory -> A with held/restored Result, bytes and epoch
Arcs. Keep every upper/registry/public family dormant.

Run only `WP-6-7A-repository-source-file-observation-implementation-retry` in
the exact two Rust files and corrected caps. STOP a third file, caller/export,
upper/registry activation, semantic/event/memory/family drift, proof deletion,
cap excess or milestone closure. After independent ACCEPT schedule only the
docs-only selected-module-graph observation frontier design.
### Repository-source observation accepted and selected-graph frontier REPLAN (2026-08-19)

Implementation `12f68983` accepts the repository-source observation sibling
from Rust base `ae8aa35e`, semantic design `9040e168` and proof correction
`edc533ff`. Exact accounting is +297 production and +30 colocated proof in
`source_preparation.rs`, +700 external proof, +1,027 aggregate semantic, and
15,267/3,170/18,437 physical. Focused proof passes 3/3, bzlmod passes 439+193,
loading 204 and query 121; core retains only the inherited 245/246 stale generic
visibility wording baseline. Formatting, diff-check, cleanup/retention and
independent final review pass.

The resumed selected-graph frontier is **REPLAN**. `HostSelectedModuleGraphKey`
still joins legacy `HostDiscoveredModuleKey` leaves. Nonregistry discovery
crosses `HostNonregistryModuleClosureKey`; its Host include horizon remains
legacy-only and reaches `HostNonregistryPackagePreflightKey`, then carrierless
`HostNonregistryRepositoryIgnoreKey`, then carrierless/event-owning
`HostNonregistryRepoFileKey`. Observing closure, discovery or selected graph
would duplicate or relocate that REPO batch. The registry
`ModuleSourcePreparationKey` -> `RegistryFileKey`/patch frontier is separate.

The uniquely smallest next producer is `HostNonregistryRepoFileKey`: it owns
only repository-source `REPO.bazel` -> neutral root REPO semantics -> pure
evaluation and one local batch, and its sole mutable source edge now has the
accepted observed sibling.

### Host nonregistry REPO-file observation design

Run only `WP-6-7A-host-nonregistry-repo-file-observation-design`, docs-only,
from scheduling/Rust base `12f68983`.

Freeze private
`HostNonregistryRepoFileObservationKey(HostNonregistryRepoFileKey)` and
`ObservedHostNonregistryRepoFile`. The carrier is exactly one local
`Arc<Result<HostRepoFileValue, HostRouteRepoFileError>>` plus compact
`PathObservationEpoch`, with `Dupe`/`Allocative` and borrowed accessors.
No export or caller is authorized.

One Legacy/Observed driver preserves source-first order. Legacy selects only
`RepositorySourceFileKey`; observed selects only
`RepositorySourceFileObservationKey`. Only Present continues to the shared
neutral `RootRepoFileSemanticsProjectionKey` and pure evaluation. Move the
exact local Result Arc to legacy.

Source Need/typed outer is immediate, carrierless and stores no parent batch.
Accept the Complete source epoch before semantics. Source Absent/error and policy
failure retain it and store the existing empty batch. Evaluation success/error
retains it and stores the exact current local batch, including semantic
`Some(empty)`. Preserve DICE-invariant behavior. No epoch or Need union exists
at this single-child owner. Complete carrier equality is Result+epoch; outer is
by outer value; Need is invalid/self-unequal.

Each sibling remains sole owner of its matching REPO batch. Source stays
eventless; Need/outer/cancel stores none and warm reuse is silent. Retain only
the local Result Arc+epoch; source carrier/bytes, path, reporter/evaluator and
event scratch are compute-local. Add no collection/cache/store/interner/lock/
task/Host read/revision/certificate/event state.

Future exact Rust authority is only
`app/slug_bzlmod_v2/src/repo_file.rs`, baseline 2,679: <=180 production,
<=320 tests, <=500 aggregate semantic and <=3,200 physical; touched helpers
remain below 200 lines.

Proof must discriminate identity/accessors/equality; real source
Need/outer/Absent/Present/error and later suppression; policy and evaluation
terminals with exact prefix and legacy Result-Arc parity; exact epoch order and
per-demand Arcs plus conflict/mismatch outer; exact family rows; child silence
and parent empty/nonempty/error batches; warm/cancel/recovery; local+immutable
A/B/absent/directory/A held Result+epoch restoration; and zero ignore/preflight/
closure/discovery/selected-graph/registry/extension/public activation.

Exact compatibility is current REPO source/order/UTF-8/result/diagnostic/event
behavior. The private sibling/carrier/typed outer is Slug-native. Ignore,
preflight, closure, discovery/selected graph, registry preparation/patches,
extensions, rules_rust actions, M8/M7B and identity bytes remain deferred.

STOP Rust during design, a second file/key/caller/export, changed event ownership
or text, retained scratch/state, direct Host read, upper/registry activation,
cap excess and milestone closure. After independent design ACCEPT schedule only
`WP-6-7A-host-nonregistry-repo-file-observation-implementation`; after its
ACCEPT design only the nonregistry repository-ignore observed owner.
### Host nonregistry REPO-file design accepted (2026-08-19)

Design `3c598dd5` accepts the uniquely smallest event-owning prerequisite from
Rust base `12f68983`. Schedule only
`WP-6-7A-host-nonregistry-repo-file-observation-implementation`.

Implementation authority is exactly `app/slug_bzlmod_v2/src/repo_file.rs`,
baseline 2,679, with <=180 production, <=320 tests, <=500 aggregate semantic and
<=3,200 physical. Helpers stay below 200 lines.

Keep the private sibling/carrier, matching Legacy/Observed source family,
source-first Present-only semantics continuation, accepted source epoch,
carrierless Need/outer, exact semantic Complete local batch, Result-Arc
projection, compact retention and complete proof contract frozen by
`3c598dd5`. Preserve DICE-invariant behavior, exact legacy REPO semantics,
event text/order, empty batches and family isolation.

STOP a second file/key/caller/export, changed event ownership/text, retained
scratch/state, direct Host read, ignore/preflight/closure/discovery/selected
graph/registry/extension activation, cap excess and milestone closure. After
independent implementation ACCEPT schedule only
`WP-6-7A-host-nonregistry-repository-ignore-observation-design`.

### Host nonregistry REPO-file proof-cap correction REPLAN (2026-08-19)

`WP-6-7A-host-nonregistry-repo-file-observation-implementation` is **REPLAN**
before acceptance. Retain the exact one-file Rust candidate from scheduling
base `aaa0bd3a`, Rust base `12f68983` and accepted semantic design `3c598dd5`;
it is non-writable during this docs-only design.

The candidate is ownership- and retention-sound. `repo_file.rs` is +163
production and +421 proof lines, +584 aggregate semantic, at 3,263 physical
lines. The source-first driver selects the matching source family, accepts the
complete observed epoch before semantics, stores only its matching local REPO
batch and retains one local Result Arc plus the compact epoch. Focused observed
proof passes 2/2; all pre-existing focused REPO proof and diff hygiene pass.

The old +320 proof/3,200 physical envelope is exhausted before the parent matrix
is complete. The retry must add real source Need/typed-outer and later-child
suppression; policy, parse and evaluation error prefixes/batches; exact epoch
iteration and shared-Arc order; parent empty/nonempty/error and warm/no-batch
behavior; real poll-drop identical-request recovery; held Result/epoch lifetime
through both local and immutable A/B/absent/directory/A; and explicit zero
ignore/preflight/closure/discovery/selected-graph/registry/extension/public
activation. Preserve the current exact observed/legacy dependency rows and
source-carrier pointer proof. Accepted child tests support but cannot replace
the parent's branch/event decisions.

Run only
`WP-6-7A-host-nonregistry-repo-file-observation-proof-cap-correction-design`,
docs-only. Keep production <=180 from the 2,679-line base. Raise only proof from
<=320 to <=550, aggregate semantic growth from <=500 to <=730 and final physical
size from <=3,200 to <=3,450. This measured +230 semantic/+250 physical proof
margin leaves the candidate 129 proof and 187 physical lines for the missing
discriminators. It authorizes no production semantic, event, retention, family,
owner or upper-activation change.

Exact behavior remains existing nonregistry REPO source order, UTF-8 policy,
values/errors/diagnostics/events and every legacy result. The private sibling,
Result-Arc+epoch carrier and typed outer remain Slug-native. Ignore/preflight/
closure/discovery/selected graph, registry preparation/patches, extensions,
rules_rust actions, M8/M7B and exact identity bytes remain deferred.

STOP Rust/Cargo/BUILD/fixture/oracle/public writes during design. STOP a second
retry file/key/caller/export, changed production semantics/order/events/memory/
families, direct Host read, upper/registry activation, proof deletion, cap
excess or milestone closure. REPLAN again if the full matrix cannot fit.
After independent design ACCEPT schedule only
`WP-6-7A-host-nonregistry-repo-file-observation-implementation-retry`; after its
independent ACCEPT schedule only the docs-only nonregistry ignore design.

### Host nonregistry REPO-file proof-cap correction accepted (2026-08-19)

Correction `6b75865f` accepts the measured proof-only envelope from Rust base
`12f68983` and semantic design `3c598dd5`. Run only
`WP-6-7A-host-nonregistry-repo-file-observation-implementation-retry` in
`app/slug_bzlmod_v2/src/repo_file.rs`.

Keep production <=180 from the 2,679-line base. Proof is <=550, aggregate
semantic growth <=730 and final physical size <=3,450. Preserve the private
source-first sibling/carrier, exact matching-family/result/event behavior and
compact one-Result-Arc-plus-epoch retention. The added room may fund only the
missing real terminal, epoch/order, event/warm/cancel, lifecycle and upper-
exclusion discriminators recorded above.

STOP a second file/key/caller/export, production semantic/order/event/memory/
family drift, direct Host read, upper/registry activation, proof deletion, cap
excess or milestone closure. After independent retry ACCEPT schedule only
`WP-6-7A-host-nonregistry-repository-ignore-observation-design`.

### Host nonregistry REPO-file second proof-cap correction REPLAN (2026-08-19)

`WP-6-7A-host-nonregistry-repo-file-observation-implementation-retry` is
**REPLAN** before acceptance. Retain the exact one-file Rust candidate from
scheduling base `9aafca07`, Rust base `12f68983`, semantic design
`3c598dd5` and first proof correction `6b75865f`; it is non-writable during
this docs-only design.

The live candidate remains production-sound and cohesive. `repo_file.rs` is
+170 production and +628 proof, +798 aggregate semantic, at 3,477 physical
lines. Focused observed proof passes 3/3; scope is exactly the one authorized
Rust file, formatting is applied and diff-check is clean. The matching-family
source-first driver, accepted source epoch, local REPO event ownership and one
Result Arc plus compact epoch retention require no semantic redesign.

The first corrected <=550 proof, <=730 aggregate and <=3,450 physical envelope
is exhausted before the exact parent matrix is complete. Safe factoring can
recover only about 25--40 lines, while exact identity/hash, semantic variants,
diagnostic/event text, error dependency rows, held Arc lifetime and complete
legacy parity need more space. Forcing the old ceiling would remove
discriminating Need/outer, cancellation, family, epoch or lifecycle proof.

Run only
`WP-6-7A-host-nonregistry-repo-file-observation-proof-cap-correction-2-design`,
docs-only. Write authority is exactly the canonical plan, current manifest,
this Stage 6 plan and the routing log at <=40/<=220/<=180/<=30 net lines and
<=470 aggregate. The retained Rust candidate is non-writable.

The sole future retry authority remains `repo_file.rs` from its 2,679-line
base. Keep production <=180. Raise only proof from <=550 to <=720, aggregate
semantic growth from <=730 to <=900 and physical size from <=3,450 to <=3,700.
This adds <=170 proof-semantic and <=250 physical lines, leaving the measured
candidate 92 proof, 102 aggregate and 223 physical lines of headroom. Helpers
remain below 200 lines.

Preserve the private key/carrier, matching Legacy/Observed source selection,
Present-only continuation, exact Result-Arc projection, carrierless Need/outer,
semantic-Complete matching local batches, source eventlessness and compact
one-Result-Arc-plus-epoch retention. Add no production owner, caller, export,
state, event, Host read, cache, store, interner, lock or task.

The retry must discriminate distinct key equality/hash and Display; exact
Absent/WrongKind/policy/parse/evaluation result classes and messages; exact
diagnostic/print batch text/order and legacy result/event parity; success and
error direct-dependency rows; source-child event silence; and the already
frozen prefix, conflict/mismatch, Need/outer, cancellation, warm, family and
upper boundaries.

For both local and immutable A -> B -> absent -> directory -> A, retain
duplicate handles to the first Result and epoch Arcs and prove those held
handles remain readable and pointer-identical to their duplicates through
churn. Prove restored carrier equality and exact restored per-demand Arc
identity against the restored observed source child. Do not require pointer
identity between independently reconstructed but equal first/restored epochs.

Exact compatibility remains existing nonregistry REPO source order, UTF-8
policy, values/errors/diagnostics/events and every legacy result. The private
sibling, Result-Arc+epoch carrier and typed outer remain Slug-native.
Ignore/preflight/closure/discovery/selected graph, registry preparation/patches,
extensions, rules_rust actions, M8/M7B and exact identity bytes remain
deferred.

STOP Rust/Cargo/BUILD/fixture/oracle/public writes during design. STOP a second
file/key/caller/export, production semantic/order/event/memory/family drift,
direct Host read, upper/registry activation, proof deletion, cap excess or
milestone closure. REPLAN again if the full matrix cannot fit.

After independent correction ACCEPT schedule only
`WP-6-7A-host-nonregistry-repo-file-observation-implementation-retry-2`.
After that retry's independent ACCEPT schedule only the docs-only
`WP-6-7A-host-nonregistry-repository-ignore-observation-design`.

### Host nonregistry REPO-file second proof-cap correction accepted (2026-08-19)

Correction `aff21fdb` accepts the second proof-only REPLAN from Rust base
`12f68983`, semantic design `3c598dd5` and first correction `6b75865f`.
Run only
`WP-6-7A-host-nonregistry-repo-file-observation-implementation-retry-2` in
`app/slug_bzlmod_v2/src/repo_file.rs`.

Keep production <=180, proof <=720, aggregate semantic growth <=900 and final
physical size <=3,700 from the 2,679-line base. Helpers stay below 200 lines.
Preserve the accepted private key/carrier, source-first matching-family driver,
Complete/Need/outer algebra, exact Result-Arc projection, matching local REPO
event ownership and compact one-Result-Arc-plus-epoch retention.

Use the added proof room only for exact key identity, semantic result/message,
diagnostic/print batch, success/error dependency-row, source-silence,
legacy-parity and feasible held/restored Arc discrimination. Preserve every
existing Need/outer, prefix, conflict/mismatch, cancellation, warm, family,
lifecycle and upper-exclusion discriminator.

STOP a second file/key/caller/export, production semantic/order/event/memory/
family drift, direct Host read, upper/registry activation, proof deletion, cap
excess or milestone closure. After independent retry ACCEPT schedule only
`WP-6-7A-host-nonregistry-repository-ignore-observation-design`.
### Host nonregistry REPO-file observation accepted (2026-08-19)

Commit `b08b7f2e` accepts
`WP-6-7A-host-nonregistry-repo-file-observation-implementation-retry-2`
from Rust base `12f68983`, semantic design `3c598dd5` and proof corrections
`6b75865f`/`aff21fdb`. The one-file implementation is +170 production and
+718 proof lines, +888 aggregate semantic, at 3,567 physical lines. Focused
proof is 3/3; full bzlmod is 442 unit plus 193 integration; loading is 204/204
and query is 121/121. Core retains only the documented stale visibility-wording
and legacy snapshot-adapter Need baselines. Formatting, diff hygiene,
cleanup/Buck2 retention and independent review are accepted.

The private sibling selects only its matching repository-source family,
forwards the exact accepted epoch before semantic inspection, owns exactly its
matching local REPO batch and retains one local semantic Result Arc plus the
compact epoch. No ignore/preflight/closure/discovery/selected-graph/registry
caller was activated.

### Host nonregistry repository-ignore observation design (2026-08-19)

Run only `WP-6-7A-host-nonregistry-repository-ignore-observation-design`
from scheduling/Rust base `b08b7f2e`. Write authority is exactly the canonical
plan, current manifest, this Stage 6 plan and the routing log at
<=40/<=220/<=180/<=30 net lines and <=470 aggregate. Rust, Cargo, BUILD,
fixtures and oracles are read-only.

`HostNonregistryRepositoryIgnoreKey` is the uniquely smallest complete next
owner. Its existing order is event-owning `HostNonregistryRepoFileKey`, then
the `.bazelignore` `RepositorySourceFileKey`, then the existing parser. Both
semantic children have accepted observed siblings. The parser's only additional
mutable edge is the existing Windows long-path observation, already returned
by `parse_ignore_file_observed` as a compact epoch with Need/typed-outer
polarity. Package preflight is the sole consumer and remains deferred.

Freeze one private structural
`HostNonregistryRepositoryIgnoreObservationKey(HostNonregistryRepositoryIgnoreKey)`
and one private `ObservedHostNonregistryRepositoryIgnore`. Retain exactly one
local `Arc<Result<RepositoryIgnoreMatcher, HostRepositoryIgnoreError>>` plus
one compact `PathObservationEpoch`, with `Dupe`/`Allocative` and borrowed
accessors. Add no export or caller.

Use one Legacy/Observed driver. Preserve exact repo -> source -> parser order.
Legacy selects only the legacy REPO and repository-source children and moves
the exact local matcher Result Arc. Observed selects only the accepted observed
siblings and uses `parse_ignore_file_observed`. Neither mode computes the
other family.
Both modes retain the same neutral Windows long-path parser dependency when
that parser path is reached.

Repo Need/typed outer is immediate and carrierless. Accept a complete repo
epoch before semantic inspection; repo semantic failure keeps the repo-only
prefix and suppresses source work. After repo success, source Need/typed outer
is carrierless. Merge the accepted repo prefix left-first with the complete
source epoch before source semantics. Source error/Absent/Directory/Present
keeps that merged prefix; Absent/Directory suppress the parser. Present invokes
the observed parser. Parser Need/typed outer is carrierless; merge the existing
repo+source prefix left-first with its complete epoch before parser semantics.
Parser error/success keeps the full prefix. Equal duplicates retain the earliest
exact Arc; conflict/operation mismatch is typed outer. This sequential owner
has no Need union.

Both ignore siblings remain eventless. Their matching REPO child remains sole
owner of its local batch; repository source and parser observations are
eventless. Need/outer/cancellation stores no parent state and warm reuse emits
nothing. Retain no child carrier, source bytes, parser scratch, second
collection, cache/interner/store/lock/task, direct Host read, revision,
certificate or event state.

After design ACCEPT, exact Rust authority is only
`app/slug_bzlmod_v2/src/repository_ignore.rs` from the 3,297-line
`b08b7f2e` baseline: <=180 production, <=400 proof, <=580 aggregate semantic
and <=3,900 physical; touched helpers remain below 200 lines.

Proof must discriminate key/hash/Display and carrier equality; production
repo/source/parser terminal reduction; real Need/outer and later suppression;
all repo/source/parser semantic terminals with exact prior/merged/full prefixes;
ordered epoch membership, exact shared Arcs, duplicate-first/conflict/mismatch;
Windows long-path behavior where applicable; exact family rows and reverse
isolation; child REPO batch and parent/source/parser silence, warm and
poll-drop recovery; independent local/immutable REPO plus `.bazelignore`
A/B/absent/directory/A with held Result/epoch handles; and zero upper, registry,
extension or public activation. Reuse accepted semantic evidence; no Bazel
oracle is needed because grammar, platform policy, errors and legacy events are
unchanged.

Exact compatibility is current REPO -> `.bazelignore` -> parser order,
grammar/platform behavior, matcher/errors and legacy child events. The private
sibling/carrier/typed outer is Slug-native. Preflight/closure/discovery/selected
graph, registry preparation/patches, extension repositories, M8/M7B and exact
identity bytes remain deferred.

STOP Rust writes during design. STOP a second file/key/caller/export, semantic
or parser drift, event-owner change, extra retained state, direct Host read,
upper/registry activation, cap excess or milestone closure. REPLAN if the
one-file envelope cannot preserve exact behavior. After independent design
ACCEPT schedule only
`WP-6-7A-host-nonregistry-repository-ignore-observation-implementation`;
after independent implementation ACCEPT schedule only the docs-only
`WP-6-7A-host-nonregistry-package-preflight-observation-design`.
### Host nonregistry repository-ignore observation design accepted (2026-08-19)

Design `9c0a5473` accepts the private observed ignore owner from Rust base
`b08b7f2e`. Run only
`WP-6-7A-host-nonregistry-repository-ignore-observation-implementation`.

Exact Rust authority is only
`app/slug_bzlmod_v2/src/repository_ignore.rs` from its 3,297-line baseline:
<=180 production, <=400 proof, <=580 aggregate semantic and <=3,900 physical;
touched helpers remain below 200 lines.

Preserve the private one-Result-Arc+compact-epoch carrier and one shared
Legacy/Observed driver. Exact order is REPO -> repository source -> parser.
Accept each Complete epoch before semantic inspection, merge the accumulated
earlier prefix left-first with each later source/parser epoch, preserve the
earliest duplicate Arc, and return Need/typed outer carrierless without later
activation. This sequential owner has no Need union.

Legacy selects only legacy REPO/source families; observed selects only their
accepted observed siblings. Both modes retain the same neutral Windows
long-path parser dependency when reached. The ignore parent remains eventless,
its matching REPO child remains sole local batch owner, and retained state is
only the local matcher Result Arc plus cumulative epoch.

Proof exact terminal prefixes, ordered shared Arcs, conflict/mismatch,
family rows, child event ownership/warm/cancellation, local+immutable lifecycle
and upper nonactivation. Exact ignore grammar/platform behavior, values/errors
and legacy events remain unchanged; the sibling/carrier/typed outer is
Slug-native. Preflight/closure/discovery/selected graph, registry preparation,
extensions, M8/M7B and identity bytes remain deferred.

STOP a second file/key/caller/export, parser/legacy/event drift, extra retained
state, direct Host read, upper/registry activation, cap excess and milestone
closure. After independent implementation ACCEPT schedule only
`WP-6-7A-host-nonregistry-package-preflight-observation-design`.

### Host nonregistry repository-ignore observation proof-cap REPLAN (2026-08-19)

Run only
`WP-6-7A-host-nonregistry-repository-ignore-observation-proof-cap-correction-design`
from scheduling base `e9fc14a6`, Rust base `b08b7f2e` and accepted semantic
design `9c0a5473`. Retain the dirty one-file implementation candidate and make
it non-writable during this docs-only correction.

The candidate is production-sound and its focused parent integration passes.
Against `b08b7f2e`, `repository_ignore.rs` is +180 production and +484 proof
lines, +664 aggregate semantic, at 3,961 physical lines. The original <=400
proof, <=580 aggregate and <=3,900 physical caps are already exceeded before
the full frozen parent matrix is discriminated. This is a proof/cap stop, not a
semantic, ownership, family, event or retention redesign.

Write only the canonical plan, current manifest, this Stage 6 plan and routing
log at <=40/<=220/<=180/<=30 net lines and <=470 aggregate. Rust, Cargo, BUILD,
fixtures, oracles, callers and public files are read-only.

The future retry retains exact authority only over
`app/slug_bzlmod_v2/src/repository_ignore.rs` from its 3,297-line baseline.
Keep production <=180; raise only proof to <=720, aggregate semantic growth to
<=900 and final physical size to <=4,250. Touched helpers remain below 200
lines.

Freeze the private one-Result-Arc+compact-epoch carrier and single
Legacy/Observed driver. Preserve exact repo -> source -> parser matching-family
order. Merge the earlier repo prefix left-first with each later Complete source
and parser epoch before semantic inspection, preserving the earliest exact Arc.
Need/typed outer is carrierless and suppresses later work; there is no Need
union. Both families share the neutral Windows long-path parser dependency when
reached. The ignore parent stays eventless, the matching REPO child stays sole
batch owner, and no child carrier, source bytes, parser scratch, second
collection, cache/interner/store/lock/task, Host read, revision, certificate or
event state may be retained.

The retry must preserve current proof and add real source-position Need/outer;
exact repo/source/parser semantic prefixes and messages; exact ordered epoch
Arcs, duplicate-first/conflict/operation mismatch; parent/source/parser silence,
child REPO text/order, warm and cancellation recovery; exact family rows and
upper exclusion; and independent local and immutable REPO-file and
`.bazelignore` A/B/absent/directory/A lifecycles with held handles and restored
child-parent Arc identity. Exact current grammar/platform values/errors/events
remain unchanged; the private sibling/carrier/typed outer is Slug-native.

STOP Rust writes during design and STOP a second file/key/caller/export,
production drift, upper/registry activation, proof deletion, cap excess or
milestone closure. After independent correction ACCEPT schedule only
`WP-6-7A-host-nonregistry-repository-ignore-observation-implementation-retry`;
after retry ACCEPT schedule only the docs-only
`WP-6-7A-host-nonregistry-package-preflight-observation-design`.

### Host nonregistry repository-ignore observation proof correction accepted (2026-08-19)

Correction `4c9f344b` accepts the measured proof envelope for semantic design
`9c0a5473` from Rust base `b08b7f2e`. Run only
`WP-6-7A-host-nonregistry-repository-ignore-observation-implementation-retry`.

Exact Rust authority is only
`app/slug_bzlmod_v2/src/repository_ignore.rs`: <=180 production, <=720 proof,
<=900 aggregate semantic and <=4,250 physical lines; helpers remain below 200.
Every other file is read-only.

Preserve the private one-Result-Arc+compact-epoch carrier and matching-family
repo -> source -> parser driver. Merge each later Complete epoch into the
earlier prefix left-first before semantic inspection; preserve the earliest
equal Arc. Need/typed outer stays carrierless and suppresses later work. Both
families share the neutral Windows parser observation when reached.

The ignore parent stays eventless; its matching REPO child remains sole local
batch owner. Retain no child carrier, source bytes, parser scratch, extra
collection/state, cache/interner/store/lock/task, Host read, revision,
certificate or event state.

Complete the corrected proof matrix: source Need/outer; exact terminal prefixes
and messages; epoch order/ptr identity/duplicate/conflict/mismatch; exact event
silence and REPO batch/warm/cancellation; family rows and upper exclusion; and
independent local+immutable REPO-file and `.bazelignore` lifecycles with held
handles and restored child-parent Arc identity.

STOP a second file/key/caller/export, production or legacy drift, retained-state
growth, upper/registry activation, cap excess and milestone closure. After
implementation ACCEPT schedule only the docs-only
`WP-6-7A-host-nonregistry-package-preflight-observation-design`.

### Nonregistry repository-ignore observation accepted (2026-08-19)

Implementation `754e7619` accepts the private matching-family repository-ignore
owner from Rust base `b08b7f2e` and semantic design `9c0a5473`. Exact growth is
+179 production/+715 proof/+894 aggregate at 4,191 physical lines. Focused
proof is 2/2; full bzlmod/loading/query remain green, and core is 245/246 with
only the recorded inherited stale external-visibility wording baseline.

The accepted owner preserves repo -> source -> parser order, earliest exact
epoch Arcs, carrierless Need/outer, child-only REPO events, warm suppression,
local+immutable lifecycles, and one local matcher Result Arc plus compact epoch.
No upper family was activated.

Run only `WP-6-7A-host-nonregistry-package-preflight-observation-design`.

### Host nonregistry package-preflight observation design (2026-08-19)

`HostNonregistryPackagePreflightKey` is the smallest complete next owner. It is
the reusable boundary consumed by the nonregistry include horizon. Accepted
observed siblings now cover its effective override, repository ignore and both
repository-source marker children; the deleted-package projection is neutral
command input and owns no path epoch or event. No lower prerequisite remains.

Freeze private `HostNonregistryPackagePreflightObservationKey`,
`ObservedHostNonregistryPackagePreflight`, and stage-aware
`HostNonregistryPackagePreflightObservationError`. Key Value is exactly
`SourcePreparationOutcome<Result<ObservedHostNonregistryPackagePreflight,
HostNonregistryPackagePreflightObservationError>>`. The carrier retains one
local semantic Result Arc plus cumulative epoch, with Dupe/Allocative and
borrowed accessors. Add no export or caller.

Use one Legacy/Observed driver in exact order: effective override, invalid-name
short circuit, neutral deleted-package policy, repository ignore, `BUILD.bazel`,
then `BUILD`. Legacy selects only legacy effective/ignore/source families;
observed selects only accepted observed siblings; both share the neutral policy.

Accept the effective epoch before semantics. Effective error, absent nonregistry
override, invalid name, policy error and nonempty deleted policy retain that
prefix. Merge the effective prefix left-first with a Complete ignore epoch
before ignore semantics. Ignored suppresses both markers. Merge each reached
marker epoch into the accumulated prefix left-first before its semantics.
`BUILD.bazel` Present suppresses `BUILD`; second-marker and NoBuild terminals
retain both marker epochs. Equal duplicates keep the earliest exact Arc;
conflict or operation mismatch is typed outer.

The outer distinguishes effective/ignore/marker child frontier errors and
stage-specific effective/policy/ignore/marker DICE compute failures. All are
carrierless and suppress later work. Invalid-name is pure; semantic deleted-
policy projection error remains Complete with the effective prefix. Need applies
only at effective, ignore and markers. There is no Need union. Need is invalid/
self-unequal; Complete outer is valid/equal by outer; Complete carrier is
valid/equal by semantic Result plus epoch. Preserve the legacy invariant and
invent no semantic compute error.

The preflight parent remains eventless. Root MODULE and matching REPO descendants
remain sole batch owners; effective, policy, ignore parent and marker sources
stay eventless. Retain no child carrier, policy value, matcher, marker bytes,
extra collection/state, cache/interner/store/lock/task, direct Host read,
revision, certificate or event state.

After independent design ACCEPT, exact Rust authority is:

- `app/slug_bzlmod_v2/src/source_preparation.rs`, 15,267-line baseline, <=320
  production and <=15,650 physical;
- `source_preparation_observation_tests.rs`, 3,170-line baseline, <=720 proof
  and <=3,950 physical;
- <=1,040 semantic and <=19,600 physical aggregate; helpers below 200 lines.

Proof key identity/accessors/equality; Need/child outer only at effective, ignore
and marker positions; DICE-compute outer at every computed dependency; every
semantic terminal and later suppression; semantic policy projection error versus
policy compute failure; invalid-name/deleted-policy order; marker preference;
exact epoch order/Arcs/duplicate/conflict/mismatch; exact family rows; ROOT/REPO
child events with parent silence/warm/cancellation; local+immutable marker
A/B/absent/directory/A and BUILD.bazel<->BUILD restoration; held handles; and
zero horizon/closure/discovery/selected-graph/registry/extension/public activation.

Exact compatibility is current order, short circuits, marker preference,
values/errors, legacy Result Arc and child events. The private sibling/carrier/
typed outer is Slug-native. Horizon/closure/discovery/selected graph, registry
preparation, extensions, M8/M7B and identity bytes remain deferred.

STOP Rust during design and STOP a caller/export/third file, semantic/order/
event/family drift, retained-state growth, upper/registry activation, cap excess
or milestone closure. After design ACCEPT schedule only
`WP-6-7A-host-nonregistry-package-preflight-observation-implementation`; after
implementation ACCEPT schedule only
`WP-6-7A-host-nonregistry-module-closure-observation-design`.

### Host nonregistry package-preflight observation design accepted (2026-08-19)

Design `0c5a1366` accepts the private stage-aware observed owner from Rust base
`754e7619`. Run only
`WP-6-7A-host-nonregistry-package-preflight-observation-implementation`.

Exact authority is `source_preparation.rs` at <=320 production/15,650 physical
and `source_preparation_observation_tests.rs` at <=720 proof/3,950 physical,
<=1,040 semantic/19,600 physical aggregate; helpers remain below 200 lines.

Preserve the exact effective -> invalid-name -> neutral deleted-policy -> ignore
-> BUILD.bazel -> BUILD driver, matching families and left-first Complete epoch
merges before semantics. The named stage-aware outer keeps effective/policy/
ignore/marker compute failures and effective/ignore/marker child frontier
failures carrierless. Semantic policy errors keep the effective prefix; Need
exists only at effective/ignore/markers and no Need union is allowed.

The parent remains eventless, ROOT/REPO children remain sole batch owners, and
retained state is only the local semantic Result Arc plus cumulative epoch.
Prove exact terminal positions/prefixes/Arcs, marker preference, family rows,
events/warm/cancellation, local+immutable lifecycles and upper nonactivation.

STOP a third file/caller/export, semantic/order/event/family drift, retained
state growth, upper/registry activation, cap excess and milestone closure.
After implementation ACCEPT schedule only
`WP-6-7A-host-nonregistry-module-closure-observation-design`.

### Host nonregistry package-preflight observation proof-cap REPLAN (2026-08-19)

Run only
`WP-6-7A-host-nonregistry-package-preflight-observation-proof-cap-correction-design`
from scheduling base `14e6a571`, Rust base `754e7619` and accepted semantic
design `0c5a1366`. Retain the dirty two-file implementation candidate and make
it non-writable during this docs-only correction.

The candidate's focused observed-preflight proof is 7/7 and production ownership
is sound. Against `754e7619`, `source_preparation.rs` is +319 production/+15
colocated proof at 15,601 physical lines; the external proof is +832 at 4,002;
aggregate semantic growth is +1,166 at 19,603 physical. The original <=720 proof
and <=1,040 aggregate caps are exceeded before every frozen semantic-prefix,
policy-error and exact child-batch discriminator is complete. Removing 126 lines
would delete evidence rather than compact an overbuilt owner.

Write only the canonical plan, current manifest, this Stage 6 plan and routing
log at <=40/<=220/<=180/<=30 net lines and <=470 aggregate. Rust, Cargo, BUILD,
fixtures, oracles, callers and public files are read-only.

The future retry retains exact authority over only
`app/slug_bzlmod_v2/src/source_preparation.rs` and
`source_preparation_observation_tests.rs` from the 15,267/3,170 baselines.
Keep source production <=320 and source physical <=15,650. Raise only external
proof to <=960 and <=4,250 physical; aggregate becomes <=1,300 semantic and
<=19,900 physical. Touched helpers remain below 200 lines.

Freeze the private stage-aware one-Result-Arc+compact-epoch carrier and the one
Legacy/Observed driver in effective -> invalid-name -> neutral deleted-policy ->
ignore -> BUILD.bazel -> BUILD order. Preserve matching-family selection and
left-first Complete epoch merging before semantic inspection. Need/typed outer
is carrierless, later work is suppressed, equal duplicates retain the earliest
exact Arc, conflict/operation mismatch is typed outer, and there is no Need
union.

The parent remains eventless; ROOT/REPO descendants remain sole batch owners.
Retain no child carrier, policy value, matcher, marker bytes, extra collection or
state, cache/interner/store/lock/task, direct Host read, revision, certificate or
event state.

Preserve the retained passing key/projection/reducer, prefix/family/event,
local+immutable lifecycle and cancellation proof. Complete exact production-used
DICE-compute projections at every child, every semantic terminal with exact
prior/merged prefix and later suppression, semantic policy projection error
versus policy DICE failure, exact ordered per-demand Arcs and first/conflict/
mismatch behavior, exact legacy/observed rows, child-owned ROOT/REPO text/order
with all parent/helper silence, warm and cancellation recovery, local+immutable
marker preference/restoration, held handles and zero upper activation.

Exact current order, values/errors, legacy Result Arc and child events remain
unchanged; the private sibling/carrier/typed outer is Slug-native. Horizon/
closure/discovery/selected graph, registry preparation/patches, extensions,
M8/M7B and identity bytes remain deferred.

STOP Rust during design and STOP a third file/caller/export, production/order/
event/family/memory drift, upper/registry activation, proof deletion, cap excess
or milestone closure. After independent correction ACCEPT schedule only
`WP-6-7A-host-nonregistry-package-preflight-observation-implementation-retry`;
after retry ACCEPT schedule only
`WP-6-7A-host-nonregistry-module-closure-observation-design`.

### Host nonregistry package-preflight proof correction accepted (2026-08-19)

Correction `ed3a9d05` accepts the measured proof envelope for semantic design
`0c5a1366` from Rust base `754e7619`. Run only
`WP-6-7A-host-nonregistry-package-preflight-observation-implementation-retry`.

Exact Rust authority is only
`app/slug_bzlmod_v2/src/source_preparation.rs`, <=320 production and <=15,650
physical, plus `source_preparation_observation_tests.rs`, <=960 proof and
<=4,250 physical; aggregate is <=1,300 semantic and <=19,900 physical. Helpers
remain below 200 lines; every other file is read-only.

Preserve the private stage-aware one-Result-Arc+compact-epoch carrier and exact
effective -> invalid-name -> neutral deleted-policy -> ignore -> BUILD.bazel ->
BUILD matching-family driver. Merge each Complete child epoch into the earlier
prefix left-first before semantics; Need/typed outer remains carrierless, later
work is suppressed, equal duplicates retain the earliest exact Arc, and there
is no Need union.

The parent remains eventless and ROOT/REPO descendants remain sole batch owners.
Retain no child carrier, policy value, matcher, marker bytes, extra collection/
state, cache/interner/store/lock/task, Host read, revision, certificate or event
state.

Preserve current passing proof and complete production-used compute projections,
every semantic terminal/prefix/suppression, semantic policy versus DICE failure,
exact ordered Arcs/conflict/mismatch, exact family rows and ROOT/REPO events,
warm/cancellation recovery, local+immutable marker lifecycle/held handles, and
upper nonactivation.

STOP a third file/caller/export, production/order/event/family/memory drift,
upper/registry activation, proof deletion, cap excess or milestone closure.
After independent implementation ACCEPT schedule only
`WP-6-7A-host-nonregistry-module-closure-observation-design`.

### Host nonregistry package-preflight second proof-cap REPLAN (2026-08-19)

Run only
`WP-6-7A-host-nonregistry-package-preflight-observation-proof-cap-correction-2-design`
from scheduling base `2439f1fd`, Rust base `754e7619`, semantic design
`0c5a1366` and first proof correction `ed3a9d05`. Retain the dirty two-file
candidate and make it non-writable during this docs-only correction.

The focused observed-preflight proof is 7/7 and production remains sound.
Against `754e7619`, source is +319 production/+24 colocated proof at 15,610
physical, external proof is +949 at 4,119, and aggregate growth is +1,292 at
19,729 physical. The first correction leaves only 11 external and eight
aggregate lines while exact later-terminal epochs, complete event sequence,
real upper exclusions and cancellation child silence remain undiscriminated.

Write only canonical/current/this Stage 6 plan/routing at
<=40/<=220/<=180/<=30 net lines and <=470 aggregate. Rust, Cargo, BUILD,
fixtures, oracles, callers and public files are read-only.

The future retry keeps exact authority only over `source_preparation.rs` and
`source_preparation_observation_tests.rs` from the 15,267/3,170 baselines.
Keep source <=320 production and <=15,650 physical; raise only external proof to
<=1,120 and <=4,500 physical, aggregate <=1,470 semantic and <=20,150 physical.
Helpers remain below 200 lines.

Freeze the private stage-aware one-Result-Arc+compact-epoch carrier and exact
effective -> invalid-name -> neutral deleted-policy -> ignore -> BUILD.bazel ->
BUILD matching-family driver. Preserve left-first merges before semantics,
carrierless Need/outer, earliest equal Arcs, typed conflict/mismatch, no Need
union, eventless parent, child-only ROOT/REPO batches and compact retention.

Preserve passing key/projection/reducer, semantic, family, lifecycle and
cancellation proof. Add exact ptr-identical effective+ignore and full reached
marker epochs for Ignored, ignore error, marker error, fallback BUILD and
NoBuildFile; the complete relevant ROOT/REPO Some-batch sequence with legacy
parity and warm silence; whole exact legacy row; exact upper prefix table
including module-source-preparation and host-selected-extension; and zero
cancelled parent/child row or batch publication before same-DICE recovery.

Exact current order, values/errors, Result Arc and child events remain unchanged.
The private sibling/carrier/typed outer is Slug-native. Horizon/closure/discovery/
selected graph, registry preparation, extensions, M8/M7B and identity bytes stay
deferred.

STOP Rust during design and STOP a third file/caller/export, production/order/
event/family/memory drift, upper/registry activation, proof deletion, cap excess
or milestone closure. After independent correction ACCEPT schedule only
`WP-6-7A-host-nonregistry-package-preflight-observation-implementation-retry-2`;
after retry ACCEPT schedule only
`WP-6-7A-host-nonregistry-module-closure-observation-design`.

### Host nonregistry package-preflight second proof correction accepted (2026-08-19)

Correction `7524cd41` accepts the remaining exact-proof envelope for semantic
design `0c5a1366`, first correction `ed3a9d05`, and Rust base `754e7619`.
Run only
`WP-6-7A-host-nonregistry-package-preflight-observation-implementation-retry-2`.

Exact write authority is only
`app/slug_bzlmod_v2/src/source_preparation.rs`, <=320 production and <=15,650
physical, plus `source_preparation_observation_tests.rs`, <=1,120 proof and
<=4,500 physical. Aggregate is <=1,470 semantic and <=20,150 physical; touched
helpers remain below 200 lines.

Preserve the private one-Result-Arc+compact-epoch carrier and exact effective ->
invalid-name -> neutral deleted-policy -> ignore -> BUILD.bazel -> BUILD
matching-family driver. Merge Complete epochs into the earlier prefix
left-first before semantics; Need/outer stays carrierless, earliest equal Arcs
survive, typed conflict/mismatch stays outer, the parent stays eventless, and
ROOT/REPO descendants remain sole batch owners.

Keep all passing proof and add exact later-terminal epochs, the complete
ROOT/REPO Some-batch sequence with legacy parity and warm silence, whole exact
legacy row, exact upper prefixes, and zero cancelled parent/child row or batch
publication before same-DICE recovery. Production semantics, event ownership,
retained state and compatibility classes are frozen.

STOP a caller/export/third file, production/order/event/family/memory drift,
upper/registry activation, proof deletion, cap excess or milestone closure.
After independent implementation ACCEPT schedule only
`WP-6-7A-host-nonregistry-module-closure-observation-design`.

### Host nonregistry package-preflight observation accepted (2026-08-19)

Implementation `18166691` accepts the private matching-family package-preflight
carrier from Rust base `754e7619`, design `0c5a1366`, first proof correction
`ed3a9d05`, and second proof correction `7524cd41`.

Exact accepted accounting is +320 production/+24 colocated proof in
`source_preparation.rs`, +1,060 external proof, +1,404 aggregate semantic, and
15,611/4,230/19,841 physical lines. Focused observed-preflight proof is 7/7;
full bzlmod is 449 plus all integration suites, loading 204, query 121, and core
245/246 with only the inherited stale generic visibility-message expectation.
Fmt, diff-check, exact accounting, cleanup/retention and independent final
review pass.

The accepted owner preserves effective -> invalid name -> deleted policy ->
ignore -> BUILD.bazel -> BUILD order, exact Complete prefix Arcs, carrierless
Need/outer, child-only ROOT/REPO batches, compact Result-Arc+epoch retention and
legacy family/result parity. No horizon, closure, discovery, selected graph,
registry or extension owner is activated.

Run only `WP-6-7A-host-nonregistry-module-closure-observation-design` from
scheduling and Rust base `18166691`.

### Host nonregistry module-closure observation design (2026-08-19)

`HostNonregistryModuleClosureKey` is the uniquely smallest complete owner. It
sequences accepted effective, materialization, root-source and package-preflight
children, then owns root validation, BFS horizons, fragment batches, cycle and
final closure semantics. Horizon and fragment reducers have no independent
consumer; `HostDiscoveredModuleKey` remains inactive.

Freeze private `HostNonregistryModuleClosureObservationKey` and a carrier with
exactly one local closure Result Arc plus compact cumulative epoch. One
Legacy/Observed driver preserves effective -> materialization -> root MODULE ->
validation -> BFS horizon -> fragment order and selects only matching accepted
families. Merge every Complete child into the earlier prefix left-first before
semantics. Explicit materialization precedes equal duplicate materialization
demands reached below it, preserving its exact Arc. Need and typed outer remain
carrierless; semantic terminals retain the full reached prefix.

Compute polarity is exact: Legacy effective failure remains the invariant panic;
Observed effective failure is empty-prefix typed outer. Materialization and root
source compute failures remain semantic with effective-only and
effective+materialization prefixes. Package and fragment compute failures remain
semantic at their occurrence with the prefix through earlier successes, unless
an earlier Need already selects the full compatible Need union. Observed frontier
errors/conflicts are carrierless outer; Legacy has no sibling frontier outer.

Each horizon parses the whole ordered request slice before child computation,
deduplicates packages by first occurrence, computes the unique batch and reduces
in original occurrence order. The first occurrence outer/conflict, Need or
semantic wins exactly as Legacy; Need returns the precomputed full compatible
union, and incompatible Need REPLANs. Later computed outcomes are not merged or
retained. Fragment reduction preserves occurrence order and existing
precedence: semantic before Need returns semantic; Need before semantic returns
the full Need union; an outer/conflict before decisive semantic wins, including
after Need; otherwise outer > Need > success. Preserve duplicate includes, BFS
order, cycles and the complete epoch.

The closure parent stays eventless; reached ROOT/REPO children retain sole local
batch ownership. Retain only one local closure Result Arc plus epoch. Child
carriers, batch results, BFS/frontier/ancestry/cycle/Need/event/union scratch are
compute-local or dependency-owned. Add no collection/cache/interner/store/lock/
task/Host read/revision/certificate/event state.

Proof must discriminate identity/equality/legacy Arc projection; every initial,
horizon and fragment Need/outer/semantic position; exact iteration, per-demand
Arcs, duplicate-first/conflict/mismatch; full Need union and mixed precedence;
multi-level BFS/duplicates/cycles; local+immutable A/B/absent/directory/A; held
carriers; cancel/recovery; exact family rows and ROOT/REPO event order; warm
silence; and zero discovery/selected/registry/extension/public activation.

Future exact authority is only `source_preparation.rs` from 15,611 physical at
<=520 production/<=16,250 physical and
`source_preparation_observation_tests.rs` from 4,230 at <=1,200 proof/<=5,550
physical; aggregate <=1,800 semantic and <=21,800 physical. Helpers stay below
200 lines. Exact legacy closure behavior is preserved; sibling/carrier/typed
outer is Slug-native; discovery/selected graph, registry/extensions, M8/M7B and
identity bytes remain deferred.

Write only canonical/current/this Stage/routing at <=40/<=220/<=180/<=30 net
lines and <=470 aggregate. STOP Rust/callers/exports/upper activation during
design. After independent design ACCEPT schedule only
`WP-6-7A-host-nonregistry-module-closure-observation-implementation`; after its
ACCEPT return only to the docs-only selected-module-graph frontier audit.

### Host nonregistry module-closure observation design accepted (2026-08-19)

Design `0ffa79cd` accepts `HostNonregistryModuleClosureKey` as the uniquely
smallest complete owner and activates only
`WP-6-7A-host-nonregistry-module-closure-observation-implementation` from Rust
base `18166691`.

Exact authority is only `source_preparation.rs`, <=520 production and <=16,250
physical from 15,611, plus `source_preparation_observation_tests.rs`, <=1,200
proof and <=5,550 physical from 4,230. Aggregate is <=1,800 semantic and
<=21,800 physical; touched helpers stay below 200 lines.

Implement one private one-Result-Arc+epoch sibling and matching-family driver in
effective -> materialization -> root source -> validation -> BFS horizon ->
fragment order. Preserve earlier-prefix left-first merges, explicit-first
materialization Arc, carrierless Need/outer, explicit compute-failure polarity,
occurrence-order horizon first-terminal behavior with full compatible Need union,
and existing fragment mixed precedence. Parent remains eventless and compact;
lower ROOT/REPO children remain sole event owners.

Proof exact identity/Arc/equality, every initial/horizon/fragment terminal,
full-batch and conflict algebra, BFS/duplicate/cycle/lifecycle/cancellation,
family rows/events/warm silence and zero upper activation. STOP a third file,
caller/export, discovery/selected/registry/extension activation, semantic/event/
memory/family drift, cap excess or milestone closure. After independent ACCEPT
return only to the docs-only selected-module-graph frontier audit.

### Host nonregistry module-closure observation accepted (2026-08-19)

Implementation `28fab9b0` accepts the private matching-family
nonregistry-closure carrier from Rust base `18166691` and design `0ffa79cd`.

Exact accounting is +519 production/+5 colocated proof in
`source_preparation.rs`, +1,032 external proof, +1,556 aggregate semantic, and
16,135/5,262/21,397 physical lines. Focused observed-Host proof is 13/13; full
bzlmod is 454 plus all integration suites, loading 204, query 121, and core
245/246 with only the inherited stale generic visibility-message expectation.
Fmt, diff-check, exact accounting, cleanup/retention and independent final
review pass.

The accepted owner preserves effective -> materialization -> root source ->
validation -> BFS horizon -> fragment order, exact occurrence/full-batch
Need/outer precedence, earliest shared Arcs, child-only ROOT/REPO batches, and
one local Result Arc plus compact epoch. Discovery, selected graph, registry
preparation, extensions and public callers remain inactive.

Run only `WP-6-7A-selected-module-graph-observation-frontier-audit` from
scheduling and Rust base `28fab9b0`.

### Selected-module-graph observation frontier audit (2026-08-19)

Audit `HostSelectedModuleGraphKey` from accepted root files and effective
overrides through fixed-point `HostDiscoveredModuleKey` expansion. Inventory
each candidate's natural Result owner, ordered DICE children, matching family,
path epoch, Need/outer/semantic polarity, event batch, retained lifetime and
proof boundary before selecting anything.

The accepted nonregistry closure must remain the sole closure semantic carrier;
its ROOT/REPO descendants remain sole batch owners. Separately trace registry
`ModuleSourcePreparationKey`, policy/registry attempt order, every
`RegistryFileKey`, root patch label/path resolution and FileBytes observation.
Do not claim selected graph or discovered module completeness while any mutable
registry/patch edge remains carrierless.

Return exactly one docs-only result: selected-graph observation design if every
mutable child already has a reusable carrier; one uniquely smaller producer
prerequisite design; or formal REPLAN. Freeze future exact Rust files, measured
baselines, production/proof/aggregate caps, helpers below 200, one local Result
Arc plus compact epoch, exact event/family/terminal/lifecycle proof and at most
one successor.

Write only canonical/current/this Stage/routing at <=40/<=200/<=180/<=30 net
lines and <=430 aggregate. STOP Rust/tests/oracles, callers/exports, direct
discovery or selected-graph activation, duplicate fetch/patch semantics, moved
child events, retained graph scratch, compatibility widening, M8/M7B or
milestone closure. Preserve M7A -> M8 -> M7B; implementation remains separately
design-gated.

### Selected-module-graph frontier registry-policy stop (2026-08-19)

The audit at `a4623d6b` returns formal REPLAN to
`WP-6-7A-registry-policy-observation-design`.

`RegistryPolicyKey` is the uniquely smallest complete owner. It projects
injected registry URLs, injected lockfile mode and root MODULE files into one
policy Result. Both local/remote `RegistryFileKey` branches consume it, and
`ModuleSourcePreparationKey` consumes it before the ordered registry attempts.
Its only path-bearing child is root files, whose observed sibling is accepted.

Registry file I/O/generation, root patches, module preparation, recursive
discovery and selected graph still lack complete carriers. Observing any of
those larger owners first would duplicate the shared policy prefix.
`HostRegistryFunctionKey` and `HostVisibleLockfileKey` belong to the separate
post-selected-graph repository-spec path and do not replace this exact owner.

### Registry-policy observation design (2026-08-19)

Freeze private crate-visible `RegistryPolicyObservationKey` and
`ObservedRegistryPolicy`. The carrier owns exactly one local policy Result Arc
plus the compact root-files epoch, with `Dupe`/`Allocative`, borrowed
accessors and no export/caller.

Use one Legacy/Observed driver in injected registry URLs -> injected lockfile
mode -> matching root-files order. Legacy selects only `RootModuleFilesKey`;
Observed selects only accepted `RootModuleFilesObservationKey`. URL/mode DICE
failures keep exact empty-prefix semantic errors. Legacy root compute failure is
semantic empty-prefix. Observed root Need/outer is carrierless, observed compute
failure is semantic empty-prefix, and every Complete root semantic/success
retains the exact root epoch unchanged.

There is only one path-bearing child, so the policy owner performs no epoch
union and adds no conflict/mismatch class. Complete carrier equality is semantic
Result+epoch, outer is by value, and Need is invalid/self-unequal. Parent is
eventless; root descendants retain sole batch ownership. Retain no child carrier
Arc, extra collection/cache/interner/store/lock/task/Host read/revision/
certificate/event state.

Future exact Rust authority after design ACCEPT is only
`app/slug_bzlmod_v2/src/registry_dice.rs`, baseline 1,413 physical, <=200
production, <=520 proof, <=720 aggregate semantic and <=2,200 physical; helpers
stay below 200 lines.

Proof identity/hash/Display/accessors/equality, exact legacy Result Arc, every
URL/mode/root compute and semantic terminal, root Need/outer and exact epoch
Arcs, exact family rows/reverse isolation, child-owned cold events/parent
silence/warm/cancel recovery, URL/mode/MODULE/lockfile A-B-A with held handles,
and zero registry-file/preparation/discovery/selected/HostRegistry/extension/
public activation.

Write only canonical/current/this Stage/routing at <=40/<=220/<=180/<=30 net
lines and <=470 aggregate. STOP Rust/tests/oracles, another key/file/caller/
export, registry-file/preparation/discovery/selected activation, semantic/event/
family/memory drift, cap excess or milestone closure. After independent design
ACCEPT schedule only `WP-6-7A-registry-policy-observation-implementation`;
after implementation ACCEPT return only to the selected-module-graph frontier
audit.

### Registry-policy observation design accepted (2026-08-19)

Design `8d00d44a` accepts `RegistryPolicyKey` as the uniquely smallest
complete shared registry-prefix owner and activates only
`WP-6-7A-registry-policy-observation-implementation` from Rust base
`a4623d6b`.

Exact authority is only `app/slug_bzlmod_v2/src/registry_dice.rs`, baseline
1,413 physical, <=200 production, <=520 proof, <=720 aggregate semantic and
<=2,200 physical; helpers remain below 200 lines.

Implement private `RegistryPolicyObservationKey` and `ObservedRegistryPolicy`
with one exact policy Result Arc plus the root epoch. One shared driver preserves
neutral registry URLs -> neutral lockfile mode -> matching root-files family.
URL/mode and root compute errors keep empty semantic prefixes; observed root
Need/outer is carrierless; Complete root semantic/success forwards the exact
epoch unchanged. No parent epoch union/conflict class exists.

Parent remains eventless; root descendants retain their batches. Retain no child
carrier, extra collection/cache/interner/store/lock/task/Host read/revision/
certificate/event state. Prove exact identity/Arc/equality/terminals, family
rows/epoch Arcs, events/warm/cancel, URL/mode/MODULE/lockfile lifecycle and zero
upper/HostRegistry activation.

STOP a second file/key/caller/export, registry-file/preparation/discovery/
selected activation, semantic/event/family/memory drift, cap excess or milestone
closure. After independent ACCEPT return only to the docs-only selected-module-
graph frontier audit.


### Registry-policy observation proof-cap correction (2026-08-19)

The implementation from `1cd4e65b` formally REPLANs to
`WP-6-7A-registry-policy-observation-proof-cap-correction-design`. Retain the
one-file Rust candidate from base `a4623d6b` and accepted design `8d00d44a`
non-writable during correction design.

Measured live accounting is +166 production/+609 proof/+775 aggregate at 2,188
physical lines. Production ownership, URLs -> mode -> matching root order,
empty/full prefix algebra, eventlessness and one Result Arc+epoch retention are
sound. Broad validation retains only the inherited core 245/246 stale visibility
wording. Exact child batches/family exclusion and independent URL/mode/MODULE/
lockfile A->B->A evidence cannot fit the former +520 proof ceiling without
deleting discriminators.

Two proof clauses are corrected to match DICE invariants. A failed injected
URL/mode key returns before activation tracking, so require its real semantic
error, empty epoch and zero root-files-family or later activation rather than a
nonexistent parent row. A naturally
computed typed outer cannot be injected through `PathObservationEpoch`, which
rejects invalid duplicate/conflicting/mismatched entries at construction; use
the production terminal projector plus the accepted lower root-owner outer
proof, with no test hook or invalid epoch. Real root Need still requires its
exact parent row and later suppression.

Phase-separate exact observed/legacy cold child `EventBatch` equality from the
parent dependency-row/event-silence proof because the parent sees the already
computed child as Reused. Require independent URL, mode, MODULE and lockfile
A->B->A, exact legacy Arc projection, carrier equality, reverse-family and
upper/extension/public exclusion, warm/cancel/recovery and held handles.

Retry authority remains only `registry_dice.rs`; correct ceilings to +200
production/+680 proof/+880 aggregate and 2,300 physical lines from the 1,413
baseline. STOP production semantic/event/family/memory change, a second file,
key/caller/export, test hook, upper activation or milestone closure. After
independent design ACCEPT schedule only
`WP-6-7A-registry-policy-observation-implementation-retry`; after retry ACCEPT
return only to the selected-module-graph frontier audit.

### Registry-policy proof correction accepted (2026-08-19)

Correction `de89907f` accepts the measured proof authority and activates only
`WP-6-7A-registry-policy-observation-implementation-retry` from Rust base
`a4623d6b` and semantic design `8d00d44a`.

Write only `app/slug_bzlmod_v2/src/registry_dice.rs`, baseline 1,413 physical,
at <=200 production, <=680 proof, <=880 aggregate semantic and <=2,300 physical;
helpers remain below 200 lines. Preserve the private Result-Arc+epoch carrier,
URLs -> mode -> matching root order, empty/full terminal prefixes, carrierless
Need/outer, eventless parent, child-owned batches and compact retention.

Prove exact Result Arc projection, equality, ordered epoch Arcs, independent
URL/mode/MODULE/lockfile A->B->A, phase-separated exact child batches, exact
family rows/reverse exclusion, warm/cancel/recovery and upper nonactivation.
Missing injected URL/mode errors require empty epochs and zero root-files-family
or later activation, not an impossible failed-key parent row. Typed outer uses
the production projector plus accepted lower-owner proof; add no invalid epoch
or test hook.

STOP a second file/key/caller/export, production semantic/event/family/memory
drift, upper activation, cap excess or milestone closure. After independent
ACCEPT return only to the docs-only selected-module-graph frontier audit.

### Registry-policy observation accepted and frontier resumed (2026-08-19)

Accepted `d0ebd79d` adds the private matching-family registry-policy carrier at
+166 production/+634 proof/+800 aggregate and 2,213 physical lines. Focused 3/3,
full bzlmod 457 plus integrations, loading 138 plus integrations and query
53+56+1+11 pass; core retains only the inherited 245/246 stale visibility
wording. Fmt/diff, Buck2 retention, cleanup and independent review pass.

Activate only
`WP-6-7A-selected-module-graph-observation-frontier-audit-resume`. Write only
canonical/current/this Stage/routing at <=40/<=220/<=180/<=30 net lines and
<=470 aggregate; Rust/tests/fixtures/oracles/public/callers are read-only.

Trace accepted nonregistry closure and registry policy through registry source
preparation, RegistryFile, patch/source preparation, discovery and selected
graph. For every candidate record structural identity, Result Arc/epoch owner,
exact dependency order and terminal polarity, child event authority, retained
lifetime and lifecycle evidence. Locate the first complete reusable producer;
do not reconstruct epochs or combine independent owners for convenience.

Return exactly one of: one smallest-owner docs-only design with measured future
scope/caps/proof; one uniquely smaller evidence/producer prerequisite; or formal
REPLAN. Do not preselect registry file/preparation, discovery or selected graph,
and authorize no implementation. After independent audit ACCEPT schedule at
most one design successor.

Exact compatibility remains current registry/nonregistry values/errors/order
and child events. Private observed carriers/epochs/outers are Slug-native.
Extensions, broader actions, M8/M7B and identity bytes remain deferred.

STOP code/tests/oracles, public/caller changes, direct selected-graph activation,
event/family drift, retained scratch/cache/interner/store/lock/task, direct Host
reads, unmeasured caps or milestone closure. M7 remains partial.

### Selected-module-graph frontier resumes at registry file (2026-08-19)

Audit `de76a83e` selects only `WP-6-7A-registry-file-observation-design`.
`RegistryFileKey` owns scheme dispatch and the complete semantic file result and
is shared below module-source preparation and selected repo-spec parsing.
Preparation still crosses raw patch path observations; discovery owns MODULE
evaluation/events; selected graph aggregates discovery. Dormant HostRegistry is
post-selected-graph policy, not a prerequisite.

Design future authority only in `app/slug_bzlmod_v2/src/registry_dice.rs`, clean
baseline 2,213 physical/first cfg(test) 905, at <=320 production, <=760 proof,
<=1,080 aggregate and <=3,400 physical; helpers stay below 200 lines.

Freeze private `RegistryFileObservationKey`/`ObservedRegistryFile`, one local
Result Arc plus compact epoch, one matching Legacy/Observed driver and exact
legacy Arc projection. Preserve scheme dispatch. Invalid/unsupported errors are
empty-prefix. Local legacy is policy -> legacy root -> IO; observed is observed
policy -> observed root -> IO, merging policy prefix left-first before root
semantics. Remote observed accepts only observed policy before unchanged
plan/IO/generation. Need/outer is carrierless; compute and semantic terminals
retain exact empty/policy/merged prefixes; no Need union.

Parent remains eventless and root descendants retain sole MODULE/lockfile batch
ownership. Retain no child carrier, IO scratch/handle, collection/cache/interner/
store/lock/task, Host read, revision, certificate or event state.

Prove identity/equality/legacy Arc, every scheme/policy/root/plan/IO/generation
terminal, exact prefix/order/ptrs, duplicate/conflict/mismatch, family rows,
phase-separated child batches/parent silence, warm/cancel/recovery, scripted
local+remote lifecycles and independent policy URL/mode/MODULE/lockfile A-B-A.
Assert zero patch/preparation/discovery/selected/HostRegistry/extension/public
activation.

Write only canonical/current/this Stage/routing at <=40/<=220/<=180/<=30 net
and <=470 aggregate. STOP Rust/tests/oracles/callers during design, a second
future file/key/export, IO semantic drift, upper activation, extra retained
state, cap excess or milestone closure. After independent ACCEPT schedule only
`WP-6-7A-registry-file-observation-implementation`; after implementation return
to the selected-graph frontier.

### Registry-file observation design accepted (2026-08-19)

Design `b7f66b06` activates only
`WP-6-7A-registry-file-observation-implementation` from Rust base `d0ebd79d`.
Write only `app/slug_bzlmod_v2/src/registry_dice.rs`, baseline 2,213/first test
boundary 905, at <=320 production, <=760 proof, <=1,080 aggregate and <=3,400
physical; helpers remain below 200 lines.

Preserve private Result-Arc+epoch ownership, scheme-first dispatch, matching
Legacy/Observed families, exact legacy Arc projection, local policy -> root ->
IO and remote policy -> IO order, policy-left epoch merge before semantics,
carrierless Need/outer, exact empty/policy/merged terminal prefixes and no Need
union. Parent is eventless; child ROOT batches remain child-owned; retention is
one local Result Arc+compact epoch without child carriers or extra state.

Complete the frozen scheme/policy/root/plan/IO/generation, Arc/order/prefix,
family/event/cancel/warm and independent local/remote/policy lifecycle proof.
Activate no patch/preparation/discovery/selected/HostRegistry/extension/public
consumer.

STOP a second file/key/caller/export, IO semantic/event/family/memory drift,
upper activation, cap excess or milestone closure. After independent ACCEPT
return only to the docs-only selected-module-graph frontier.

### Registry-file observation accepted; frontier resumed again (2026-08-19)

Accepted `0f9a0559` adds the private registry-file Result-Arc+epoch owner from
Rust base `d0ebd79d` and design `b7f66b06`. Exact accounting is +246
production/+760 proof/+1,006 aggregate at 3,219 physical lines, within
320/760/1,080/3,400. Focused observed-registry-file proof is 4/4; full bzlmod,
fmt/diff, cleanup, compact-retention and independent review pass.

The owner preserves scheme -> matching policy -> local root -> IO/generation,
or scheme -> matching remote policy -> plan/IO/generation. Policy-left union
keeps the earliest exact Arc before semantics; Need/typed outer is carrierless;
the parent is eventless and retains one local Result Arc plus the compact epoch.
No patch, preparation, discovery, selected, HostRegistry, extension or public
consumer is activated.

Activate only
`WP-6-7A-selected-module-graph-observation-frontier-audit-resume-2`. Write only
canonical/current/this Stage/routing at <=40/<=220/<=180/<=30 net lines and
<=470 aggregate; Rust/tests/fixtures/oracles/public/callers are read-only.

Trace the accepted nonregistry closure, registry policy and registry file
through module-source preparation, root patch resolution/FileBytes, discovery
evaluation/events and selected graph. Record each candidate's structural
identity, semantic Result and complete epoch owner, exact order and terminal
polarity, event ownership, retained lifetime, family boundary and lifecycle.
Do not reconstruct epochs, duplicate IO/parser semantics, move child events or
combine independent owners for convenience.

Return exactly one smallest-owner docs-only design with measured future scope,
caps and proof; one uniquely smaller prerequisite; or formal REPLAN. After
independent audit ACCEPT schedule at most one design successor.

Exact registry/nonregistry values/errors/order/events remain exact; private
carriers/epochs/outers are Slug-native; extensions, broader actions, M8/M7B and
identity bytes remain deferred. STOP code/oracles, callers/public exports,
direct selected activation, semantic/event/family/memory drift, unmeasured
authority or milestone closure. M7 stays partial and M7A -> M8 -> M7B remains.

### Selected-graph frontier stops at module source preparation (2026-08-19)

Audit `79b56e8a` selects only
`WP-6-7A-module-source-preparation-observation-design`. The preparation key is
the first complete owner of effective selection, the nonregistry/registry split,
ordered registry attempts and the two-phase root-patch pipeline. Accepted
effective/source/policy/registry-file/resolved-path carriers plus neutral
FileBytes close all lower path edges. Patch processing has no other consumer;
discovery only evaluates completed bytes and owns its MODULE batch, while
selected graph only joins discovery horizons.

This design is docs-only. Future Rust authority is exactly
`source_preparation.rs` at baseline 16,135, <=700 production/+60 colocated
proof and <=16,900 physical, plus
`source_preparation_observation_tests.rs` at baseline 5,262, <=1,440 proof and
<=6,800 physical. Aggregate caps are <=2,200 semantic and <=23,700 physical;
helpers/tests remain below 200 lines and no third file/export/caller is allowed.

Freeze a private crate-visible preparation observation key/carrier retaining one
local Result Arc plus the cumulative epoch. One Legacy/Observed driver preserves
normalize -> matching effective -> nonregistry source, or version/policy ->
ordered registries -> each registry file -> all patch resolutions in label
order -> for each retained resolution in order, FileBytes -> immediate
cumulative patch application. Early FileBytes or patch application failure
suppresses every later FileBytes. Legacy moves the exact Result Arc.

Merge each Complete child epoch into the existing prefix left-first before
semantic inspection, and append the exact FileBytes Arc before inspection.
Equal duplicates retain the earliest Arc; conflict/mismatch is typed outer.
Need/outer is immediate carrierless with no later activation or Need union.
Preserve exact empty/effective/policy/registry/resolution/full terminal prefixes,
including all-miss and patch errors.

Preparation stays eventless; ROOT/MODULE/lockfile children keep their batches
and discovery remains the later evaluation owner. Retain no child carrier,
resolved path, patch bytes/list, policy/search scratch, extra collection/cache/
interner/store/lock/task, Host read, revision, certificate or event state.

Prove identity/equality/exact legacy Arc; every stage and first/middle/last
attempt/patch terminal; exact prefix/order/ptr/conflict algebra; nonregistry,
override and multi-registry semantics; patch filtering/two-phase order; exact
family rows/events/warm/cancel; independent registry/policy/patch A-B-A and held
handles; and zero discovery/selected/repo-spec/HostRegistry/extension/public
activation.

Exact values/errors/search/patch order and child events remain exact. The
private carrier/outer/epoch association is Slug-native. Discovery, selected
graph/repo specs, extensions, M8/M7B and identity bytes remain deferred.

Write only canonical/current/this Stage/routing at <=40/<=220/<=180/<=30 net
and <=470 aggregate. STOP Rust/tests/oracles during design, a third future file,
caller/export, reordered IO/patches, event/family/memory drift, upper activation,
cap excess or milestone closure. After independent design ACCEPT schedule only
`WP-6-7A-module-source-preparation-observation-implementation`; after its
acceptance return only to the selected-graph frontier for discovery.

### Module-source preparation observation design accepted (2026-08-19)

Design `5436a421` activates only
`WP-6-7A-module-source-preparation-observation-implementation` from Rust base
`0f9a0559`. Write exactly `source_preparation.rs` and
`source_preparation_observation_tests.rs` at the frozen 16,135/5,262 baselines:
<=700 production/+60 colocated proof/16,900 and <=1,440 external proof/6,800,
<=2,200 semantic and <=23,700 physical aggregate. Helpers/tests stay below 200.

Preserve the private Result-Arc+epoch carrier and one Legacy/Observed driver.
Order is normalize -> matching effective -> nonregistry source, or
version/policy -> ordered registry files -> all patch resolutions in label order
-> for each retained resolution, FileBytes then immediate cumulative apply.
Merge Complete epochs left-first before semantics; append FileBytes before
inspection; Need/outer is carrierless with later suppression and no Need union.
Legacy moves the exact Result Arc.

Preparation remains eventless; children retain ROOT/MODULE/lockfile batches and
discovery remains the later evaluation owner. Retain no child carrier/resolved
path/patch scratch or extra collection/cache/interner/store/lock/task/Host read/
revision/certificate/event state.

Complete the frozen stage, Arc/order/prefix/conflict, nonregistry/registry,
patch, family/event/warm/cancel and independent lifecycle proof, including early
patch apply failure suppressing later FileBytes. Activate no discovery/selected/
repo-spec/HostRegistry/extension/public consumer.

STOP a third file/export/caller, reordered IO/patch semantics, event/family/
memory drift, upper activation, cap excess or milestone closure. REPLAN before
wider authority. After independent ACCEPT return only to the docs-only
selected-module-graph frontier for discovery.

### Module-source preparation proof-cap stop (2026-08-20)

Implementation review of the retained candidate from Rust base `0f9a0559`
accepts its production owner and one-driver structure. Exact order is matching
effective -> source or policy -> ordered registry files -> resolve every patch
-> each FileBytes plus immediate apply. Complete epochs merge left-first before
semantics, Need/outer is carrierless, the parent remains eventless, and retained
state is exactly the local Result Arc plus compact epoch. No production,
retention, family, event or cleanup redesign is required.

The proof cap is not sufficient for the accepted matrix. Live measured
accounting is +344 net at 16,479 physical in `source_preparation.rs`, +1,332
proof at 6,594 physical in `source_preparation_observation_tests.rs`, and
+1,676/23,073 aggregate. The existing proof discriminates the shared reducer,
exact registry/nonregistry families and child batches, neutral FileBytes,
warm/cancellation recovery, URL and patch metadata/bytes restoration and upper
nonactivation. The remaining 108 external plus 60 colocated proof lines cannot
also cover production-called compute/semantic projectors, exact mismatch outer,
every patch terminal at first/middle/last positions, and independent
registry/lockfile/symlink lifecycles without deleting those accepted
discriminators.

Formally REPLAN only to
`WP-6-7A-module-source-preparation-observation-proof-cap-correction-design`.
During correction, the two dirty Rust files are retained and non-writable.
Write only canonical/current/this Stage/routing at <=40/<=220/<=180/<=30 net
and <=470 aggregate.

Freeze the same two-file retry authority and production contract. Keep
`source_preparation.rs` at <=700 production, <=60 colocated proof and <=16,900
physical. Raise only `source_preparation_observation_tests.rs` to <=1,800 proof
and <=7,150 physical, making <=2,560 semantic and <=24,050 physical aggregate.
No third file, export, caller, upper activation, semantic/event/family/memory
change or milestone closure is permitted.

The retry must use production-called stable finishers/projectors to table exact
effective/source/policy/registry-file/resolution/FileBytes compute and semantic
prefixes, carrierless Need/outer, operation-mismatch propagation and later
suppression. It must cover nonregistry Absent/error; patch skip/invalid/missing/
wrong-kind/resolution compute+semantic; first/middle/last resolution and
FileBytes Need/outer/compute/Missing/Error; and exact parse/apply suppression.
Extend parent lifecycles through registry bytes, URL/mode/MODULE/lockfile and
patch symlink/path/bytes A-B-A with held Result/epoch handles. Preserve the
already discriminating family rows, exact child batches, warm, cancellation,
Arc, ordering and upper exclusions.

After independent correction ACCEPT schedule exactly
`WP-6-7A-module-source-preparation-observation-implementation-retry`. Only
after retry ACCEPT return to the docs-only selected-module-graph frontier for
discovery. M7 stays partial and M7A -> M8 -> M7B remains.

### Module-source preparation proof authority corrected (2026-08-20)

Correction `2c505e13` activates only
`WP-6-7A-module-source-preparation-observation-implementation-retry` from
Rust base `0f9a0559` and semantic design `5436a421`. Write exactly
`source_preparation.rs` and `source_preparation_observation_tests.rs`.

Keep source authority <=700 production/+60 colocated proof and <=16,900
physical. External proof is <=1,800/7,150; aggregate is <=2,560 semantic and
<=24,050 physical. Helpers/tests remain below 200; no third file/export/caller
is writable.

Preserve the single Legacy/Observed driver, exact effective/source/policy/
registry/resolution/FileBytes order, resolve-all then per-resolution
FileBytes->immediate-apply sequencing, left-first exact-Arc merging,
carrierless Need/outer, child-only events and one Result Arc+compact epoch
retention.

Complete production-called compute/semantic prefix projectors, exact operation
mismatch, nonregistry Absent/error, every patch terminal at first/middle/last
positions, later suppression and independent registry bytes, URL/mode/MODULE/
lockfile and patch symlink/path/bytes lifecycles. Preserve accepted family rows,
exact child batches, warm/cancel/recovery, Arc/order and upper exclusions.

STOP semantic/event/family/memory drift, wider authority, proof waiver, cap
excess or milestone closure. After independent implementation ACCEPT return
only to the docs-only selected-module-graph frontier for discovery.

### Module-source preparation second proof-cap stop (2026-08-20)

Terminal review of the retained retry from scheduling base `3e83a42d`, Rust
base `0f9a0559`, semantic design `5436a421` and first correction `2c505e13`
accepts the production owner, shared driver, exact ordering/prefix algebra,
event ownership and compact retention. Focused new proof and the complete
`slug_bzlmod_v2` suite pass.

Measured candidate accounting is +367 net at 16,502 physical in
`source_preparation.rs`, +1,780 proof at 7,042 physical in
`source_preparation_observation_tests.rs`, and +2,147 semantic/23,544 physical
aggregate. The retry closes production-called compute projection,
OperationMismatch, nonregistry Absent/error, first/middle/last resolution and
FileBytes terminals, registry-byte and root-MODULE lifecycles while preserving
the accepted family/event/warm/cancel/upper matrix.

Four frozen discriminators remain: distinct observed-key equality/hash; exact
multi-patch cumulative epoch reconstruction from effective, policy, decisive
registry-file, reached resolutions and reached FileBytes carriers; lockfile
mode/content A-B-A; and patch symlink-retarget A-B-A. Only 20 external proof
lines remain, so completing these honestly under the first correction would
replace lifecycle evidence with compressed inference.

Formally REPLAN only to
`WP-6-7A-module-source-preparation-observation-proof-cap-correction-2-design`.
The two Rust files remain retained and non-writable. Write only canonical,
current, this Stage and routing at <=40/<=220/<=180/<=30 net and <=470
aggregate.

Keep `source_preparation.rs` at <=700 production/+60 colocated proof and
<=16,900 physical. Raise only external proof to <=2,100/7,500, yielding
<=2,860 semantic and <=24,400 physical aggregate. Freeze every production,
event, family, memory and two-file owner term.

The retry must add the exact distinct-key hash proof, reconstruct and compare
the multi-child cumulative epoch's iteration order and shared Arcs, and drive
independent mode/lockfile and symlink-retarget A-B-A with held Result/epoch
handles. Preserve all existing exact prefix, terminal-position, family, event,
warm, cancellation, lifecycle and upper-exclusion proof.

STOP Rust during correction, a production change, third file/export/caller,
upper activation, proof waiver, cap excess or milestone closure. After
independent correction ACCEPT schedule exactly
`WP-6-7A-module-source-preparation-observation-implementation-retry-2`; only
after retry ACCEPT return to the selected-module-graph frontier for discovery.

### Module-source preparation second proof authority corrected (2026-08-20)

Correction `77904203` activates only
`WP-6-7A-module-source-preparation-observation-implementation-retry-2` from
Rust base `0f9a0559`, semantic design `5436a421` and first correction
`2c505e13`. Write exactly `source_preparation.rs` and
`source_preparation_observation_tests.rs`.

Keep source authority <=700 production/+60 colocated proof and <=16,900
physical. External proof is <=2,100/7,500; aggregate is <=2,860 semantic and
<=24,400 physical. Helpers/tests remain below 200; no third file/export/caller
is writable.

Preserve the single Legacy/Observed driver, exact effective/source/policy/
registry/resolution/FileBytes order, resolve-all then per-resolution
FileBytes->immediate-apply sequencing, left-first exact-Arc merging,
carrierless Need/outer, child-only events and one Result Arc+compact epoch.

Complete distinct key equality/hash, exact cumulative child epoch order/shared
Arcs, independent mode+lockfile A-B-A and symlink-retarget A-B-A with held
handles. Preserve every accepted prefix/terminal-position/family/event/warm/
cancel/lifecycle/upper discriminator.

STOP production/event/family/memory drift, wider authority, proof waiver, cap
excess or milestone closure. After retry ACCEPT return only to the docs-only
selected-module-graph frontier for discovery.

### Module-source preparation observation accepted (2026-08-20)

Implementation `223c8112` completes the owner designed in `5436a421` and
corrected by `2c505e13` and `77904203`. Against Rust base `0f9a0559`, the
accepted two-file delta is +367 net at 16,502 physical in
`source_preparation.rs` and +1,969 proof lines at 7,231 physical in
`source_preparation_observation_tests.rs`: +2,336 semantic and 23,733 physical
aggregate, within every corrected cap.

The accepted shared driver preserves effective -> nonregistry source or
registry policy -> ordered registry-file attempts -> resolve-all patches ->
per-resolution FileBytes/immediate-apply order. Complete epochs merge
left-first before semantics; Need/outer is carrierless; child keys retain sole
event ownership; and the carrier retains exactly one Result Arc plus a compact
epoch. Final proof discriminates distinct key/hash identity, exact cumulative
child order/shared Arcs, independent mode/lockfile restoration and symlink
retarget restoration while preserving every accepted prefix, family, event,
warm, cancellation, lifecycle and upper exclusion.

Focused proof is 12/12. The full `slug_bzlmod_v2` suite passes 475 unit tests
plus all integrations/docs; fmt and diff-check pass. Two independent terminal
reviews accept production, proof, compact retention, helpers/tests below 200
lines and the exact two-file scope.

### Selected-module-graph observation frontier audit resumed (2026-08-20)

Activate only
`WP-6-7A-selected-module-graph-observation-frontier-audit-resume-3`.
This is a docs-only, read-only-Rust audit. Write exactly canonical, current,
this Stage and the orchestration routing log.

Net docs caps are canonical <=40, current <=220, Stage <=180 and routing <=30,
with <=470 aggregate.

Trace the accepted nonregistry closure and module-source preparation into
`HostDiscoveredModuleKey`, then into the selected-graph join. Inspect discovery
evaluation/Result/event ownership, recursive dependency horizon and every
remaining carrierless mutable/path edge. Inspect selected repo-spec, extension
and public consumers only enough to prove that they are later owners or a
necessary part of the smallest candidate. Do not reopen accepted source search,
patch or epoch ownership.

The audit must establish exact key/Result/Arc/epoch association, sequential or
joined Need/outer/error precedence, event ownership/order, matching families,
cancellation/warm behavior, lifecycle restoration, retained-state lifetime and
Buck2 memory shape. Preserve current Bazel 9 behavior as exact; private typed
outer/epoch association is Slug-native; selected repo specs/extensions, broader
bootstrap work, M8/M7B and exact identity bytes remain deferred unless live
owner evidence requires them.

Terminate with exactly one independently reviewable docs-only frozen design for
the uniquely smallest complete owner, one uniquely smaller prerequisite design,
or formal REPLAN with contradictory evidence and one smallest next audit/design.
At most one successor may be scheduled, and no Rust authority exists before
independent design ACCEPT.

STOP direct implementation, a speculative discovery/selected-graph carrier,
moving or duplicating child event ownership, weakened Arc/epoch equality,
retained collection/cache/interner/store/lock/task/Host reads, upper/public
activation, M7 acceptance, M8/M7B work or exact identity-byte work. M7 remains
partial and M7A -> M8 -> M7B remains.

### Selected frontier chooses discovered-module owner (2026-08-20)

The read-only frontier audit at `cc95fe3f` and two independent owner reviews
select `HostDiscoveredModuleKey` as the uniquely smallest complete next owner.
It always reads effective selection, then chooses immutable builtin content,
accepted nonregistry closure or accepted registry preparation. It alone
evaluates the chosen MODULE and owns the local evaluation batch.
`HostSelectedModuleGraphKey` only compute-joins discovery leaves into
BFS/fixed-point horizons; observing the graph first would reconstruct child
epochs and move or duplicate discovery's event authority. Selected repo-spec
and extension consumers are later. No smaller carrierless prerequisite remains.

Activate only `WP-6-7A-host-discovered-module-observation-design`. During this
design write only canonical/current/this Stage/routing at <=40/<=220/<=180/
<=30 net and <=470 aggregate. Rust, tests, fixtures, oracles, callers and public
exports remain read-only.

After independent design ACCEPT, future authority is exactly:

- `source_preparation.rs`, baseline 16,502, <=360 production/+40 colocated
  proof and <=16,950 physical; and
- `source_preparation_observation_tests.rs`, baseline 7,231, <=820 proof and
  <=8,100 physical.

Aggregate is <=1,220 semantic and <=25,050 physical; helpers/tests remain below
200 and no third file/export/caller is writable.

Add private crate-visible `HostDiscoveredModuleObservationKey` and
`ObservedHostDiscoveredModule`. Retain exactly one local semantic Result Arc
plus one compact epoch with `Dupe`/`Allocative` and borrowed accessors. Use
one Legacy/Observed driver and a private typed outer that preserves effective,
closure and preparation frontier classes. Legacy projection moves the exact
local Result Arc.

Exact order is matching effective first; then builtin validation/neutral
builtin and immediate termination without discovery evaluation/batch, or
nonregistry empty-version validation/matching closure, or registry missing-
version validation/matching preparation. Only nonregistry and registry then run
owner-local evaluation and the matching discovery batch. Legacy selects only
legacy children; observed selects only accepted observed siblings. Builtin
stays neutral.

Merge each observed Complete child epoch left-first before semantics. Equal
duplicates keep the earliest Arc; conflict/mismatch is typed outer. Any child
Need/outer is immediate carrierless, suppresses later children/evaluation and
stores no parent batch. Effective compute has empty prefix. Effective semantics
and pure/builtin terminals keep effective; closure/preparation compute keeps
effective; their semantic/evaluation terminals keep effective+child. No Need
union occurs.

Need is invalid/self-unequal; Complete outer is outer-by-value; Complete carrier
equality is semantic Result+epoch. For nonregistry/registry each discovery
sibling remains sole owner of its matching local MODULE batch; evaluation
success/error stores one batch including empty under capture. Builtin stores no
discovery-local batch; other pre-evaluation terminals, Need/outer/cancel store
none. Child batches precede discovery and warm reuse is silent.

Retain no child carrier Arc, included-fragment Vec, logical-id/evaluator/event
scratch, extra collection, cache/interner/store/lock/task/direct Host read,
revision or certificate. Existing semantic provenance remains inside the local
Result and all other scratch is compute-local.

Proof covers identity/hash/Display/accessors/equality/validity and exact legacy
Result-Arc projection; every effective/builtin/nonregistry/registry validation,
Need/outer/compute/semantic/evaluation position and prefix; duplicate first Arc,
conflict/mismatch; real builtin/nonregistry/registry terminals; exact cumulative
order/shared Arcs; exact dependency rows and reverse family isolation; exact
child/discovery batches including empty/error, warm and cancellation/recovery;
independent effective/closure/preparation/evaluated-MODULE A-B-A with held
Result/epoch; explicit builtin zero discovery batch with neutral child behavior;
and zero selected/repo-spec/extension/public activation.

Exact compatibility is current branch/value/error/order/legacy Arc and event
behavior. Slug-native is the private carrier/typed outer/epoch association.
Selected graph/repo specs/extensions, broader bootstrap work, M8/M7B and exact
identity bytes remain deferred.

STOP a third file/export/caller, selected activation, event-owner drift,
semantic/family/order drift, weakened Arc/epoch association, retained-state
growth, direct Host read, cap excess or milestone closure. REPLAN before
widening. After independent design ACCEPT schedule exactly
`WP-6-7A-host-discovered-module-observation-implementation`; only after that
implementation ACCEPT return to the docs-only selected-module-graph frontier.

### Discovered-module observation design accepted (2026-08-20)

Design `b8e4cc03` activates only
`WP-6-7A-host-discovered-module-observation-implementation` from Rust base
`223c8112`. Write exactly `source_preparation.rs` and
`source_preparation_observation_tests.rs`.

Keep source authority <=360 production/+40 colocated proof at <=16,950 physical,
external proof <=820/8,100, and aggregate <=1,220 semantic/25,050 physical.
Helpers/tests remain below 200; every third file/export/caller is read-only.

Preserve the private discovered key/carrier and typed effective/closure/
preparation outer, one Legacy/Observed driver, matching-family effective then
builtin/nonregistry/registry branch order, left-first Complete merging before
semantics, carrierless Need/outer, exact legacy Result-Arc projection, and one
local Result Arc+compact epoch retention.

Builtin terminates after neutral `BuiltinBazelToolsModuleKey` and stores no
discovery-local batch. Only nonregistry/registry proceed to owner-local MODULE
evaluation and the matching one-batch publication; pre-evaluation terminals,
Need/outer/cancel store none. Preserve every exact prefix, first-Arc/conflict/
mismatch, child event, family, warm, cancellation, lifecycle and upper
nonactivation discriminator.

STOP a third file/export/caller, selected-graph activation, semantic/order/
family/event-owner drift, retained-state growth, direct Host read, proof waiver,
cap excess or milestone closure. REPLAN before widening. After independent
implementation ACCEPT return only to the docs-only selected-module-graph
frontier. M7 remains partial and M7A -> M8 -> M7B remains.
### Discovered-module observation proof-cap REPLAN (2026-08-20)

The retained two-file candidate from design `b8e4cc03` is production-sound.
One private observed discovery owner and one Legacy/Observed driver preserve
effective -> builtin/nonregistry/registry order, left-first Complete merging,
carrierless Need/outer, exact legacy Result-Arc projection, builtin zero-parent-
batch behavior, nonregistry/registry evaluation ownership and one local Result
Arc plus compact epoch retention. Independent review found no production,
ownership, event, family, memory or cleanup defect.

Measured against Rust base `223c8112`, `source_preparation.rs` is +309 at
16,811 physical and `source_preparation_observation_tests.rs` is +809 at 8,040
physical: +1,118 semantic and 24,851 physical aggregate. Six focused discovery
tests and complete bzlmod validation pass. Only 11 external proof lines remain,
which cannot honestly cover the missing real branch terminals, complete exact
family/event sequences and separated held-handle lifecycles.

Formally REPLAN only to
`WP-6-7A-host-discovered-module-observation-proof-cap-correction-design` from
scheduling base `858e9b8e`. The two dirty Rust files are retained and
non-writable. Write only canonical/current/this Stage/routing at <=40/<=220/
<=180/<=30 net and <=470 aggregate.

Freeze production and the same two-file authority. Keep source at <=360
production/+40 colocated proof and <=16,950 physical. Raise only external proof
to <=1,200 and <=8,500 physical, making <=1,600 semantic and <=25,450 physical
aggregate. Helpers/tests remain below 200; no third file/export/caller is
writable.

The retry must add real builtin success/explicit-override/invalid-version,
nonregistry closure-semantic/cycle and reachable registry missing-version/
preparation/evaluation terminals with exact prefixes and later suppression.
Use production-called discovery compute/semantic/invariant projectors plus
accepted lower typed builtin-error proof for unreachable classes; forbid hooks
or inconsistent child injection. Compare the complete ordered neutral-child-to-
discovery EventBatch sequences, legacy/observed parity, exact rows and reverse-
family isolation for all three branches, including empty/error batches. Separate
effective, closure, preparation and evaluated-MODULE A-B-A restoration and
retain original/restored Result+epoch handles so child-epoch propagation cannot
be masked by evaluation-semantic changes.

Preserve every accepted key/equality/Arc/prefix/outer/family/event/warm/cancel/
upper discriminator, exact branch/result/error/order/events, child-owned event
authority and compact one-Result-Arc+epoch retention. STOP any production
semantic/event/family/memory change, wider authority, upper activation, proof
waiver, cap excess or milestone closure.

After independent correction ACCEPT schedule exactly
`WP-6-7A-host-discovered-module-observation-implementation-retry`; only after
retry ACCEPT return to the docs-only selected-module-graph frontier. M7 remains
partial and M7A -> M8 -> M7B remains.

### Discovered-module proof authority corrected (2026-08-20)

Correction `b09d5e70` activates only
`WP-6-7A-host-discovered-module-observation-implementation-retry` from Rust
base `223c8112` and semantic design `b8e4cc03`. Write exactly
`source_preparation.rs` and `source_preparation_observation_tests.rs`.

Keep source authority <=360 production/+40 colocated proof and <=16,950
physical. External proof is <=1,200/8,500; aggregate is <=1,600 semantic and
<=25,450 physical. Helpers/tests remain below 200; no third file/export/caller
is writable.

Preserve the one Legacy/Observed driver, exact effective then builtin/
nonregistry/registry order, left-first Arc/prefix algebra, carrierless Need/
outer, exact legacy projection, builtin zero-parent-batch behavior,
nonregistry/registry event ownership and one Result Arc+compact epoch retention.

Complete only real reachable branch terminals, production-called invariant
projectors plus accepted lower typed proof for unreachable classes without hooks
or inconsistent child injection, exact complete family/event sequences and
separated held-handle effective/closure/preparation/evaluation lifecycles.
Preserve every existing identity/prefix/Arc/family/event/warm/cancel/upper
discriminator.

STOP production semantic/event/family/memory drift, wider authority, upper
activation, proof hook/waiver, cap excess or milestone closure. REPLAN before
widening. After independent implementation ACCEPT return only to the docs-only
selected-module-graph frontier. M7 remains partial and M7A -> M8 -> M7B remains.

### Discovered-module observation accepted; selected frontier resumed (2026-08-20)

Accepted implementation `c6b1e108` completes design `b8e4cc03` under proof
correction `b09d5e70` from Rust base `223c8112`. The shared driver preserves
effective -> builtin/nonregistry/registry order, left-first Complete merging,
carrierless Need/typed outer and exact legacy Result projection. Builtin owns no
discovery-local batch; nonregistry and registry retain their exact evaluation
batches. The carrier retains only one local Result Arc plus compact epoch.

Exact accepted accounting is +309/16,811 physical in `source_preparation.rs`,
+1,175/8,406 in external proof and +1,484/25,217 aggregate. Focused discovery
proof passes 10/10; full bzlmod validation passes 485 unit tests plus every
integration/doc test. Fmt/diff hygiene and independent production/proof/
retention/cleanup review pass. M7 remains partial.

Activate only
`WP-6-7A-selected-module-graph-observation-frontier-audit-resume-4`. This is a
docs-only, read-only-Rust audit. Write exactly canonical, current, this Stage
and routing. Net docs caps are canonical <=40, current <=220, Stage <=180 and
routing <=30, with <=470 aggregate.

Trace accepted discovered-module observation leaves through
`HostSelectedModuleGraphKey`'s BFS/fixed-point join and every direct consumer.
Audit discovery request/horizon ordering and every remaining mutable/path or
carrierless edge. Inspect selected repo-spec, extension and public consumers
only enough to distinguish a later owner from a necessary smaller prerequisite.
Do not reopen accepted lower ownership for structural uniformity.

Require exact Result/Arc/epoch association; full-batch Need/typed-outer/
semantic precedence and suppression; matching families; child event ownership
and order; warm/cancel recovery; mixed nonregistry/registry/recursive lifecycle;
and compact retention with all join/frontier/event scratch compute-local. Exact
compatibility is existing branch/result/error/order/events. Private typed outer
and epoch association are Slug-native. Repo specs/extensions, broader bootstrap,
M8/M7B and exact identity bytes remain deferred absent contrary owner evidence.

Terminate with exactly one independently reviewable smallest-owner design, one
uniquely smaller prerequisite design, or formal REPLAN with one smallest next
audit/design. At most one successor may be scheduled and no Rust authority
exists before independent design ACCEPT.

STOP direct implementation, speculative selected/public activation, moved or
duplicated child events, weakened Arc/epoch equality, retained-state growth,
proof waiver, cap excess, M7 acceptance, M8/M7B or exact identity-byte work.
Preserve M7A -> M8 -> M7B.

### Selected frontier chooses the selected-module graph (2026-08-20)

The read-only resume-4 audit at `e399cd10` and independent owner review select
`HostSelectedModuleGraphKey` as the uniquely smallest complete aggregate. It
alone sequences accepted root files, neutral command policy, candidate
effective overrides, root/discovered transformations, repeated BFS horizons/
fixed-point and graph selection. Owner-local raw transformations/cache have no
other consumer. Every mutable child now has an accepted observed sibling.

`HostSelectedRegistryRepoSpecsKey` and `HostSelectedModuleRoutesKey` consume the
graph and add later repo-spec/route semantics. They cannot absorb the graph
without duplicating ownership. No smaller prerequisite remains.

Activate only `WP-6-7A-host-selected-module-graph-observation-design`. Write
only canonical/current/this Stage/routing at <=40/<=220/<=180/<=30 net and
<=470 aggregate. Rust base `c6b1e108`, tests, fixtures, oracles, callers and
public exports are read-only until independent design ACCEPT.

Future authority is exactly `selected_graph.rs` baseline 1,592/test boundary
907 at <=520 production/+320 colocated proof/2,450 physical, plus
`source_preparation_observation_tests.rs` baseline 8,406 at <=1,500 proof/
10,000 physical; <=2,340 semantic/12,450 physical aggregate. Helpers/tests stay
below 200; no third file/export/caller is writable.

Freeze a private graph observation key/carrier with one exact local Result Arc
plus cumulative compact epoch, `Dupe`/`Allocative`, borrowed accessors and no
caller. Use one Legacy/Observed driver with matching root-files/effective/
discovery families and shared pure parse/transform/BFS/fixed-point/select logic.
Preserve exact root -> policy -> ordered candidate overrides -> root transforms
-> first-seen BFS horizons -> horizon-ordered raw transforms -> fixed point ->
select/rewrite order. Append an effective epoch only on its first cached
computation; repeated equal epochs preserve the earliest Arc.

Merge sequential Complete epochs left-first before semantics. Root compute has
empty prefix; policy/effective/transform/select failures retain the exact reached
prefix. Each horizon scans full input order and merges all Complete carrier
epochs before selection. First horizon-ordered merge conflict/mismatch or child
outer is carrierless fail-closed; otherwise preserve exact legacy precedence:
first compute/semantic leaf error > incompatible Need > compatible Need union >
ordered success. Need/outer discards provisional state; Complete semantic error
retains the full valid merged sibling epoch and suppresses later work.

Graph siblings remain eventless; discovery children keep sole batch ownership
and order even on graph Need/error. Warm is silent; cancel publishes no graph
state. Retain no raw/traversal/frontier/prior-name/seen/cache/outcome/event/child-
carrier state; graph Arc slices remain existing semantic Result state and all
join/merge scratch stays compute-local.

Proof covers identity/equality/legacy Result Arc, exact merge errors and prefix
positions, full-horizon terminal algebra, exact families/rows/events, implicit
bazel_tools, duplicate candidate, diamond/cycle, nodep second round, mixed
nonregistry+registry horizons, warm/cancel recovery, independent root/policy/
effective/discovery/recursive/mixed A-B-A with held Arcs, and zero later-owner/
public activation.

Exact compatibility is existing graph values/errors/order/events. Slug-native
is private typed outer/epoch association. Repo specs/routes/extensions/public,
broader M7A, M8/M7B and exact identity bytes remain deferred.

STOP third-file/caller/export work, legacy precedence or child-event drift,
retained traversal state, weakened Arc association, cap excess or milestone
closure. REPLAN before widening. After independent design ACCEPT schedule only
`WP-6-7A-host-selected-module-graph-observation-implementation`; only after its
ACCEPT return to the docs-only selected frontier. M7 remains partial and
M7A -> M8 -> M7B remains.

### Selected-module-graph observation design accepted (2026-08-20)

Accepted design `09177514` from Rust base `c6b1e108` selects
`HostSelectedModuleGraphKey` as the first complete aggregate after the
accepted root-files, effective-override and discovered-module carriers. Activate
only `WP-6-7A-host-selected-module-graph-observation-implementation`.

Write exactly `selected_graph.rs` at baseline 1,592/test boundary 907 with
<=520 production/+320 colocated proof and <=2,450 physical, plus
`source_preparation_observation_tests.rs` at baseline 8,406 with <=1,500
proof/10,000 physical. Aggregate authority is <=2,340 semantic/12,450 physical;
helpers/tests remain below 200 and no third file/export/caller is writable.

Preserve one Legacy/Observed driver and exact root-files -> neutral policy ->
ordered candidate effective overrides -> root transforms -> first-seen BFS
horizons -> horizon-ordered raw transforms -> fixed point -> select/rewrite
order. Append effective epochs only on first computation and merge Complete
epochs left-first before semantics, preserving the earliest equal Arc.

Every horizon scans all Complete carriers in input order before selection.
Merge conflict/mismatch or child typed outer is carrierless fail-closed;
otherwise preserve first compute/semantic error > incompatible Need >
compatible Need union > ordered success. Need/outer discards provisional state;
Complete semantic errors retain the full valid sibling prefix and suppress later
work.

Graph siblings remain eventless and discovery children keep sole batch
ownership. Retain exactly one local Result Arc plus compact epoch; all raw,
traversal, frontier, cache, outcome, event and merge state remains compute-local.
Complete the frozen identity/Arc/prefix/horizon/family/event/warm/cancel/
lifecycle/upper proof without hooks or caller activation.

STOP third-file/caller/export work, precedence or child-event drift, retained
traversal state, weakened Arc association, cap excess or milestone closure.
REPLAN before widening. After independent implementation ACCEPT return only to
the docs-only selected frontier. M7 remains partial; preserve M7A -> M8 -> M7B.

### Selected-module-graph observation proof-cap REPLAN (2026-08-20)

Formal correction packet
`WP-6-7A-host-selected-module-graph-observation-proof-cap-correction-design`
retains the two-file Rust candidate from implementation scheduling base
`c0623a4b`, Rust base `c6b1e108` and accepted design `09177514`. Rust is
non-writable during this docs-only correction.

Measured candidate accounting is +370 production/+295 colocated proof in
`selected_graph.rs`, +807 external proof, +1,472 semantic aggregate, and
2,257/9,213/11,470 physical. Focused graph proof passes 21/21; full bzlmod
validation passes 494 unit tests plus all integration/doc tests; fmt/diff
hygiene pass.

Independent review accepts production ownership and the corrected horizon fold.
One private Legacy/Observed graph driver preserves root -> neutral policy ->
ordered candidate effective overrides -> transforms -> first-seen BFS horizons
-> fixed point -> select/rewrite. Sequential epochs merge left-first before
semantics. Every horizon now attempts all Complete epoch merges in input order
while retaining the first horizon-ordered child or merge outer. Graph siblings
remain eventless; discovery batches stay child-owned. Retention is exactly one
local Result Arc plus compact epoch, with traversal/frontier/cache/outcome/event/
merge state compute-local.

The old proof caps cannot honestly hold the remaining frozen discriminators.
Synthetic stage errors do not exercise live root/policy/effective/transform/
select boundaries; candidate first/middle/last Need/outer suppression is absent;
and semantic/real Discovery outer versus merge horizon ordering is incomplete.
Pure legacy topology tests do not prove observed dependency/epoch/event
association for implicit bazel_tools, duplicate-candidate cache-first,
diamond/cycle/nodep second rounds, recursive or mixed horizons. Complete exact
event sequences and independent policy/effective/recursive/mixed held-carrier
restoration also remain.

Correction write authority is canonical/current/this Stage/routing only, at net
caps <=40/<=220/<=180/<=30 and <=470 aggregate. The same implementation retry
may write exactly:

- `selected_graph.rs`, baseline 1,592/test boundary 907, <=520 production,
  <=480 colocated proof and <=2,650 physical; and
- `source_preparation_observation_tests.rs`, baseline 8,406, <=2,300 proof and
  <=10,800 physical.

Aggregate retry authority is <=3,300 semantic and <=13,450 physical. Helpers/
tests remain below 200; no third file/export/caller is writable.

Add only production-used live-stage and full-horizon terminal proof; real
observed/legacy implicit-bazel_tools, duplicate, diamond, cycle, nodep,
recursive and mixed topology rows/epochs/complete ordered EventBatch sequences;
and independent policy/effective/nonregistry/registry/recursive/mixed A -> B ->
A held-carrier restoration with unaffected Arc preservation and success/error/
cancel upper exclusion. Use no hooks or inconsistent child injection.

Preserve the accepted full-horizon correction: validate every Complete carrier
merge after the first outer without replacing that first horizon-ordered outer.
Freeze all production semantics, legacy precedence, family/event ownership,
left-first Arc association and one-Result-Arc+epoch retention. Exact
compatibility remains graph values/errors/order/events; the private typed outer
and shared-Arc epoch association remain Slug-native. Repo specs/routes/
extensions/public owners, broader M7A, M8/M7B and exact identity bytes remain
deferred.

STOP production semantic/event/family/memory change, a third file/caller/export,
upper activation, proof hook/waiver, cap excess or milestone closure. After
independent correction ACCEPT schedule only
`WP-6-7A-host-selected-module-graph-observation-implementation-retry`; after
retry ACCEPT return only to the docs-only selected frontier. M7 remains partial
and M7A -> M8 -> M7B remains.

### Selected-module-graph proof correction accepted (2026-08-20)

Accepted correction `19cc508d` from Rust base `c6b1e108` and semantic design
`09177514` activates only
`WP-6-7A-host-selected-module-graph-observation-implementation-retry`.

Write exactly `selected_graph.rs` at baseline 1,592/test boundary 907 with
<=520 production/+480 colocated proof and <=2,650 physical, plus
`source_preparation_observation_tests.rs` at baseline 8,406 with <=2,300
proof/10,800 physical. Aggregate authority is <=3,300 semantic/13,450 physical;
helpers/tests remain below 200 and no third file/export/caller is writable.

Preserve the private one-Result-Arc+compact-epoch carrier, one Legacy/Observed
driver and exact root -> policy -> candidate effective -> transform -> BFS/
fixed-point -> select order. Complete epochs merge left-first before semantics.
Every horizon attempts every Complete merge after the first outer while
retaining that first horizon-ordered outer; all other legacy terminal precedence
and suppression remain exact.

Add only production-used live-stage and full-horizon position proof; exact
observed/legacy implicit-bazel_tools, duplicate, diamond, cycle, nodep,
recursive and mixed topology rows/epochs/complete event sequences; and
independent command-policy/effective/nonregistry/registry/recursive/mixed
held-carrier A -> B -> A proof with unaffected Arc preservation and zero upper
activation on success/error/cancel.

Graph siblings remain eventless and discovery batches stay child-owned.
Traversal/frontier/cache/outcome/event/merge structures stay compute-local; add
no retained map/cache/interner/store/lock/task/direct Host read.

STOP production semantic/event/family/memory drift, third-file/caller/export or
upper activation, proof hooks/waiver, cap excess or milestone closure. REPLAN
before widening. After independent retry ACCEPT return only to the docs-only
selected frontier. M7 remains partial and M7A -> M8 -> M7B remains.

### Selected-module-graph observation accepted and frontier resumed (2026-08-20)

Accepted implementation `d5e8f461` from Rust base `c6b1e108`, semantic
design `09177514` and proof correction `19cc508d` completes the private
observed selected-graph aggregate. One shared Legacy/Observed driver preserves
root -> policy -> effective -> transform -> BFS/fixed-point -> select order.
Every horizon validates all Complete carrier epochs while retaining the first
ordered outer; graph siblings remain eventless and discovery children retain
their exact batches.

The DICE value retains one semantic Result Arc plus one compact cumulative
epoch. Traversal maps/sets, horizons, event and merge scratch remain
compute-local. Accepted accounting is +418 production/+471 colocated proof in
`selected_graph.rs`, +1,428 external proof and +2,317 aggregate semantic, at
2,481/9,834/12,315 physical. Focused selected-graph proof passes 27/27; full
bzlmod validation passes 500 unit tests plus every integration/doc target.
Formatting/diff hygiene and independent terminal ownership/retention review
pass.

Activate only
`WP-6-7A-selected-module-graph-observation-frontier-audit-resume-5`. Rust,
tests, fixtures, oracles, exports and callers are read-only. Audit write
authority is canonical/current/this Stage/routing only, at net caps
<=40/<=220/<=180/<=30 and <=470 aggregate.

Trace the accepted selected-graph carrier through
`HostSelectedRegistryRepoSpecsKey`, `HostSelectedModuleRoutesKey`,
registry/repository-spec interpretation, generated-repository extension
ownership and command/bootstrap consumers. Inspect only far enough to select
the uniquely smallest complete remaining mutable frontier or prove a smaller
carrierless prerequisite. Do not reopen accepted lower owners.

Require exact Result/Arc/epoch association; graph -> spec/route/extension order
and complete-batch Need/outer/error precedence; matching families, downstream
event ownership, warm/cancel behavior; independent lifecycle restoration; and
compact retention with all parser/join/frontier/event scratch compute-local.
Preserve admitted Bazel 9 values/errors/order/events as exact, private typed
outers/shared-Arc association as Slug-native, and extension breadth,
bootstrap execution, M8/M7B and exact identity bytes as deferred.

Reach exactly one terminal: one independently reviewed smallest-owner design,
one uniquely smaller evidence/association prerequisite, or formal REPLAN. A
design may name at most one implementation successor. STOP Rust/test/oracle/
caller/export work, speculative public activation, umbrella ownership,
milestone closure, M8/M7B work or bypassing the accepted graph carrier. M7
remains partial and M7A -> M8 -> M7B remains.

### Selected-graph frontier audit: visible-lockfile prerequisite (2026-08-20)

The accepted `d5e8f461` selected-graph owner and frontier packet `98aaf23c`
establish that `HostVisibleLockfileKey` is the uniquely smaller prerequisite
before `HostRegistryFunctionKey` and selected registry repo-spec ownership.
`HostVisibleLockfileKey` owns the exact Host sequence
`HostFileBytesKey -> RootModuleLockfileModeKey -> file semantics/parser`.

The accepted `RootModuleFilesObservationKey` cannot substitute for this owner.
It reads mode first, returns `VisibleLockfileRead::Ignored` with an empty
lockfile epoch in `LockfileMode::Off`, includes root MODULE observations and
projects different errors. The Host-visible owner instead observes and parses
present bytes even in Off mode and returns an `Arc<BazelLockfile>`. Substitution
would change exact dependency order, value/error behavior and epoch membership.

Activate only `WP-6-7A-host-visible-lockfile-observation-design`. Design write
authority is canonical/current/this Stage/routing at net caps
<=40/<=220/<=180/<=30 and <=470 aggregate. Rust, tests, fixtures, oracles,
exports and callers remain read-only pending independent design ACCEPT.

### Visible-lockfile observation design

The sole future Rust authority is
`app/slug_bzlmod_v2/src/host_lockfile.rs`, baseline 965 physical with first
`#[cfg(test)]` at line 142. Permit <=140 production, <=280 proof, <=420
aggregate semantic and <=1,400 physical lines. Helpers/tests remain below 200;
the cohesive owner file is the only allowed exception to file-size pressure.

Add only private crate-visible
`HostVisibleLockfileObservationKey(HostVisibleLockfileKey)` and
`ObservedHostVisibleLockfile`. The latter retains one exact existing
`Arc<Result<HostVisibleLockfileValue, HostVisibleLockfileError>>` plus the
exact `PathObservationEpoch` from its Host FileBytes child. Require
`Dupe`/`Allocative`, borrowed accessors, distinct key identity and no export
or caller.

One Legacy/Observed driver preserves:

1. form the visible lockfile path;
2. Legacy computes only `HostFileBytesKey` with an empty epoch while Observed
   computes only `HostFileBytesObservationKey` and forwards its exact epoch;
3. after a Complete file child, both compute the neutral
   `RootModuleLockfileModeKey`; and
4. both then inspect file semantics and invoke the same Host parser.

File Need or observed typed outer is immediate and carrierless, suppressing mode
and parse. Preserve the legacy DICE invariant behavior. After a Complete file
child, missing mode wins over any stored file semantic error and retains the
file epoch. File error, Missing, Present, `BadLockfile`, `UncaughtParse`,
unsupported-version and success all retain that same exact epoch. No root-files
family, epoch union, reconstruction or direct Host read is allowed.

Complete equality uses the local semantic Result plus epoch. Need is invalid and
self-unequal; typed outer equality is by outer value. Legacy projection moves
the exact Result Arc. The parent, Host FileBytes semantic child and mode child
remain eventless; path observations stay lower-owned. Warm reuse is silent and
poll-drop publishes no parent value or batch.

Retain no child carrier, file bytes/parser scratch, extra collection,
map/cache/interner/store/lock/task, revision/certificate or event state. The
DICE value is exactly one semantic Result Arc plus one compact shared-Arc epoch.

Required proof is:

- key equality/hash/Display, accessors, `Dupe`/`Allocative`, and
  Complete/Need/outer equality and validity;
- production-used family adapters/finishers, exact file-first observed and
  legacy dependency rows, reverse-family isolation, file Need/outer, completed
  file then missing-mode precedence, and later suppression;
- exact File/BadLockfile/UncaughtParse/unsupported-version/success variants and
  messages across Missing, Present, WrongKind, read and resolution terminals;
- exact per-demand/result Arc forwarding and explicit Off+present bytes proving
  Host parsing with a nonempty epoch rather than root-files Ignored/empty;
- exact zero semantic batches, warm silence and real poll-drop/no-publication/
  same-DICE recovery;
- independent mode and lockfile bytes/symlink A -> B -> A with held Result and
  epoch handles; and
- zero RootModuleFiles, HostRegistryFunction, repo-spec, route, extension,
  public-command and bootstrap activation.

Reuse accepted lower Host FileBytes/path proof and add no oracle. Exact
compatibility is the current Host-visible values/errors, file -> mode -> parse
order, parser and eventlessness. The private typed outer/shared-Arc association
is Slug-native. Host registry function, repo specs/routes/extensions,
public/bootstrap activation, M8/M7B and identity bytes remain deferred.

STOP a second Rust file/key, caller/export, root-carrier substitution,
parser/order/error/event drift, extra retained state, upper activation, cap
excess, proof waiver, milestone closure or M8/M7B work. REPLAN before widening.
After independent design ACCEPT schedule only
`WP-6-7A-host-visible-lockfile-observation-implementation`; after its ACCEPT
resume only the docs frontier for Host registry-function observation, then repo
specs. M7 remains partial and M7A -> M8 -> M7B remains.

### Visible-lockfile observation design accepted (2026-08-20)

Accepted design `ba21c0e8` from Rust base `d5e8f461` activates only
`WP-6-7A-host-visible-lockfile-observation-implementation`.

Write exactly `app/slug_bzlmod_v2/src/host_lockfile.rs`, baseline 965 physical
and first `#[cfg(test)]` line 142, within <=140 production, <=280 proof,
<=420 aggregate semantic and <=1,400 physical. Helpers/tests remain below 200;
no second file, caller, export, fixture or oracle is writable.

Add the private one-Result-Arc+exact-Host-FileBytes-epoch carrier and one
Legacy/Observed driver. Preserve file -> mode -> file semantic/Host parser,
matching Host FileBytes families, carrierless Need/typed outer, completed-file
then missing-mode precedence, exact legacy Result-Arc projection and all current
terminal values/errors. Off with present bytes must still observe and parse;
the root-files Ignored/empty carrier is forbidden.

Retain only the local semantic Result Arc plus compact shared epoch. Add no
union/child carrier/scratch collection/cache/interner/store/lock/task/direct
Host read/revision/certificate/event state. Parent and children remain
eventless; warm reuse is silent and poll-drop publishes nothing.

Require the full accepted identity/family/terminal/Arc/Off/event/warm/cancel/
independent mode+bytes+symlink lifecycle and upper-nonactivation proof. Exact
compatibility remains current Host-visible values/errors/order/parser/events;
the private typed outer/shared-Arc epoch is Slug-native; Host registry function,
repo specs/routes/extensions/public/bootstrap, M8/M7B and identity bytes remain
deferred.

STOP semantic/order/event/memory drift, wider authority, root-carrier
substitution, proof waiver, cap excess or milestone closure. REPLAN before
widening. After independent implementation ACCEPT resume only the docs frontier
for Host registry-function observation, then repo specs. M7 remains partial and
M7A -> M8 -> M7B remains.

### Visible-lockfile observation accepted; Host-registry frontier resumed (2026-08-20)

Accepted implementation `2a4041bb` from Rust base `d5e8f461` and semantic
design `ba21c0e8` completes the private Host-visible-lockfile observation
owner. One shared Legacy/Observed driver preserves FileBytes -> mode -> file
semantic/Host-parser order, including present bytes under Off. Missing mode
retains the completed file epoch and wins over stored file errors; Need/outer
remains carrierless.

The DICE value retains one semantic Result Arc plus the exact compact Host
FileBytes epoch. No child carrier, union, parser scratch, collection/cache/
interner/store/lock/task/direct Host read, revision/certificate or event state
is retained. Accepted accounting is +114 production/+280 proof/+394 aggregate
at 1,359 physical lines. Focused proof passes 10/10; the full bzlmod suite passed
501 unit tests plus all integration/doc targets before the final proof-only
tracker correction, and focused validation passes afterward. Formatting,
diff hygiene, cleanup/retention and independent terminal review pass.

Activate only
`WP-6-7A-host-registry-function-observation-frontier-audit`. Audit write
authority is canonical/current/this Stage/routing only, at net caps
<=40/<=220/<=180/<=30 and <=470 aggregate. Rust, tests, fixtures, oracles,
exports and callers are read-only.

Trace accepted selected-graph and Host-visible-lockfile carriers through
`HostRegistryFunctionKey`, selected registry repo specs/routes,
extension-generated repositories and public/bootstrap consumers only far enough
to identify the uniquely smallest complete remaining mutable frontier. Do not
presume Host-registry ownership or reopen accepted lower carriers.

Require exact Result-Arc/epoch association; graph -> visible lockfile ->
registry-function -> repo-spec/route/extension order and Need/outer/error
precedence; matching families and downstream event ownership; warm/cancel
behavior; independent lifecycle restoration; and compact retention with all
parse/join/event scratch compute-local.

Preserve admitted Bazel 9 values/errors/order/events as exact, private typed
outers/shared-Arc association as Slug-native, and repo-spec/route/extension
breadth, bootstrap execution, M8/M7B and exact identity bytes as deferred.
Reach exactly one terminal: one independently reviewed smallest-owner design,
one uniquely smaller evidence/association prerequisite, or formal REPLAN. A
design may name at most one implementation successor.

STOP Rust/test/oracle/caller/export work, speculative public activation,
umbrella ownership, milestone closure, M8/M7B work or bypassing the accepted
graph/visible-lockfile carriers. M7 remains partial and M7A -> M8 -> M7B
remains.

### Host-registry-function observation design (2026-08-20)

The accepted `2a4041bb` visible-lockfile owner closes the last mutable
prerequisite below `HostRegistryFunctionKey`. This key alone owns exact mode
-> vendor projection -> conditional refresh -> visible lockfile -> resolved
spelling -> module mirrors -> primary URI/hash policy -> mirror validation ->
Result order. Selected repo specs add graph/registry-file/effective/spec work
and therefore remain later.

Design authority is canonical/current/this Stage/routing only, at net caps
<=40/<=220/<=180/<=30 and <=470 aggregate. Rust/tests/fixtures/oracles/exports/
callers remain read-only pending independent design ACCEPT.

Future Rust authority is only `app/slug_bzlmod_v2/src/host_registry.rs`,
baseline 1,536 physical/test boundary 529, within <=220 production, <=520 proof,
<=740 aggregate semantic and <=2,300 physical. Helpers/tests remain below 200;
no second file, export or caller is writable.

Add a private crate-visible observed wrapper key/carrier retaining one exact
existing semantic Result Arc plus the exact visible-lockfile epoch. Derive
`Dupe`/`Allocative`, add borrowed accessors and no public caller.

One Legacy/Observed driver preserves:

1. neutral lockfile mode;
2. neutral vendor projection;
3. conditional neutral refresh token;
4. matching legacy-empty versus observed-exact visible-lockfile family;
5. resolved registry spelling;
6. neutral module mirrors; and
7. primary URI parsing, scheme/hash selection, ordered mirror validation and
   current Result construction.

Preserve the duplicated neutral mode dependency inside the visible child and
legacy DICE-invariant behavior. Do not reorder the parent. Mode/vendor/refresh
failures complete with an empty epoch. Visible Need/typed outer is immediate
and carrierless. Visible semantic failure and every later mirrors/URI/success
terminal retain the exact visible epoch. No union/reconstruction is needed.
Legacy projection moves the exact Result Arc.

Complete equality is semantic Result+epoch; Need remains invalid/self-unequal
and typed outer compares by value. Parent/children remain eventless, warm reuse
is silent and cancellation publishes no parent value/batch.

Retain only the local Result Arc plus compact epoch. Child carriers, URI/parser
and mirror-selection scratch remain compute-local; add no collection/cache/
interner/store/lock/task/direct Host read/revision/certificate/event state.

Proof requires identity/hash/Display/accessors/equality/validity; production-used
adapters/finishers for every terminal; exact Refresh/non-Refresh dependency rows
and reverse family isolation; carrierless Need/outer, exact epoch shared Arcs and
legacy Result-Arc projection; exact current errors, schemes/hash policies and
primary/mirror URI terminals; exact zero batches,
warm and poll-drop recovery; independent same-key mode/vendor/refresh/visible/
mirrors A -> B -> A with held carriers; workspace/original-registry key-identity
reuse; and zero repo-spec/route/extension/public/bootstrap activation.

Exact compatibility is current Host registry values/errors/order/URI/hash/
mirror behavior and eventlessness. The private typed outer/shared-Arc
association is Slug-native. Repo specs/routes/extensions/public/bootstrap,
M8/M7B and exact identity bytes remain deferred.

STOP a second file/key, caller/export, ordering/error/event drift, retained
child state, upper activation, proof waiver, cap excess or milestone closure.
REPLAN before widening. After independent design ACCEPT schedule only
`WP-6-7A-host-registry-function-observation-implementation`; after its ACCEPT
resume only the docs frontier for selected registry repo specs. M7 remains
partial and M7A -> M8 -> M7B remains.

### Host-registry-function observation design accepted (2026-08-20)

Accepted design `38f40427` from Rust base `2a4041bb` activates only
`WP-6-7A-host-registry-function-observation-implementation`.

Write exactly `app/slug_bzlmod_v2/src/host_registry.rs`, baseline 1,536
physical/test boundary 529, within <=220 production, <=520 proof, <=740
aggregate semantic and <=2,300 physical. Helpers/tests remain below 200; no
second file, export or caller is writable.

Add the private one-Result-Arc+exact-visible-epoch carrier and one
Legacy/Observed driver. Preserve mode -> vendor -> conditional refresh ->
matching visible child -> spelling -> mirrors -> URI/hash -> ordered mirror
validation. Preserve the duplicate neutral mode relationship and legacy
DICE-invariant behavior.

Pre-visible mode/vendor/refresh failures retain an empty epoch. Visible
Need/typed outer is carrierless; visible semantic and all later terminals retain
the exact visible epoch. No union/reconstruction is allowed. Legacy projection
moves the exact Result Arc.

Retain only the local Result Arc+compact epoch. Parent/children remain eventless;
all child/parser/mirror scratch is compute-local with no added collection/cache/
interner/store/lock/task/direct Host read/revision/certificate/event state.

Require the full accepted identity/terminal/prefix/family/Arc/value/error/
scheme/hash/URI/event/warm/cancel/input-lifecycle/key-identity and upper-
nonactivation proof.

STOP semantic/order/event/memory drift, wider authority, caller/export, proof
waiver, cap excess or milestone closure. REPLAN before widening. After
implementation ACCEPT resume only the docs frontier for selected registry repo
specs. M7 remains partial and M7A -> M8 -> M7B remains.

### Host-registry-function observation accepted; selected repo-spec frontier resumed (2026-08-20)

Accepted implementation `e155d74f` from Rust base `2a4041bb` and semantic
design `38f40427` completes the private Host-registry-function observation
owner. One Legacy/Observed driver preserves mode -> vendor -> conditional
refresh -> matching visible lockfile -> spelling -> mirrors -> URI/hash ->
ordered mirror validation. Visible Need/typed outer is carrierless; every
completed visible/later terminal retains the exact visible epoch and legacy
projection moves the exact Result Arc.

The DICE value retains one local semantic Result Arc plus the compact exact
visible-lockfile epoch. No child carrier, parser/URI/mirror scratch,
collection/cache/interner/store/lock/task/direct Host read, revision,
certificate or event state is retained. Parent and children remain eventless.

Accepted accounting is +149 production/+460 proof/+609 aggregate at 2,145
physical lines. Focused proof passes 13/13 and the full bzlmod suite passes 503
unit tests plus every integration/doc target. Formatting, diff hygiene,
cleanup/retention, security and independent terminal review pass.

Activate only
`WP-6-7A-selected-registry-repo-specs-observation-frontier-audit`. Audit write
authority is canonical/current/this Stage/routing only, at net caps
<=40/<=220/<=180/<=30 and <=470 aggregate. Rust, tests, fixtures, oracles,
exports and callers are read-only.

Trace accepted selected-graph, registry-file, effective/discovery/preparation
and Host-registry carriers through `HostSelectedRegistryRepoSpecsKey`, then
routes, extensions and public/bootstrap consumers only far enough to identify
the uniquely smallest complete remaining mutable frontier. Do not presume the
aggregate owner or reopen accepted lower carriers.

Require exact Result-Arc/epoch association; graph -> registry-file/effective/
Host-registry -> entry -> aggregate order and full-batch terminal precedence;
matching families and child event ownership; warm/cancel behavior; independent
lifecycle restoration; and compact retention with join/frontier/event scratch
compute-local.

Preserve admitted Bazel 9 values/errors/order/events as exact, private typed
outers/shared-Arc association as Slug-native, and route/extension/public/
bootstrap breadth, M8/M7B and exact identity bytes as deferred. Reach exactly
one terminal: one independently reviewed smallest-owner design, one uniquely
smaller evidence/association prerequisite, or formal REPLAN. A design may name
at most one implementation successor.

STOP Rust/test/oracle/caller/export work, speculative public activation,
umbrella ownership, milestone closure, M8/M7B work or bypassing the accepted
selected-graph/registry/Host-registry carriers. M7 remains partial and
M7A -> M8 -> M7B remains.

### Selected registry repo-spec observation owner design (2026-08-20)

The accepted frontier audit selects only
`WP-6-7A-selected-registry-repo-specs-observation-design`.
Scheduling base is `041b4476` and Rust base is the accepted
`e155d74f`.
`HostSelectedRegistryRepoSpecsKey` is the smallest complete aggregate above
the accepted selected-graph, Host-registry, registry-file and effective
carriers. Its owner-local entry computation has no independent consumer; the
sole consumer, selected routes, adds separate route semantics.

Design authority is canonical/current/this Stage/routing only, at net caps
<=40/<=220/<=180/<=30 and <=470 aggregate. Rust/tests/fixtures/oracles/exports/
callers remain read-only. Future Rust authority is only
`app/slug_bzlmod_v2/src/selected_repo_spec.rs`, baseline 6,830 physical/test
boundary 2,974, within <=520 production, <=1,100 proof, <=1,620 aggregate
semantic and <=8,500 physical. Helpers/tests remain below 200; no second file,
key, export or caller is writable.

Add one private crate-visible observation key/carrier retaining one exact local
semantic Result Arc plus one cumulative compact epoch. Derive
`Dupe`/`Allocative`, provide borrowed accessors and use one Legacy/Observed
semantic driver with matching graph, Host-registry, registry-file and effective
families. Legacy contributes empty epochs and moves the exact Result Arc.

Preserve graph first, then graph-occurrence order. Root/nonregistry entries are
owner-local `None`; registry entries preserve Host registry -> source JSON
registry file -> parse/projection -> optional registry JSON file -> module
projection -> effective override -> augmentation. Per-entry terminals suppress
later children, while the aggregate continues its existing full cross-entry
scan.

Merge every Complete epoch immediately, left-first and before semantics, in
graph/entry/child order. Equal duplicates retain the earliest exact Arc.
Conflicts and operation mismatch are typed outer. A private stage-aware outer
distinguishes graph, Host registry/module, registry file/module/URL,
effective/module and merge/module/stage. Continue attempting later Complete
merges after the first outer but retain that first outer.

Final precedence is first typed child/merge outer, then first semantic or
DICE-compute error, then first incompatible Need, compatible Need union and
ordered success. Need/outer is carrierless; Complete error/success retains the
full valid cumulative prefix. Graph Need/outer suppresses entries and graph
semantic failure retains only its complete graph prefix.

The aggregate remains eventless. Selected-graph/discovery descendants keep
their exact batches; other children remain eventless and lower ownership is
unchanged. Warm reuse is silent and poll-drop publishes no parent state.
Retain only one Result Arc+epoch; graph/child carriers, entry epochs, traversal,
merge, terminal and event scratch remain compute-local with no added map/cache/
interner/store/lock/task/direct Host read/revision/certificate state.

Proof covers identity/accessors/equality/legacy Arc, every graph and per-entry
terminal/prefix/later suppression, duplicate-first/conflict/mismatch, full-scan
first/middle/last semantic/Need/outer cases, root/nonregistry and optional-file
semantics, exact family rows and complete child batch parity, warm/cancel
recovery, independent graph/Host-registry/source JSON/registry JSON/effective
A -> B -> A held-carrier lifecycles, and zero route/extension/generated/public/
bootstrap activation.

Exact compatibility is current admitted repo-spec values/errors/order/events.
The private typed outer/shared-Arc epoch is Slug-native. Routes/extensions/
generated/public/bootstrap work, M8/M7B and exact identity bytes remain
deferred.

STOP a second file/key, caller/export, changed precedence/event ownership,
retained traversal state, upper activation, proof waiver, cap excess or
milestone closure. REPLAN before widening. After independent design ACCEPT
schedule only
`WP-6-7A-selected-registry-repo-specs-observation-implementation`; after its
ACCEPT resume only the route/extension frontier. M7 remains partial and
M7A -> M8 -> M7B remains.

### Selected registry repo-spec observation design accepted (2026-08-20)

Accepted design `0444dd40` from Rust base `e155d74f` activates only
`WP-6-7A-selected-registry-repo-specs-observation-implementation`.

Write exactly `app/slug_bzlmod_v2/src/selected_repo_spec.rs`, baseline 6,830
physical/test boundary 2,974, within <=520 production, <=1,100 proof, <=1,620
aggregate semantic and <=8,500 physical. Helpers/tests remain below 200; no
second file/key, export or caller is writable.

Add the private one-Result-Arc+cumulative-epoch carrier and one
Legacy/Observed semantic driver. Preserve graph first, graph-occurrence order,
owner-local root/nonregistry `None`, and each registry entry's Host registry ->
source JSON file -> parse/projection -> optional registry JSON file -> module
projection -> effective override -> augmentation order.

Per-entry terminals suppress later children; the aggregate still scans all
entries. Merge every Complete epoch immediately, left-first and before
semantics. Preserve earliest exact Arcs, retain the first stage-aware child or
merge outer while attempting later Complete merges, and keep final precedence
outer -> first semantic/DICE error -> incompatible Need -> compatible Need
union -> ordered success. Need/outer is carrierless; Complete terminals retain
the full valid cumulative prefix.

Retain only the local Result Arc+compact epoch. Parent is eventless, existing
child batches/ownership remain exact, all traversal/merge/terminal/event scratch
is compute-local, and no collection/cache/interner/store/lock/task/direct Host
read/revision/certificate state is added.

Require the full accepted identity/terminal/prefix/full-scan/family/Arc/event/
warm/cancel/held-lifecycle and upper-nonactivation proof. STOP semantic,
precedence, event or memory drift, wider authority, caller/export, proof waiver,
cap excess or milestone closure. REPLAN before widening. After implementation
ACCEPT resume only the docs route/extension frontier. M7 remains partial and
M7A -> M8 -> M7B remains.

### Selected registry repo-spec observation accepted; route/extension frontier resumed (2026-08-20)

Accepted implementation `ccf7421e` from Rust base `e155d74f` and semantic
design `0444dd40` completes the private selected registry repo-spec observation
owner. One Legacy/Observed driver preserves selected graph first,
graph-occurrence order, root/nonregistry `None`, and registry Host-registry ->
source JSON -> optional registry metadata -> effective override -> augmentation
order. It preserves full cross-entry scanning and outer -> semantic error ->
incompatible Need -> compatible Need union -> ordered success precedence.

The DICE value retains one local semantic Result Arc plus one cumulative compact
epoch. Traversal, entry, merge, terminal and event scratch remains compute-local
with no child carrier, cache/interner/store/lock/task/direct Host read, revision
or certificate state. The aggregate is eventless and accepted child batch
ownership/order remains exact.

Accepted accounting is +492 production/+1,100 proof/+1,592 aggregate at 8,422
physical lines. Focused proof passes 42 unit and 3 integration tests; the full
bzlmod suite passes 509 unit tests plus every integration/doc target. Formatting,
diff hygiene, cleanup/retention, security and independent terminal review pass.

Activate only `WP-6-7A-route-extension-observation-frontier-audit`. Audit write
authority is canonical/current/this Stage/routing only, at net caps
<=40/<=220/<=180/<=30 and <=470 aggregate. Rust, tests, fixtures, oracles,
exports and callers are read-only.

Trace the accepted repo-spec carrier through `HostSelectedModuleRoutesKey`,
`HostSelectedExtensionMappingsKey`, extension definition/load/evaluation
owners, generated-repository definitions and public/bootstrap consumers only
far enough to identify the uniquely smallest complete remaining mutable
frontier. Do not presume routes or extension mappings are complete and do not
reopen accepted lower owners.

Require exact repo-spec -> route -> extension -> generated-repository order,
terminal precedence and later suppression; reusable versus request-local
ownership; matching Legacy/Observed families; exact child batches and eventless
parent behavior where applicable; warm/cancel recovery; independent route,
extension and generated-repository held-carrier lifecycles; and compact
Buck2-shaped retention with mapping/frontier/event/parser scratch compute-local.

Preserve admitted Bazel 9 values/errors/order/events as exact, private typed
outers/shared-Arc association as Slug-native, and extension-generated/public/
bootstrap breadth, M8/M7B and exact identity bytes as deferred unless live
evidence proves one is the uniquely smaller prerequisite.

Reach exactly one terminal: one independently reviewed smallest-owner design,
one uniquely smaller evidence/association prerequisite, or formal REPLAN. A
design may name at most one implementation successor. STOP Rust/test/oracle/
caller/export work, speculative public activation, umbrella ownership,
milestone closure, M8/M7B work or bypassing the accepted selected-graph,
registry and repo-spec carriers. M7 remains partial and M7A -> M8 -> M7B
remains.

### Selected-module-routes observation owner design (2026-08-20)

The accepted frontier audit selects only
`WP-6-7A-host-selected-module-routes-observation-design`. Scheduling base is
`cbd8e285` and Rust base is accepted repo-spec implementation `ccf7421e`.

`HostSelectedModuleRoutesKey` is the smallest complete owner above the accepted
selected-graph and repo-spec carriers. It alone owns graph -> repo specs ->
canonical identity/repository mapping/route projection. Repo specs do not retain
the graph, so both children remain necessary. Routes are independently reused by
canonical selected definitions and extension mappings; mappings add root usages,
overrides and extension semantics and cannot absorb route observation.

Design authority is canonical/current/this Stage/routing only, at net caps
<=40/<=220/<=180/<=30 and <=470 aggregate. Rust/tests/fixtures/oracles/exports/
callers remain read-only. Future Rust authority is only
`app/slug_bzlmod_v2/src/selected_repo_spec.rs`, baseline 8,422 physical/test
boundary 3,466, within <=300 production, <=600 proof, <=900 aggregate semantic
and <=9,400 physical. Helpers/tests remain below 200; the selected repository
pipeline is one cohesive large-file exception.

Add one private matching observation key/carrier retaining one exact local route
Result Arc plus one cumulative compact epoch. Derive `Dupe`/`Allocative`, provide
borrowed accessors and use one Legacy/Observed driver. Legacy selects legacy graph
then repo specs with empty epochs; observed selects their accepted matching
siblings. Both share unchanged pure canonical/mapping/registry association and
route projection. Legacy moves the exact Result Arc.

Merge every Complete epoch immediately left-first before semantics: direct graph
first, then repo specs. Equal graph demands inside repo specs retain the direct
graph Arc. Conflict/operation mismatch is typed outer; distinguish Graph child,
RepoSpecs child and Graph/RepoSpecs merge stage.

Graph Need/outer is carrierless and suppresses repo specs. Graph compute error has
empty prefix; graph semantic error retains graph prefix. Repo-spec Need/outer is
carrierless. Repo-spec compute error retains graph-only prefix; repo-spec semantic
error retains the full merged prefix. Pure canonical collision, mapping invalid,
registry mismatch and success retain the full prefix. There is no Need union or
batch fold.

The parent is eventless. Graph/discovery keep exact batch ownership/order and
repo specs remain eventless. Warm is silent and cancellation publishes no
accepted parent state. Retain only the route Result Arc+epoch; canonical/mapping
SmallMaps, traversal, child carrier, merge, terminal and event state remain
compute-local with no added collection/cache/interner/store/lock/task/direct Host
read/revision/certificate/event state.

Proof covers key/accessors/equality/legacy Arc; production finishers/projectors;
every graph/repo-spec Need/outer/compute/semantic prefix and suppression;
duplicate-first Arc/conflict/mismatch; exact root/nonregistry/registry values and
order; canonical collision, mapping invalidity and registry mismatch; exact
Legacy/Observed rows and complete child event parity; warm/cancel recovery;
independent graph/repo-spec/pure-route held-carrier lifecycles; and zero canonical
definition/extension/generated/public/bootstrap activation. Unreachable classes
use production projectors plus accepted lower proof without hooks or inconsistent
child injection.

Exact compatibility is current route values/errors/order/events. The private
typed outer/shared-Arc epoch is Slug-native. Extension/generated/public/bootstrap,
M8/M7B and identity bytes remain deferred.

STOP a second key/file, caller/export, route/event drift, retained traversal
state, upper activation, proof waiver, cap excess or milestone closure. REPLAN
before widening. After independent design ACCEPT schedule only
`WP-6-7A-host-selected-module-routes-observation-implementation`; after its
ACCEPT resume only the extension frontier. M7 remains partial and
M7A -> M8 -> M7B remains.
