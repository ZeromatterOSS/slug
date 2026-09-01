# Current Slug V2 Packet

Packet: `WP-6-7A-default-exec-configured-label-dependency-implementation-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 configured dependency,
execution-platform, and retained edge identity.

Status: implementation active from independently accepted architecture R2.
Independent R1 review returned `REVISE`: a singular Starlark output
discriminator could not represent already-admitted zero/multi-output
transitions, and the production allowlist contained an open-ended
compiler-required escape. R2 retains the complete immutable canonical output
slice and names every production file; focused rereview returned `ACCEPT` with
only the already-deferred named/composed-group residual.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Observable result and compatibility classification

Freeze one generic architecture for Bazel 9.2 default execution-configured
ordinary dependencies. The implementation successor must load and analyze
`cfg = "exec"` across every label-bearing Starlark attribute constructor,
select the dependency value in the final owning-rule target configuration,
then configure each selected dependency for the rule's already-selected
default execution platform. The rules_rust `proc_macro_deps` failure is one
authentic discriminator, never an implementation branch.

Exact behavior within the admitted surface:

- `attr.label`, `attr.label_list`, `attr.string_keyed_label_dict`,
  `attr.label_keyed_string_dict`, and `attr.label_list_dict` all accept the
  named `cfg` parameter. Omitted `cfg`, explicit `None`, and `"target"` select
  the target configuration; `"exec"` selects the default execution group; an
  existing regular Starlark transition retains its accepted behavior. Other
  strings and value kinds fail during declaration conversion;
- `executable = True` remains scalar-label-only and requires an explicitly
  non-`None` `cfg`. Both explicit `"target"` and `"exec"` are valid. The final
  selected/configured dependency must retain an executable `DefaultInfo`, and
  `ctx.executable` uses the existing typed FilesToRun projection;
- all selectors and label/dictionary values resolve under the owning rule's
  final target configuration after any accepted rule transition. The exec
  transition is applied only to the resulting dependency labels. Dictionary
  orientation, flattened label occurrence order, duplicates, and attribute
  indexes remain unchanged;
- the default exec transition uses the default toolchain context's selected
  execution platform. Ordinary exec dependencies neither add toolchain
  requirements nor participate as candidate inputs to execution-platform
  selection. Owners with zero toolchain requirements still use the selected
  default execution platform already produced from registrations and Host
  fallback;
- every ordinary exec dependency uses the existing structural
  `to_exec_for_platform` projection: target-scoped settings are removed,
  universal/project settings survive, Host compilation/action environments
  are applied, and the selected execution platform is final. Multiple exec
  attributes selecting the same label converge on the same configured child
  key;
- source files remain null-configured nodes, but their incoming attribute edge
  still retains attribute name/index and exec-tool role. Generated files retain
  the exec configuration on their generating-target side. Existing
  allow-files, allow-single-file, provider-DNF and executable validation runs
  against the final dependency result for all five shapes;
- exec attributes are tool dependencies independently of visibility or child
  node kind. The retained edge therefore supports exact target/tool filtering
  and constraint-policy decisions later without reconstructing role from a
  child configuration. This packet does not widen current cquery tool-edge
  traversal, which continues to fail closed; and
- same-DICE changes to selected execution platform, target-scoped/universal
  settings, resolved selector values, attribute transition, dependency
  declaration, file/provider/executable result or source/rule kind invalidate
  through the existing configuration, package, toolchain and child-analysis
  dependencies. Exact restoration cuts off at structural equality.

Slug-native behavior:

- dependency configuration, edge, execution-group and configured-target
  equality use complete Rust structural identity. Display tokens, Bazel
  configuration checksums/output roots, Bazel ActionKeys and REAPI digests
  remain distinct domains; and
- Host fallback may canonicalize with an explicit Host platform where the
  existing structural configuration does. No Java object identity, transition
  cache identity, or SkyKey byte layout is reproduced.

Unsupported/deferred behavior:

- legacy `cfg = "host"`, `config.exec(exec_group = ...)`, named and automatic
  exec groups, `rule(exec_groups = ...)`, `exec_group()`, composed exec plus
  Starlark transitions, and exec-group-specific toolchain contexts remain
  fail-closed. The retained identity is shaped so these categories extend the
  same owner rather than replacing it;
- configured aspect selection/execution, aspect tool edges, materializers,
  dormant dependencies, `skip_validations`, `for_dependency_resolution`,
  legacy environment constraints and alias-to-file actual identity remain
  their existing deferred categories;
- cquery traversal through tool dependencies, `--notool_deps` output parity,
  exact Bazel configuration/output bytes and all new rule/action families
  remain outside this packet; and
- no parser grammar, `set`, builtin registry, rule-family, rules_rust,
  `cc_common`, `cc_internal`, C++ or Rust semantic special case is added. Bazel
  9 BCR Starlark continues to own every rule body.

## Bazel 9.2 authority and evidence

Bazel tag `9.2.0` commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
Pinned source SHA-256 values are:

- `StarlarkAttrModuleApi.java`:
  `af70c851882fa049034184dbb6f6580731cfa738d79dfb8abcf61af176257670`;
- `StarlarkAttrModule.java`:
  `388421c44c623c1c6625fd9f2b059d2a7d1e13b8d45e7c96173f24866a917967`;
- `ExecutionTransitionFactory.java`:
  `b1804d08620dac05873ce856e8bad76aeb8b5db44b374bf90a9419bae0df14d5`;
- `DependencyResolutionHelpers.java`:
  `9736c25c376501594d9a123132ae25e3d8fe842cde3e33cb50eb39a62897aa97`;
- `DependencyProducer.java`:
  `832d63692d0696aa9487e3a4cd2c4d82b2b7af64a1034f8051747cacf07a39f8`;
- `Attribute.java`:
  `fbe208c37ad4ed88030f874fa6cd8bd5cf2f4aac63f9a01a4ff24ca499c9a6a4`;
- `DependencyFilter.java`:
  `db4bc1c9c9e300c2d53851d9438bbee80c6e0c568ffbbb8b8452ecbb45046b68`;
  and
- `RuleContextConstraintSemantics.java`:
  `8ae242015971a1e9434a54214a520ef876a3d73712dd2131571c401253fc4090`.

The API and `convertCfg` prove the complete five-constructor binding family and
target/None/exec/Starlark conversion. `ExecutionTransitionFactory` proves
default-group identity, tool classification, selected-platform finalization,
target-option removal and Host-option application. `DependencyResolutionHelpers`
and `DependencyProducer` prove that configured attribute values precede the
transition and the matching toolchain context supplies its platform.
`Attribute.isToolDependency`, `DependencyFilter.ONLY_TARGET_DEPS`, and
`RuleContextConstraintSemantics` prove that tool role is an attribute fact, not
an inference from the child configuration.

Pinned upstream regressions are `ConfigurationsForTargetsTest.execDeps`,
`ExecutionTransitionFactoryTest.executionTransition` and `noExecPlatform`,
`StarlarkExecGroupTest.testExecGroupTransition`,
`ConstraintsTest.execDependenciesAreNotChecked_customRule`,
`GraphOutputFormatterCallbackTest.nullAndToolDeps`, and
`BuildConfigurationValueTest.starlarkFlagExecScopes`. Their pinned SHA-256
values are respectively
`caa534007cfbfe907d2c026f26ef55cf5bc9fdf1ff686977d99239de735199b6`,
`ac856601feb0069880fe918546b4b3942ba4960c6d03a90df5fd052bb49cb39c`,
`5f99076670edcd8570d124aeb03430a7b2f8f41f48b41b31e6d19a90b6391fe2`,
`2d812ceb45e18c7555930d3d26eb742432e0d2076ac8a8a847e0c76b47d21722`,
`0c4ece8e43dae0c451e80e5959c6b1fa198032c631d57ec1c3de21f22c2ebc0f`,
and `0cf0e828e038f3aa6927b180c5204ba55aec0811ce50375cdd030abc5f4fc73b`.
The named-exec-group test is representation guidance and a negative boundary,
not an acceptance claim for named groups.

Add one permanent focused fixture,
`tests/v2_oracle/fixtures/exec-configured-label-attributes`, with Stage 1
`fixture.toml` provenance. Bazel build/analysis output is compared by
message-shape or structured semantics; cquery configuration/tool filtering is
exact normalized text. The fixture must discriminate all five shapes, target
select-before-exec transition, target-scoped flag removal, selected execution
platform, scalar executable materialization, direct source-file null
configuration/tool role, label order and dictionary orientation. Do not copy a
ruleset or registry subtree. Existing Rust loading/analysis fixtures carry
same-DICE restoration and detailed internal identity proofs.

## Learned Slug facts and architecture decision

Loading already owns
`AttributeDependencyConfiguration::{Target, Exec, Starlark}` on one final
attribute schema, but only scalar/list constructors bind `cfg`; conversion
recognizes `"exec"` and otherwise demands a transition, incorrectly rejecting
explicit `None` and `"target"`. Bind `cfg` on the three dictionary siblings and
centralize conversion in the existing attribute-definition path. Keep the sole
loading enum: current `Exec` means the default group, and the future
`config.exec()` category may extend it with the shared group identity. Add no
second transition registry, parser feature or per-constructor policy.

Analysis currently configures ordinary label dependencies before toolchain
resolution, stamps them `exec_configuration = false`, and rejects every
visible exec or executable schema not lifted into a late-bound configured row.
Lift ordinary target/exec/Starlark dependency preparation into one bounded
phase helper. Resolve labels first. Target and Starlark rows may prepare before
toolchain resolution; any default-exec row defers the combined preparation,
derives one configuration from `resolution.execution_platform()`, and prepares
all rows once. Exec dependencies never mutate the toolchain-resolution key.
Remove the broad visible exec/executable gate; the existing final
file/provider/executable validator remains the sole admission gate.

Replace action-specific `ConfiguredActionExecGroup` with one public
`ConfiguredExecGroup::{Default, Named(CompactString)}` in a small dedicated
analysis module. The existing action-context field and exact FileWrite
identity consume the renamed owner without semantic or byte changes. No alias
or migration shim is kept.

Replace `OrdinaryAttribute`, `TransitionedAttribute`, `ImplicitAttribute`, and
`Source` edge variants with one public retained form:

```text
Attribute {
    attribute: CompactString,
    index: u32,
    hidden: bool,
    dependency: ConfiguredAttributeDependency,
}

ConfiguredAttributeDependency =
    Target
  | Exec(ConfiguredExecGroup)
  | Starlark {
        outputs: Arc<[CanonicalLabel]>,
        exec_group: Option<ConfiguredExecGroup>,
    }
```

Only `Target`, `Exec(Default)`, and `Starlark { exec_group: None }` are
constructed by this implementation. Starlark retains the transition
definition's complete canonical output slice, including zero and multiple
outputs, rather than the current lossy `outputs().first()` projection. The
optional group reserves the source-proven composed-transition fact proven by
Bazel without claiming its execution. `implicit()` reads `hidden` for
attribute edges and preserves the existing nonattribute implicit set. `tool()`
reads `Exec` or a Starlark role with an exec group. Thus source/null nodes
retain their declaration and tool role, and default-to-named exec growth
changes values rather than the public edge shape. `DeclaredDependencyKey`
carries this typed phase-scratch role; remove the parallel
`transition_output`/`exec_configuration` pair. Prove empty, singleton and
multiple Starlark output slices, output-order/equality discrimination, and
unchanged configured child convergence.

The new exec-group enum and edge facts are DICE-retained semantic memory; the
dependency scratch row is phase memory and dies after final analysis. Continue
using `CompactString`, canonical-label sharing, cheap clones and
`Allocative`. Add no map, set, interner, cache, global registry, lock, task,
DICE key, duplicate configuration, evaluator borrow or retained selected-
platform pointer. Measure `size_of` for the old/new edge discriminants and
require no unexplained retained growth; semantic identity takes precedence
over preserving an already-incomplete size. Existing DICE key publication,
equality cutoff, cancellation, eviction and shutdown own the final result.
No lock crosses a DICE computation.

Buck2 commit `088c75c7e36805df99c3de29062baa95db700b8b` is concept/utility-only:
its separate exec-dependency traversal and
`ExecutionPlatformResolution::cfg_for_exec_dep` support typed phase ownership,
while its selection semantics are not imported. Reuse existing Buck2-derived
`CompactString`, `Arc`, `Dupe`, compact maps/sets and `Allocative`; import no
code and add no utility. Zabel commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer concept/test-only:
`exec_group_resolution.zig`, `session_configured_exec_group_relation.zig`, and
`session_configured_owner_value.zig` support typed default/named group identity
and relation-owned selected configuration. Copy no Zig code, IDs, stores,
selection inputs, scheduler, cache, representation, limits or behavior.

## Bounded implementation successor

Allowed production files:

- `app/slug_loading_v2/src/package.rs`;
- `app/slug_analysis_v2/src/{exec_group,configured_target,subrule,dice,result,lib}.rs`;
- `app/slug_core_v2/src/runtime/file_write_identity.rs`.

No other production file is allowed. Compile preflight finds no production
consumer beyond those named paths; an additional production consumer or any
query traversal/formatter behavior change is `REPLAN`.

Allowed proof files are focused loading/analysis/core/query/reapi tests, the
new oracle fixture and its manifest/expected output, and the Stage 6/Stage
9/status documents at acceptance. Cap production at 520 gross Rust lines,
proof at 760,
and total at 1,280. Renames and exhaustive match updates count toward gross
but not net. `package.rs` and `dice.rs` exceed 2,000 lines: keep the former to
three sibling bindings plus the shared converter, and extract a cohesive
dependency-preparation helper from the latter. Do not add more policy inline to
`finish_analysis`. If the shared edge replacement requires unrelated query
semantics, action bytes change, a second configuration owner, new lock/key, or
more than these caps, return `REPLAN`.

Validation after implementation:

- focused loading binding/conversion/schema equality and A/B/A tests;
- complete `slug_loading_v2` and `slug_analysis_v2` suites;
- configured-edge layout/predicate/equality tests, every direct public
  dependent, and FileWrite identity goldens proving the rename changes no
  bytes;
- focused permanent Bazel 9.2 oracle plus changed/protected fixture checks;
- `cargo fmt --all -- --check`, `git diff --check`, source hashes, archive
  status, forbidden-surface/allowlist/cap checks;
- rebuild `slug_cli_v2`, clean `slugd` before and after, and replay the
  authentic rules_rust 0.73 frontier; and
- independent terminal implementation review of the actual diff and recorded
  validation.

Residual risk is named/automatic/composed exec-group behavior. The public edge
can retain that identity, but its loading declaration, toolchain-context
selection, configured aspect interaction and query behavior require their own
complete future category.

## Immediate predecessor

Commit `96ffe8ef3` terminally accepts generic rule-level Starlark transition
execution. The authentic rules_rust 0.73 replay clears that boundary and stops
at `proc_macro_deps` declared with `cfg = "exec"`. Parser/set semantics are
unchanged; Bazel 9 BCR Starlark owns all rules, and `cc_common` remains only a
downstream generic host-ABI consumer.
