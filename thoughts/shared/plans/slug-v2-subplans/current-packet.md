# Current Slug V2 Work Packet

Packet: WP-4-6-7A-r2-execution-group-configurable-alias-combined-r2
Status: Phase A design freeze accepted; Phase B active

## Observable result

Complete and accept one atomic stack containing the unaccepted selected-request /
output-conflict R2 candidate, named and automatic execution groups, and the
narrow `attr.label` computed-default and native configurable-alias prerequisites
demanded in sequence by the authentic configured CLI fixture. Then pass bounded
partitions of the three real CLI consumer gates and replay unchanged F3. Nothing
in this packet may reach `main` separately.

The stack is preserved on
`integration/selected-request-output-conflict-r2`. Its checkpoints are R2
`f3c90ea46`, group implementation `7a2a149af`, compact automatic identity
`7b2fccb76`, and automatic qualifier/property correction `498ea2f49`. They are
reviewable preservation commits, not accepted behavior. The complete reviewed
R2/group contract is retained at
`7d48e6671:thoughts/shared/plans/slug-v2-subplans/current-packet.md`; this packet
inherits every invariant, proof, cap, and atomic-integration condition from that
record unless it explicitly tightens one below.

The first final review found that automatic groups had dropped label-qualified
target constraints and group properties. Commit `498ea2f49` corrects that
within the frozen owner: `//rule:type` and `@//rule:type` normalize against the
configured target, automatic constraints select platforms, and Default, Named,
and Automatic rows receive the full platform-default < platform-group <
target-default < target-group property order. Full analysis now passes 164/164.
That correction remains subject to final review.

The same review exposed the computed-default boundary. All three real CLI gates
and supervised F3 reach rules_java `toolchains/BUILD:138`, enter rules_cc
`cc_library.bzl:19`, and stop at its private `_def_parser` attribute because
computed-default target invocation is guarded. F3 selected exactly one test,
published a valid native observer result and cleaned up, then failed at that
guard. Commit `dab5cb5ea` implements the frozen computed-default contract. Its
three discriminating proofs, every affected group proof, full loading 563 active
plus one ignored, configuration 67/67, analysis 165/165, Build API 76/76, direct
REAPI 23 active plus one ignored, and the direct server REAPI consumer pass.

The next supervised F3 selected exactly one test, retained valid observer and
cleanup evidence, and advanced in 9.82 seconds through computed-default loading
to rules_java `toolchains/BUILD:205`. It now stops because native
`alias.actual` rejects that package's `select()`. Pinned Bazel defines `actual`
as an ordinary mandatory LABEL and explicitly documents alias as the reusable
configured-selection carrier. This later semantic stop triggers the bounded
replan below; it is not a timeout waiver.

## Compatibility and source authority

Exact within the admitted signature: an omitted Starlark
`attr.label(default = callback)` invokes its retained ordinary Starlark
callback during target declaration. The callback must have one or more required
positional-or-named parameters and no positional-only, defaulted, variadic,
keyword-only or residual parameters; each admitted parameter is supplied
positionally from the same-named, already-coerced, noncomputed rule attribute.
The result is a typed Label or `None`;
`None` becomes the label type default; an explicit target value bypasses the
callback; and the resulting label enters the attribute's already-retained
dependency configuration, including `cfg = "exec"`. Callback evaluation uses
the lexical `.bzl` source identities retained with the frozen rule and preserves
print capture and package failure atomicity.

Slug-native restrictions: callback signatures outside that fixed required
positional shape are rejected. A parameter that is absent, names another
computed attribute, has a present `None` value, or currently holds a
selector/concatenation fails before target publication; Bazel likewise omits
noncomputed null/None values from the parameter structure. Return values other
than Label or `None` fail with the attribute name. These restrictions cover
the selected rules_cc `def _def_parser_computed_default(name, tags)` shape
without inventing configured values.

Exact for native `alias.actual`: loading publishes a mandatory configurable
label expression and derives its selector labels into the existing hidden
`$config_dependencies` native attribute. Unconfigured query exposes every
actual branch as an `Ordinary` edge and every `$config_dependencies` condition
as an `Implicit` edge; only `actual` carries the typed selector value.
Configured analysis evaluates the expression in the alias's own target or exec
configuration and follows exactly the selected label with the unchanged
configuration. Literal aliases remain structurally and observably unchanged.

Loading rejects a missing required `actual`, direct `None`, a list, a branch
whose value cannot coerce as a label, and any multi-element selector expression
because Bazel's scalar LABEL type does not support select concatenation. A
retained expression that resolves to `None`, has no match/default, or is
ambiguous fails during configured analysis. These failures publish no configured alias result;
the loaded package remains a valid owner of any expression that passed loading.

Deferred: configurable-assignment precomputation for computed defaults and its combination limit,
computed-to-computed dependencies, function defaults on attribute kinds other
than label, rule initializers, aspects, subrules, symbolic macros, repository
rules, tag classes, and broader C++/Java provider/action behavior. The existing
initializer guard keeps precedence. This packet adds no fallback.

Primary computed-default authority is pinned Bazel commit
`8220c6198837d5c13d53fea211cf3282aa12408a`: `StarlarkAttrModule.java:356-368`
retains callback parameter names; `Attribute.java:1370-1492` computes defaults
from noncomputed attributes, invokes the callback, maps `None` to the type
default and type-checks its result; `StarlarkCallbackHelper.java:37-101` supplies
callback arguments by parameter name. `AttributeProvider.java:415-431` and
`RuleClass.java` establish loading-time specialization before configured
dependency consumption. The selected rules_cc 0.2.17
`cc/common/semantics.bzl:54-66` callback reads only `name` and `tags`, returning
`None` or `Label("@bazel_tools//tools/def_parser:def_parser")`; its descriptor
uses `cfg = "exec"`. The existing Stage 6 dependency-row resolver is the direct
configured consumer. Bazel configurable-combination tests are skipped because
that broader category remains explicitly unsupported. Native alias authority at
the same commit is `rules/Alias.java:54-89,102-116`: `actual` is a mandatory
LABEL without a nonconfigurable flag, the configured factory consumes its
selected prerequisite, and the public rule documentation names `select()`
reuse. Existing `configured_attribute.rs` supplies the admitted condition
matching, specialization, ambiguity and default semantics; this packet
introduces no second selector implementation. `BuildType.java:580-595` and
`BuildTypeTest.java:680-692` establish that LABEL rejects select concatenation
during attribute conversion.

## Owner and lifecycle

`FrozenRuleDefinition` remains the sole retained owner of sparse callback and
schema-index identity. `FrozenRuleDefinition::invoke` is the sole producer: it
coerces ordinary attributes, evaluates omitted computed labels in a fresh
attempt-local Starlark module using `MacroEvaluationContext`, replaces only
their default values, and publishes the final `StarlarkRuleImplementation`
through the existing `PackageRecorder`. Existing source/recursive-manifest DICE
dependencies own invalidation and A/B/A restoration. The result is part of the
ordinary package target's structural equality; no new DICE key, map, cache,
interner, registry, lock, service memory, request input, or retained evaluator
heap is admitted. Callback modules, lifted arguments and return values are
attempt scratch and drop on success, error, cancellation, or retry.

The callback is evaluated only after every noncomputed attribute has its final
loading value and before selector-key collection, implicit-output callbacks,
generated-file publication, or target publication. Explicit values remain
`AttributeProvenance::Explicit`; computed values use
`AttributeProvenance::Default`. A callback error publishes neither its target nor
earlier declarations from the failing package attempt. Existing analysis reads
the resulting `CoercedAttributeValue::Label` and the unchanged schema
`AttributeDependencyConfiguration`, so `cfg = "exec"` produces the configured
execution dependency without a command-side repair.

For configurable aliases, `PackageTargetKind::Alias.actual` changes from a
literal `CanonicalLabel` to the loading-owned `CoercedAttributeValue`. The
existing package load is the sole producer, publishes `actual` plus derived
`$config_dependencies` atomically, and its structural equality owns
invalidation. `ConfiguredNodeAnalysisKey` prepares the expression's selector
conditions through the existing DICE keys, resolves it once against the alias
configuration, verifies a single label, and computes that configured child.

Condition evaluation and the selected child are DICE dependencies of the alias
key. The configured graph publishes exactly one `AliasActual` edge. Consistent
with the existing Starlark-selector model, condition nodes do not become
configured graph edges; their completed configured results contribute
`RunfilesPackageClosureRow` extras so their packages remain in the alias
runfiles closure. A selector-condition or selected-child `Need` returns the
unioned `Need` and publishes no `ConfiguredNodeResult`; retry recomputes the same
alias key. Query derives branch labels and condition labels from the retained
expression with the edge kinds above. There is no new cache, key, registry,
lock, fallback scan or command repair.

## Scope, caps, and stops

The configurable-alias correction may additionally edit only:

- `app/slug_loading_v2/src/package.rs`;
- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- `app/slug_loading_v2/tests/build_file_loading.rs`;
- `app/slug_analysis_v2/src/dice.rs`;
- `app/slug_analysis_v2/tests/starlark_rule.rs` for the end-to-end configured
  dependency discriminator;
- `app/slug_query_v2/src/graph.rs` and
  `app/slug_query_v2/tests/loading_query.rs` only for the public
  representation's unconfigured projection;
- `app/slug_cli_v2/tests/cli.rs` only to partition the inherited monolithic
  consumer proofs into exact selectors that each fit the standing deadline;
- this manifest, canonical Live Status, Stage 4, Stage 6, bootstrap readiness,
  and the configured CLI fixture ledger.

No Cargo manifest, BUILD file, fixture payload, server or CLI production source,
configuration source, or new DICE key may change for this prerequisite.
`package.rs` stays cohesive because the frozen callback,
evaluator-private values, package recorder, coercion and publication all meet in
the target invocation loop; extraction would expose evaluator lifetimes without
creating a second semantic owner. Existing `dice.rs` already owns configured
native alias analysis and selector preparation; the correction joins those two
local paths rather than adding an owner.

From `f3c90ea46`, the inherited combined caps remain 2,300 production, 3,600
proof, and 5,900 gross added Rust lines, with no deletion credit. The alias and
test partition delta is estimated at 90 production and 260 proof lines.
Tightened physical caps are
12,300 lines for `package.rs`, 39,000 for
`host_package_load_tests.rs`, 7,150 for `dice.rs`, 2,550 for production
`starlark_rule.rs`, 13,250 for its integration test, and 650 for
`execution_groups.rs`; the newly admitted caps are 5,300 for
`build_file_loading.rs`, 3,500 for query `graph.rs`, 6,000 for
`loading_query.rs`, and 6,050 for CLI `cli.rs`. Stop and replan for a second retained owner, a new DICE
key/cache, configurable callback precomputation, an alias transition,
initializer execution, a
manifest/build/fixture change, a required production file outside the inherited
allowlist, or any cap breach.

## Discriminating evidence

Add and preflight exact selectors for:

- `host_package_load_tests::rule_computed_default_reexport_invokes_labels_bypasses_explicit_and_restores`:
  imported/re-exported callbacks observe exact `name`/`tags`, return typed root
  and external labels or `None`, explicit `_def_parser` bypasses a failing
  callback, label provenance is Default versus Explicit, and callback `print()`
  reaches the existing capture. A callback-source A/B/A edit runs on one DICE
  graph and restores the exact package result. The failure leg declares an
  earlier target before the failing callback target and proves the failed
  package publishes neither;
- `host_package_load_tests::rule_computed_default_rejects_unavailable_configurable_and_invalid_results`:
  missing/computed/None parameters, selectors/concatenations, every excluded
  callback signature class, string/integer/list returns and callback failure
  produce bounded attribute-named errors;
- `computed_default_label_uses_retained_exec_dependency_configuration` in
  `app/slug_analysis_v2/tests/starlark_rule.rs`: a real loaded callback result
  produces a `ConfiguredAttributeDependency::Exec(Default)` edge whose child
  uses the selected Exec configuration, while a same-label Target attribute is
  the structural control.

The positive loading proof must use `Label()` inside the retained callback so
lexical source mapping is exercised. The configured end-to-end selector is
mandatory.

Add and preflight these exact alias selectors:

- loading `native_alias_retains_configurable_actual_and_config_dependencies`
  proves literal and multi-branch `actual`, typed selector retention, derived
  `$config_dependencies`, and branch/condition separation;
- loading `native_alias_rejects_invalid_loading_shapes_atomically` proves
  missing, direct-None, list, non-label branch coercion and scalar-label select
  concatenation fail at loading and publish no target from the failed package
  attempt;
- analysis `configurable_alias_selects_actual_and_restores_same_key` edits one
  config-setting source A/B/A on one DICE graph while recomputing the same
  configured alias key, proves exact restoration, preserves the alias
  configuration on its selected child, exposes only one `AliasActual` graph
  edge and includes condition packages through extra runfiles rows;
- analysis
  `configurable_alias_propagates_condition_and_child_needs_without_publication`
  separately forces selector-condition and selected-child `Need`, then retries
  the same key to completion and proves no configured alias result published
  before either dependency completed;
- analysis `configurable_alias_rejects_invalid_configured_selections` covers
  selected `None`, no-match/no-default and ambiguity at configured analysis
  with no stale configured result;
- query `configurable_alias_projects_branch_and_condition_edges` proves root
  branch labels are `Ordinary`, condition labels are `Implicit`, and only the
  `actual` attribute retains the typed selector;
- query `external_configurable_alias_restores_same_owner_route` proves only
  same-owner external labels regain that owner's apparent route. Cross-repo and
  cross-package external alias query projection retain the existing explicit
  deferred errors; no arbitrary apparent spelling is claimed.

Rerun every affected corrected group selector and the full loading,
configuration, analysis, Build API and direct REAPI suites. Run the direct
server REAPI consumer and query/server/CLI compile dependents. Account for the
exact Core binary: candidate and current main both produced 313 passes, eight
matching assertion failures and nine matching 12-second timeouts across 330
active tests with one ignored; this supersedes the older 309/13/7 receipt.
Rerun Core only if this package-only delta changes its binary or dependencies.

The original one-shot, stable-daemon and positive shared-closure tests now exceed
the whole-test deadline after progressing farther through the real closure.
Replace them with the exact selectors below, using shared helpers without
weakening or deleting an assertion:

- `one_shot_build_conflict_is_atomic_and_recovers`,
  `one_shot_run_conflict_is_atomic_and_recovers`,
  `one_shot_aquery_literal_conflict_is_atomic_and_recovers`, and
  `one_shot_aquery_deps_conflict_is_atomic_and_recovers`: each owns a fresh
  workspace and transport probe and performs baseline aquery -> source mutation
  -> its one rejecting command -> unchanged output/unused transport check ->
  source restore -> exact baseline aquery restoration. The build and run cases
  retain their transport flags. The aquery cases retain their command-specific
  diagnostic category. Each mutation additionally runs the conflict-disabled
  cquery control before restoration;
- `stable_daemon_build_conflict_retries_and_restores`,
  `stable_daemon_run_conflict_retries_and_restores`,
  `stable_daemon_aquery_literal_conflict_retries_and_restores`, and
  `stable_daemon_aquery_deps_conflict_retries_and_restores`: each owns one
  output base and transport probe, records one baseline/PID, mutates source
  once, invokes the same rejecting command twice while proving unchanged output,
  unused transport and the same PID after each attempt, then restores source and
  proves exact aquery/PID/output restoration;
- `validated_shared_closure_keeps_semantic_owners_and_lowers_one_reapi_plan`
  jointly proves the same fixture has two distinct semantic owners, one shared
  execution representative and one REAPI plan;
- `validated_shared_closure_aquery_keeps_both_owners` proves that fixture's
  unshared aquery owner view still names both writers.

Run each bounded selector separately under
the standing deadline, followed by
unchanged supervised F3 with observer and cleanup. A new later authentic
semantic stop triggers a bounded replan; a passed package callback that merely
advances the closure does not accept the stack. Matching timeouts are open
resource gates.

Use pinned `nightly-2025-09-14-x86_64-unknown-linux-gnu`. Preparation and
compilation have a 60-second ceiling; each test command has a 12-second deadline
and 15-second absolute outer ceiling. Use direct pinned binaries, explicit
`--target-dir /home/wgray/slug/target`, serial Cargo commands, and
`scripts/v2_test_preflight.py` for every exact nonignored selector after its
final rebuild. Finish with pinned formatting, `git diff --check`, scope/growth
checks, `scripts/v2_plan_status.py`, and independent final invariant review.

## Review and acceptance

Independent design review must confirm the alias retained representation,
condition/branch separation, configured selection point, target/exec
configuration propagation, query projection, failure/retry behavior, bounded
CLI partition, and unchanged computed-default/R2/group ownership before alias
runtime edits. Final review must confirm the narrow computed-default execution
point, label context, parameter availability, provenance, `cfg = "exec"`
handoff, failure atomicity and unchanged R2/group ownership. Final review
must inspect `498ea2f49`, the computed-default delta, every affected proof and
the CLI/F3 receipts. Only an `ACCEPT` after all joint gates permits one atomic
integration commit on `main`; push only that accepted main commit.

Computed-default design review returned `ACCEPT` on 2026-09-14 after the freeze
made the configured handoff selector end to end, excluded present `None`
parameters and unsupported callback signatures, and required observed print
capture, one-DICE A/B/A restoration and whole-package failure atomicity. The
configurable-alias correction requires the new review above before production
edits.

Configurable-alias design review returned `ACCEPT` on 2026-09-14 after two
corrections froze loaded versus configured publication, extra-runfiles-only
condition ownership, same-key A/B/A and both `Need` retries, exact query edge
kinds/external spelling, loading-time scalar-label concatenation rejection,
physical caps/selectors, and the original one-mutation/two-rejection daemon
lifecycle. Phase B may edit only the frozen allowlist.

## Immediate predecessor

`WP-4-6-7A-r2-execution-group-computed-default-combined-r1` implemented the
computed-default prerequisite at `dab5cb5ea` and passed its focused and broad
library gates. Its F3 advanced to native configurable alias loading, and its
three real CLI tests advanced far enough that their multiple complete command
matrices no longer fit one 12-second test process. It remains part of this
unaccepted atomic stack. Before it,
`WP-4-6-7A-r2-execution-group-combined-r1` implemented the shared group
collection on unaccepted R2 and passed loading 562 active tests, configuration
67/67, corrected analysis 164/164, Build API 76/76, direct REAPI 23 active plus
one ignored, and compile consumers. Its final review returned `REPLAN` for the
automatic qualifier defect now corrected at `498ea2f49` and for this authentic
computed-default prerequisite. The three CLI gates and F3 all identify the same
`_def_parser` boundary.
