# Current Slug V2 Work Packet

Packet: WP-4-6-7A-r2-execution-group-combined-r1
Status: Phase A design freeze accepted; Phase B ready

## Result and owner

Preserve the reconciled selected-toolchain request/configured-action conflict R2
candidate as an explicitly unaccepted base, freeze the complete named and
automatic rule execution-group contract against that base, implement the group
runtime on top of it, and accept the two owners only as one combined checkpoint.
Then replay the unchanged authentic F3 configured-source proof.

R2 commit `f3c90ea46` is local on
`integration/selected-request-output-conflict-r2`, based on current main
`2b3fedf76`. It contains only the reconciled 19-file app candidate plus the
12-second CLI deadline correction and positive producer-to-REAPI proof. It is a
preservation commit, not accepted behavior, and must not reach `main` alone.
The older preserved source is `27e9e9c0c` based on `97dffd5d4`; exclude all
`review-evidence/` content.

The configured-target-owned group collection is the semantic owner. Loaded rule
declarations produce detached requirements, execution constraints and automatic
policy. Configured analysis resolves every group under the owner's target
configuration and retains group identity, selected execution platform,
configured toolchain providers and merged properties. That one immutable
collection supplies named dependency transitions, `ctx.exec_groups`, action
routing and automatic group inference. R2 supplies selected-platform request
identity and root-set action-closure validation before success, RPC or
materialization.

Exact: the pinned Bazel named/automatic declaration, resolution, transition,
provider, property and action-routing behavior enumerated in Stage 6, plus R2's
selected request and FileWrite equality/conflict contract. Slug-native: DICE
keys, structural configured owners, closure ordering and error storage.
Deferred: configured aspects, built-in test-runner groups unless current demand
proves them, unrelated action-family equivalence, computed defaults, complete
C++/Java behavior, exact configuration/output/ActionKey bytes and execution.

## Why the order changed

The first reconciliation pass completed Analysis, Build API, REAPI and compile
consumers, and accounted for every Core library test. Its required real CLI
one-shot, stable-daemon and positive handoff tests enter builtin `bazel_tools`
transitive registration before their command assertions. They cannot complete
before the retained rules_java/rules_cc named/automatic group declarations are
supported. Minimal local-module stubs merely advanced to the first required
autoload and were reverted; they are no acceptance evidence.

Independent correction review returned `REVISE`: landing groups alone on current
main would consume the still-unaccepted R2 selected-platform identity and
validated closure. The safe boundary is therefore R2 plus complete groups on one
branch, with joint gates and atomic integration. Matching resource timeouts are
open gates, not semantic passes.

## Phase A: freeze the implementation contract

Do not edit runtime code until Phase A is complete and independently reviewed.
Against `f3c90ea46`:

1. Reconcile production-closure demand for named and automatic groups under
   `//app/slug_cli_v2:slug`. Preserve the authenticated F3 chain
   rules_java `toolchains/BUILD:138` -> rules_cc `cc_library.bzl:19`, but do not
   claim broader bootstrap membership without the current Bazel/Cargo closure.
2. Trace the live declaration, configured-target, dependency-transition,
   provider, property and action-factory call paths. Decide the cohesive owner
   and any necessary extension of the existing structural toolchain-resolution
   key. Do not activate `toolchains/exec_groups.rs` as a detached registry.
3. Freeze exact Rust/test files, direct consumers, executable proof selectors,
   upstream adaptations, production/proof growth estimates and review caps.
   Give concrete split/cohesion decisions for large `package.rs` and `dice.rs`.
4. Freeze guard replacement: the loading guard remains until the complete
   collection supplies resolution, transitions, providers, properties and
   action routing. Unknown names fail; no default fallback is allowed.
5. Obtain independent terminal design review. Update this manifest with the
   reviewed Phase B allowlist and caps before the first runtime edit.

## Phase A audit and proposed Phase B freeze

The 2026-09-14 closure audit used a detached worktree at current `main`, set
`CARGO_BAZEL_REPIN=1` only there to repair the stale Cargo.Bazel digest, and ran
the bounded Bazel query `deps(//app/slug_cli_v2:slug)`. The query completed in
14.82 seconds after loading 511 packages. Its C++ rule-class slice contains only
`@rules_rust//rust/private/cc:empty`, a `cc_empty_library` whose definition has
no toolchains, execution groups or automatic-policy attribute. The closure has
no `cc_library` instance. The rules_rust rule definitions in the closure retain
default toolchains but contain neither `exec_groups` nor
`_use_auto_exec_groups`, and neither repository configuration nor the native
registry changes the false `incompatible_auto_exec_groups` default. Therefore
compiling the production `//app/slug_cli_v2:slug` binary does not itself demand
named or automatic execution groups.

That result does not remove the runtime requirement. The production CLI's
configured-source consumer enters the authenticated F3/R2 builtin registration
chain rules_java `toolchains/BUILD:138` -> rules_cc 0.2.17
`cc_library.bzl:19`. That target's retained rule definition has public named
`cpp_link`, `_use_auto_exec_groups=True`, a default C++ toolchain and actions
that select group/toolchain contexts. The exact demand claim is consequently
limited to the production CLI runtime and its authentic configured fixture; it
is not a claim about the Bazel graph that compiles the Slug executable. The
temporary repinned worktree and its two changed lockfiles were removed after the
query.

The live path is now traced end to end. `package.rs` owns live/frozen rule
declarations and target publication; `attrs.rs` owns the dependency-transition
tag. `dice.rs` resolves attributes, default toolchains and one selected platform,
then prepares children and calls `evaluate_loaded_rule`. `starlark_rule.rs`
materializes one toolchain view and registers actions; `result.rs` already owns
structural action group/platform/toolchain identity and routes retained action
specs by group. `ConfiguredToolchainResolutionKey` is the correct shared
resolution key, but it must add a sorted, duplicate-free
`Arc<[CanonicalLabel]>` execution-constraint field. Requirements preserve their
declared order; semantically equal constraint sets share one key regardless of
source order. Equal requirement/constraint/configuration inputs may then cut off
at that key while the parent collection retains distinct group identities.
Empty-requirement resolution must filter candidates by those constraints before
selecting a platform.

The sole new configured semantic owner is an immutable
`ConfiguredExecGroupCollection` in a new top-level analysis module
`execution_groups.rs`. Its ordered rows retain `Default`, public `Named`, or
internal `Automatic(CanonicalLabel)` identity, the complete normalized
requirements/constraints, the selected action owner context and selected-
toolchain runfiles closure. It also owns the existing default candidate and
preferred-platform `ToolchainTopology` data. The final `ConfiguredNodeResult`
replaces its separate topology field with this one collection, and
`toolchain_topology()` becomes a borrowed projection of the collection's default
row for unchanged consumers. Normalization and joins are phase scratch. The existing mutable
`toolchains/exec_groups.rs` prototype and its public test/export are deleted;
they are not adapted or activated as a second registry.

Loading retains canonical group constraints and writes public named-transition
identity directly into `AttributeDependencyConfiguration::ExecGroup`. Target
publication retains the transition name without membership validation and moves
the immutable declarations into `StarlarkRuleImplementation` and its structural
equality. Membership is checked later by configured dependency resolution,
matching Bazel loading/query timing; an unknown name fails configured analysis
without Default fallback. The existing invocation guard is removed only in that
same carrier-complete change. Initializer, computed-default, aspect, subrule,
macro, repository-rule and tag-class guards keep their current precedence and
remain closed.

Configured normalization reads `exec_compatible_with`,
`exec_group_compatible_with`, raw `exec_properties`, retained rule declarations,
default toolchain requirements, `_use_auto_exec_groups` when declared, and the
structural native option through one typed configuration getter. An explicit
private rule attribute, true or false, overrides the native option; absence uses
the option. Automatic mode empties default requirements and creates one
internal group for each default toolchain type. Public named groups never share
that identity even when their inputs are equal. Unknown target constraint or
property prefixes fail during configured collection construction, and unknown
named transitions fail during configured dependency resolution.
Platform property prefixes follow Bazel: unknown prefixes are parsed but unused.
Public action group names cannot address automatic groups.

`root_declared_dependency_keys` and `subrule.rs::configured_dependency_rows`
receive the completed collection and preserve `Named(name)` while selecting the
named row's execution configuration for `config.exec(exec_group=...)`. Unknown
names error without default fallback. The evaluator receives the same
collection. With automatic policy off, `ctx.toolchains` reads only the Default
row. With automatic policy on, Default retains zero resolution requirements but
`ctx.toolchains` still exposes exactly the rule's default-declared toolchain
labels, resolving each provider from its corresponding Automatic row and using
the pinned requested-label alias fallback: both the declared alias and resolved
real toolchain type address the provider. Action `toolchain=` deliberately does
not use that alias search and routes only by the Automatic identity created from
the declared requirement. Named rows never enter the `ctx.toolchains` view.
Every nondefault row forms a thin `ctx.exec_groups[name].toolchains` view, so
Automatic label-named rows are indexable there even though an explicit action
`exec_group=` must pass public-identifier validation and cannot name them.
Constraint-only Named groups expose a present empty toolchain map. The views
retain the existing analysis token and do not own a second map.

The action binder replaces erased `Option<Value>` toolchain state with an
explicit omitted/None/label-or-string enum. The synchronous sink validates a
public explicit group first; automatic policy plus an explicit toolchain selects
that toolchain's internal group; supplying both requires membership agreement
only when automatic policy is enabled. With automatic policy off, `toolchain=`
does not reroute an explicitly named action.
The existing executable/tool provenance path supplies the Bazel discriminator:
under automatic mode, with no explicit group and multiple contexts, an
unassociated File/tool requires an explicit toolchain or explicit None, while
recognized dependency runfiles and string executables remain valid. Nested tool
depsets perform this check before element conversion. Only after validation does
the sink attach the selected internal group spelling to `ActionSpec`;
`with_action_specs` resolves that spelling against the one collection and binds
the corresponding structural context.

The action proof table runs both `run` and `run_shell` across automatic policy
off/on, omitted toolchain versus explicit None, unassociated File and
FilesToRunProvider executables, direct and nested-depset tools, recognized
dependency runfiles and string executables, valid and unknown toolchain labels,
declared toolchain aliases versus their resolved real types (provider-view
fallback succeeds; action routing by the resolved real type fails),
valid, invalid and unknown explicit group names, and compatible versus
mismatched group/toolchain pairs. It checks both the selected context and error
precedence; no row may fall back to Default after a failed lookup.

Group properties are parsed once during collection construction. Both target
and each selected platform accept unqualified default keys and qualified group
keys. Every effective row is merged in the pinned order platform-default,
platform-group, target-default, target-group; the target-default-over-platform-
group discriminator is mandatory. The proof also supplies an unknown platform
property prefix and proves it is parsed but unused. `ConfiguredActionOwnerContext` preserves the
selected platform's unchanged `raw_platform_fact` separately from the completed
effective fact. R2 FileWrite sharing remains field-for-field unchanged: it
compares output shape, mnemonic/content/executable, selected platform label,
`raw_platform_fact` and constraint labels, and does not add group identity or
effective properties. Effective merged group properties feed action ownership,
aquery and REAPI. The cross-group conflict proof uses nonshareable Spawn actions.
No output suffix,
configuration-byte change or action-family expansion is admitted.

Before node publication, the parent runfiles collector unions the selected
toolchain runfiles closure from every Default, Named and Automatic row into the
existing `runfiles_packages`. A group child Need/error/cancellation publishes no
partial parent collection or closure. Named or automatic toolchain runfiles
edits must invalidate the parent even when its providers and actions are
otherwise equal.

The Phase B Rust allowlist, measured from preservation commit `f3c90ea46`, is
exactly:

- loading: `app/slug_loading_v2/src/package.rs`, `attrs.rs`, `subrule.rs`,
  `subrule_invocation.rs`, `host_package_load_tests.rs`, and
  `app/slug_loading_v2/tests/build_file_loading.rs`;
- configuration: `app/slug_configuration_v2/src/native/configuration.rs` and
  `native/tests.rs`;
- analysis production: new `app/slug_analysis_v2/src/execution_groups.rs`,
  `exec_group.rs`, `dice.rs`, `starlark_rule.rs`, `files_to_run_spawn.rs`,
  `subrule.rs`, `result.rs`, `lib.rs`, `toolchains/mod.rs`, and deletion of
  `toolchains/exec_groups.rs`;
- analysis proof: `app/slug_analysis_v2/tests/starlark_rule.rs`,
  `configured_target.rs`, and `toolchain.rs`;
- combined R2 consumers: `app/slug_core_v2/src/runtime/file_write_identity.rs`,
  `tests/configured_action_conflicts_tests.rs`,
  `app/slug_reapi_v2/tests/reapi.rs`, and `app/slug_cli_v2/tests/cli.rs`.

No Cargo manifest, BUILD file, fixture payload, server/query source or other R2
file may change. If a compiler error proves one omitted direct consumer needs a
mechanical exhaustive-match adaptation, stop for focused scope review before
editing it.

Estimated group-only growth is 1,750 production and 2,700 proof Rust lines.
Review caps are 2,300 production, 3,600 proof and 5,900 gross added Rust lines;
deletions do not create cap credit. Physical caps are 12,300 lines for
`package.rs`, 7,150 for `dice.rs`, 2,550 for `starlark_rule.rs`, 13,250 for its
integration test, and 650 for the new `execution_groups.rs`. `package.rs` keeps
only the binding/live/frozen/final carrier changes because extracting them would
expose evaluator-private types. `dice.rs` keeps DICE awaits and root orchestration
but moves normalized rows, property parsing and immutable collection validation
to the new module. Stop and replan on any cap breach or a second retained owner.

The exact new proof selectors are:

- `package::tests::exec_group_declarations_retain_constraints_and_named_transitions`;
- `host_package_load_tests::exec_group_reexports_publish_complete_target_semantics_and_restore`;
- `host_package_load_tests::rules_cc_exec_groups_retain_named_and_automatic_policy`;
- `execution_groups::tests::normalization_preserves_named_constraint_only_and_automatic_groups`;
- `execution_groups::tests::property_precedence_target_default_beats_platform_group`;
- `execution_groups::tests::unknown_target_group_prefixes_fail_and_unknown_platform_prefixes_are_unused`;
- `named_exec_group_resolves_independent_toolchains_constraints_and_provider_view`;
- `named_exec_transition_preserves_ordinary_and_configured_rows_uses_selected_platform_and_rejects_unknown`;
- `automatic_exec_group_policy_obeys_attribute_over_flag_aba`;
- `automatic_exec_group_projects_default_labels_from_automatic_rows_and_indexes_all_nondefault_rows`;
- `automatic_exec_group_routes_run_and_run_shell_across_the_complete_parameter_matrix`;
- `exec_group_collection_publishes_only_after_complete_resolution`;
- `exec_group_collection_invalidates_sources_requirements_constraints_payload_and_properties`;
- `exec_group_collection_unions_and_invalidates_named_and_automatic_toolchain_runfiles`;
- `default_exec_group_behavior_is_unchanged`;
- `configured_action_context_distinguishes_default_named_and_automatic_groups`;
- `configured_action_specs_route_every_group_without_fallback`;
- `configured_action_conflicts_cross_group_spawns_reject_before_publication`.

Each selector is preflighted and run exactly under the packet deadline. The
joint R2 selectors include
`configured_action_conflicts::validated_shared_closure_lowers_one_reapi_plan_and_keeps_aquery_owners`,
`configured_action_conflicts::one_shot_rejects_unused_conflicts_before_transport_and_output_changes`,
`configured_action_conflicts::stable_daemon_restores_sharing_after_repeated_conflicts`,
`configured_file_write_reapi_plan_reads_retained_platform_properties`, and
`selected_toolchain_request_reapi_payload_is_not_owner_identity`, plus every
selected-request/raw-platform/cutoff/cancellation A/B/A test touched by R2.
Complete loading, configuration, analysis, Build API and direct REAPI suites;
the direct server REAPI test; query/server/CLI compile checks; partitioned Core;
the three separately bounded CLI gates; and unchanged F3 remain mandatory.

Independent terminal design review returned `ACCEPT` on 2026-09-14 after the
freeze corrected automatic-row `ctx.exec_groups` exposure, composite
`ctx.toolchains` alias behavior, action parameter precedence, property validation,
the R2 raw/effective split, topology/runfiles ownership, configured transition
timing and both dependency-row producers. Phase B may begin exactly within this
allowlist and these caps. The seven prior Core timeouts remain joint acceptance
gates requiring diagnosis or bounded discriminating replacements.

Primary source authority is Bazel commit
`8220c6198837d5c13d53fea211cf3282aa12408a` in `/home/wgray/bazel`, using the
Stage 6 source/test matrix. Read only that pinned object when working-tree HEAD
differs. Buck2/DICE sources are ownership and memory guidance only.

## Required Phase B semantics

The reviewed freeze must preserve all of these obligations:

- named requirements and execution constraints resolve independently, including
  constraint-only groups, under the owner's target configuration;
- named dependency edges use the named group's selected execution platform and
  reject unknown names;
- public `ctx.exec_groups` exposes every resolved nondefault row, including
  Automatic rows, excludes Default and distinguishes an absent collection from
  an empty toolchain map;
- effective property precedence is platform-default < platform-group <
  target-default < target-group, including the target-default-over-platform-group
  discriminator;
- actions validate the public group then bind that group's owner/platform;
  automatic actions also validate explicit toolchain/group agreement;
- automatic policy comes from the retained native option plus rule attribute
  precedence, derives a distinct structural group per toolchain type and does
  not collapse public named identity;
- equal resolution inputs may share computation but never collapse group/action
  identity;
- the complete immutable collection publishes only after every group resolution,
  child and final source-certificate validation; Need/error/cancellation exposes
  no partial provider or action state;
- no second registry/cache, CLI reconstruction, evaluator-owned retained map,
  lock across DICE awaits, output suffix, fallback or broader action equivalence;
- R2 output-conflict validation remains before command success, RPC and
  materialization, while cquery remains independent.

Required invalidation proof covers same-DICE source/configuration/platform and
policy A/B/A; requirements, constraints, toolchain payload and properties;
absent/edit/delete/recreate; unavailable history; overlapping policies;
unchanged-input Arc cutoff; Need/error/cancellation; and default-only
nonregression. Views and joins are phase scratch; declarations and configured
collections are DICE-retained structural memory with Allocative coverage.

## Joint validation and acceptance

Use the pinned nightly `nightly-2025-09-14-x86_64-unknown-linux-gnu`. Every
compile/preparation command has a 60-second ceiling. Every test command has a
12-second deadline and 15-second absolute outer ceiling. Run Cargo serially per
target directory and preflight every exact nonignored selector with
`scripts/v2_test_preflight.py` after its final rebuild.

Joint acceptance requires:

- all reviewed declaration and configured group tests, including named,
  constraint-only, automatic-policy, transition, provider, property-precedence,
  action-routing and negative controls;
- all R2 selected-request/raw-platform/cutoff/cancellation/A-B-A tests and all
  configured-action conflict/sharing tests;
- complete `slug_analysis_v2` and `slug_build_api_v2` suites;
- every `slug_core_v2` library test in bounded exact partitions, with each
  candidate failure compared by exact selector to unchanged current main;
- the CLI drain control and separately bounded real one-shot, stable-daemon and
  positive completed-closure -> one `FileWriteReapiPlan` tests;
- complete direct `slug_reapi_v2` tests, the direct server REAPI consumer, and
  query/server/CLI compile dependents;
- unchanged F3 with its observer and cleanup receipt after complete group
  activation;
- pinned formatting, `git diff --check`, scope/growth checks,
  `python3 scripts/v2_plan_status.py` and independent final invariant review.

The reconciliation receipt is reusable only for untouched code: Analysis
150/150, Build API 79/79, direct REAPI 23 active with one ignored, direct server
REAPI consumer pass, and query/server/CLI checks pass. Core preflighted 329
nonignored tests with one ignored; 309 passed, 13 assertions failed identically
on current main, and seven exact commands timed out at 12 seconds on both lines.
Those seven remain resource gates and must be diagnosed or replaced by bounded
discriminating proof. Group edits invalidate every affected receipt.

Commit intermediate preservation/design checkpoints on the isolated branch.
Do not push R2 or group behavior to `main` until every joint gate passes.
Integration must be atomic, followed immediately by the unchanged F3 receipt.
Return `REPLAN` if Phase A cannot freeze one configured-target owner, demand is
unresolved, required semantics exceed a bounded reviewed packet, or the combined
runtime cannot preserve landed behavior.

## Immediate predecessor

Commit `2b3fedf76` recorded the now-superseded R2-before-groups order after F3
reached `target invocation for named execution-group semantics is unsupported`
in 9.98 seconds with valid observer and cleanup evidence. The reconciled trial
proved that R2's real command consumers traverse that same builtin registration
boundary. Independent review therefore requires a combined R2-based group stack
and joint acceptance.
