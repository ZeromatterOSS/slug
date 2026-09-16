# Current Slug V2 Work Packet

Packet: WP-4-6-7A-r2-main-core-attribution-final-preparation-r2
Status: final comparator preparation recovery ready; independent replan review ACCEPT

## Result and acceptance boundary

Run one final compile-only continuation for the missing unchanged-main Core
comparator, using clean main `4824a0861`, frozen feature `055b6fe18`, the
identical absolute Cargo command, working directory, pinned PATH/toolchain,
environment, single build job and warmed target directory. Do not clean, change
features, alter profile or incremental settings, or prepare another package
first. Exactly one invocation is authorized under a 360-second ceiling. This is
a newly reviewed resource recovery for the final `slug_core_v2` crate, not a
retry of the failed 180-second packet.

Success requires completely parsed Cargo JSONL, zero compiler errors, one
`build-finished` row with `success=true`, exactly one `slug_core_v2` test
executable, clean sole process-tree cleanup and the executable SHA-256. A stop,
malformed receipt or multiple/missing executable ends this evidence path as an
external resource blocker; do not retry or raise the ceiling again.

Only after success, exact-list the eight frozen names once, require each exactly
once and nonignored, then run only those eight once under the unchanged
12-second TERM/three-second kill limits into one atomic receipt. Compare each
main row with its frozen candidate row by selector, exit/timing class, terminal
assertion or panic family and material expected/actual diagnostic. A pass,
timeout, missing or different row, or incomplete comparison rejects baseline
attribution.

Freeze every passing feature artifact and both earlier preparation records. No
candidate rerun, source/test/lockfile/Cargo change, historical-timeout selector,
replacement partition, full sweep, CLI conflict, F3, merge, acceptance commit
or push is allowed.

## Failed 180-second preparation history

The accepted r1 recovery invoked the identical main Core preparation once under
its 180-second ceiling. It exited 124 while compiling `slug_core_v2`, after
clearing bzlmod, loading and analysis, and ran zero tests. JSON receipt
`/tmp/slug-main-core-attribution-180-compile.json` has SHA-256
`232d6ce07b1cfaefaf8ddfde359d3c728751e26281c780534fd3f930ec5684c7`:
366 compiler artifacts, 51 build-script rows, 17 compiler messages, zero
compiler errors, zero executables and no `build-finished` row. The process tree
and main worktree were clean afterward.

## Prior comparator recovery contract

Recover only the missing unchanged-main attribution for the eight frozen
candidate Core assertion failures. Freeze clean feature checkpoint `055b6fe18`
and its accepted artifacts: Core proof receipt
`35c469ecd891903220ebde8bb7a13d16edf691155d6f9acfdb22c7e4ae6d3bef`,
REAPI receipt `1fca8402a39c0007f65ed57dc3c6a4de624e00d6ae992877cd9cc443ee7f691b`,
server receipt `1a61d473b634226b683cc42ca945db1237c8cf2fae9051671c162ba182845076`
and the successful query/server/CLI compile check. Do not rerun any of them.

The unchanged-main worktree is clean at exact commit `4824a0861`. Six resumed
60-second `slug_core_v2 --lib --no-run --message-format=json` preparation
slices ran zero tests and produced no valid current-main Core binary; the last
three repeatedly restarted `slug_bzlmod_v2`. This packet authorizes exactly one
same-command preparation with the pinned toolchain, environment and warmed
target directory under a 180-second ceiling. This resource-boundary exception
does not change any test deadline. Require complete Cargo JSON, zero compiler
errors, `build-finished` success, exactly one Core test executable, clean
process-tree cleanup and the executable SHA-256. Do not retry if it stops.

If preparation succeeds, preflight only the eight frozen selector names below,
then run each exactly once into one atomic receipt with the unchanged 12-second
TERM and three-second kill envelope. Compare every main row with its frozen
candidate row by selector, exit/timing class, terminal assertion or panic
family, and material expected/actual diagnostic. All eight must reproduce the
candidate failure class to become baseline-attributed. A main pass, missing or
ignored selector, timeout, different assertion class, incomplete receipt or
180-second preparation stop requires replan. Source inspection cannot waive a
result.

No source, test, lockfile, fixture, candidate, Cargo/BUILD or production change
is allowed. Do not run the nine prohibited original timeouts, either completed
replacement partition, a candidate selector, a full Core sweep, authentic CLI
conflict, or F3. Merge, the main acceptance commit and the authorized main push
remain prohibited until this attribution succeeds and independent final atomic
review accepts every predecessor gate.

## Frozen eight-selector comparator set

- `runtime::dice::tests::build_command_tests::build_command_root_selects_each_terminal_producer_once_for_duplicate_targets`;
- `runtime::dice::tests::build_command_tests::build_command_root_terminal_closure_retains_reused_and_clears_retry_only_batches`;
- `runtime::dice::tests::build_command_tests::multi_target_exported_sources_do_not_enter_revision_bridge`;
- `runtime::dice::tests::build_command_tests::public_external_single_uses_observed_family_and_full_source_certificate`;
- `runtime::dice::tests::build_command_tests::resolved_run_view_reuses_exact_executable_filewrite_relation`;
- `runtime::dice::tests::build_command_tests::root_exported_source_revision_bridge_retries_changed_terminal_and_preserves_epoch`;
- `runtime::dice::tests::cquery_drives_only_the_requested_root_across_platform_retries`; and
- `runtime::dice::tests::query_command_tests::direct_external_query_uses_host_route_native_materialization_and_apparent_output`.

## Completed proof history

Replace nine over-broad Core tests that exceeded the standing 12-second exact
deadline with a proof-only set of smaller exact selectors. Preserve every
assertion and semantic return path, including real public build/cquery command
ownership, while making each selector independently bounded. This packet
changes no production behavior, compatibility class, DICE key, retained value,
command surface or execution-group/R2 implementation.

The predecessor implementation remains unaccepted at local commit
`1b09dbfa1` on `integration/selected-request-output-conflict-r2`. Freeze its
production source. No feature-branch commit may be pushed, and no merge or
`main` push is authorized until the replacement proof set passes, the remaining
predecessor gates resume and independent final review accepts the complete
atomic stack.

Exact compatibility remains limited to the predecessor packet's pinned Bazel
9.2 named/automatic execution-group and selected-request/configured-conflict
surface. DICE key shape, immutable configured owners, closure ordering and
typed error storage remain Slug-native. This packet changes only the size and
composition of tests for already-declared behavior.

## Predecessor stop and frozen evidence

The corrected final Core candidate binary was compiled in 59.20 seconds and is
frozen at SHA-256
`6001c0a41074422961a36c08f6e4618df37db8182504ccc059aa46b4632b83d6`.
Its complete supervised receipt is SHA-256
`0df68baf746ca3916614ee2f103a7a64ab2116d9cd60bf37248ce40927699d8d`.
The receipt preflighted 331 listed tests, exactly one ignored test and 330
active selectors. In the approved host environment it recorded 313 passes,
eight non-timeout exit-101 results and nine exact timeouts, with a durable row
for every active selector. All seven selectors in
`runtime::dice::tests::configured_action_conflicts_tests` passed, including the
new cross-group conflict proof.

The eight non-timeout results are:

- `runtime::dice::tests::build_command_tests::build_command_root_selects_each_terminal_producer_once_for_duplicate_targets`;
- `runtime::dice::tests::build_command_tests::build_command_root_terminal_closure_retains_reused_and_clears_retry_only_batches`;
- `runtime::dice::tests::build_command_tests::multi_target_exported_sources_do_not_enter_revision_bridge`;
- `runtime::dice::tests::build_command_tests::public_external_single_uses_observed_family_and_full_source_certificate`;
- `runtime::dice::tests::build_command_tests::resolved_run_view_reuses_exact_executable_filewrite_relation`;
- `runtime::dice::tests::build_command_tests::root_exported_source_revision_bridge_retries_changed_terminal_and_preserves_epoch`;
- `runtime::dice::tests::cquery_drives_only_the_requested_root_across_platform_retries`; and
- `runtime::dice::tests::query_command_tests::direct_external_query_uses_host_route_native_materialization_and_apparent_output`.

An unchanged-main comparison remains missing. Preparing a Core test binary at
unchanged `main` `4824a0861` advanced through worktree-specific dependency
layers but reached the 60-second preparation ceiling three times without
selecting or running a test. This comparison is useful baseline attribution but
cannot cure the timeout stop. Resume it only after this proof design is
accepted; never run the nine prohibited timeout selectors on `main`.

The nine selectors that reached 12.004--12.008 seconds are:

- `runtime::dice::tests::build_command_tests::public_multi_build_aggregates_sources_and_accepts_analysis_dependency_superset`;
- `runtime::dice::tests::build_command_tests::public_singleton_observation_replays_lifecycle_and_isolates_legacy`;
- `runtime::dice::tests::cquery_command_tests::cquery_evaluates_ordered_function_free_set_expressions_over_shared_roots`;
- `runtime::dice::tests::cquery_command_tests::cquery_executables_deps_filters_complete_closure_and_induces_edges`;
- `runtime::dice::tests::cquery_command_tests::cquery_restores_structural_configuration_and_display_projection`;
- `runtime::dice::tests::cquery_command_tests::cquery_uses_only_observed_families_and_replays_child_events_once`;
- `runtime::dice::tests::cquery_deps_uses_the_retained_noimplicit_graph_with_null_sources`;
- `runtime::dice::tests::real_build_command_drives_typed_analysis_and_cold_events_without_warm_replay`; and
- `runtime::dice::tests::retained_runtime_restores_default_transition_configuration_after_explicit_override`.

Do not run these nine selectors again on either line. The earlier restricted
sandbox attempt recorded only 291 passes, 39 exit-101 results and zero timeouts,
lost exact failure membership to truncated console output and is inadmissible.
The approved-host receipt supersedes it.

The direct REAPI compile preparation also reached 60 seconds before producing
a test executable. All remaining REAPI/server/compile/final-review gates were
stopped after the mandatory Core timeout boundary; their earlier focused passes
remain valid only where their binaries and relevant source stay unchanged.

## Semantic owners and proof rule

The proof remains attached to the real owners named in the timed bodies:

- `WorkspaceRuntime::{build_command_with_bzlmod_inputs,cquery_command_with_bzlmod_inputs}`
  owns public command driving, accepted terminal publication and event replay;
- `BuildCommandRootKey`, `BuildCommandRootObservationKey` and their observed
  terminal/source certificates own build root results, dependencies and
  lifecycle;
- configured target keys and structural configurations own target/transition
  identity, output roots and C0/C1/C0 restoration;
- `CqueryCommandEvaluation` owns ordered result sets, configured analyses and
  induced graph projections; and
- accepted native demand snapshots and activation audits observe retained
  path epochs and route-family use without becoming semantic state.

The repository DICE guidance in `docs/developers/dice.md` remains binding:
warm reuse must be explained by tracked dependencies, and a silent timeout may
signal lock/re-entry trouble. Therefore a replacement for a public root or DICE
lifecycle assertion must execute that same real owner. Every table row preserves
the original public command expression or expressions, the outer `Ok`/`Err`
classification and the terminal `Ok`/`Err` classification. Test-only helpers
may factor fixture construction or assertions over the returned evaluation;
they may not synthesize an evaluation, substitute a direct set/graph call or
eliminate a command. No mock, fresh-graph bypass, observer, diagnostic
supervisor, deleted assertion, changed expectation, semantic repair or longer
deadline is allowed.

## Assertion-to-owner table

Line references identify every semantic assertion and command return path in
the frozen nine bodies. Fixture-construction `unwrap` calls before a command
are setup preconditions; each command success/error `unwrap`, terminal branch
and assertion is included in the ranges below.

| Frozen selector and source range | Owned claim/return path | Required replacement selector |
|---|---|---|
| `public_singleton...`, `build_command_tests.rs:2169-2184` | Real build cold terminal, exact module/package events, no repository requests and observed-only root activation | `public_singleton_cold_terminal_events_and_observed_route` |
| same, `2186-2195` | Warm command emits no events and reuses every retained path-observation Arc | `public_singleton_warm_reuses_epoch_without_event_replay` |
| same, `2196-2209` | Real edit changes the terminal, delete errors and recreate restores the original A terminal | `public_singleton_edit_delete_restore_lifecycle` |
| same, `2211-2216` | Legacy exact-target request does not activate the observed family and does activate legacy | `public_singleton_legacy_route_isolation` |
| `public_multi...`, `build_command_tests.rs:2602-2630` | Real three-root cold order, target/source/analysis shapes and selected repository request/validation inventory | `public_multi_cold_aggregates_targets_and_repository_inputs` |
| same, `2632-2701` | Observed root equals public result, is a dependency subset, validates superset association, certificates both sources by shared Arc, orders revision before source dependencies and excludes neutral/legacy roots | `public_multi_observed_superset_certificate_and_dependency_order` |
| same, `2702-2735` | Warm silence, source edit to `ONE` with certificate bytes/Arc verification, then a silent restore command | `public_multi_source_edit_restore_without_event_replay` |
| same, `2737-2783` | Analysis error publishes no events/selected repository state, stays observed-only and a corrected fresh runtime recovers | `public_multi_analysis_failure_is_atomic_and_recovers` |
| `cquery_executables...`, `cquery_command_tests.rs:61-102` | Public cquery depth 0/1/full executable filtering, repeated configured analysis/label occurrences, kinds and induced root/direct edges | `cquery_executables_depth_and_complete_closure` |
| same, `104-159` | Reverse self/0/full selection, distinct configurations, depth bounds, induced edges and empty bridge case | `cquery_executables_reverse_depth_and_edges` |
| same, `161-234` | `filter`, `kind` and chained executable projections preserve every admitted text/graph view | `cquery_executables_filter_kind_composition` |
| same, `236-324` | Depth 0/1/2/max composition matrices and empty filter/kind outputs | `cquery_executables_depth_boundaries_and_empty_results` |
| `cquery_evaluates...`, `cquery_command_tests.rs:486-526` | Empty set/let returns no results or roots; oversized count fails request parsing before activation | `cquery_set_empty_and_count_preflight` |
| same, `527-587` | Ordered union/set/let/intersect/except/filter/some semantics and deduplication | `cquery_set_ordered_operators_filter_and_some` |
| same, `588-606` | Empty/zero/negative `some` returns the declared terminal error | `cquery_some_empty_zero_and_negative_errors` |
| same, `607-676` | Starlark labels, missing-target precedence over malformed nested regex and malformed-regex terminal | `cquery_set_starlark_and_error_precedence` |
| `cquery_restores...`, `cquery_command_tests.rs:716-799` | Public C0/C1/C0 structural option, display projection, topology and event behavior | `cquery_configuration_c0_c1_c0_projection_and_topology` |
| same, `801-827` | Missing-target envelope, fresh-runtime equality and default build-setting configuration | `cquery_configuration_missing_fresh_and_setting_default` |
| same, `829-845` | Parent dependency uses the transitioned child configuration | `cquery_configuration_transitioned_child` |
| `cquery_uses_only...`, `cquery_command_tests.rs:941-985` | Real cold event order, a nonempty cold path-observation epoch, warm silence and equal/pointer-identical retained path results | `cquery_observed_cold_warm_events_and_epoch_reuse` |
| same, `988-1011` | Target/union/rdeps requests avoid legacy package/analysis families and activate only the required observed families | `cquery_observed_family_route_isolation` |
| `real_build_command...`, `dice.rs:8996-9026` | Real cold build counts and exact module/Bzl/BUILD/analysis event order | `real_build_cold_counts_and_event_order` |
| same, `9028-9076` | Warm silence, explicit configuration change, output-root distinction and C0 restoration | `real_build_warm_transition_restore_and_output_root` |
| same, `9078-9101` | Empty roots return empty counts; missing target returns typed exact diagnostic after loading events | `real_build_empty_and_missing_target_terminal` |
| `retained_runtime...`, `dice.rs:9207-9288` | Parent C0/C1/C0 identity, analysis-event cutoff, topology equality and distinct output roots | `retained_configuration_c0_c1_c0_topology_and_events` |
| same, `9290-9317` | Fresh default equality/projection and transitive left-transition child | `retained_configuration_fresh_and_transitive_transition` |
| same, `9319-9350` | Top/parent reanalysis excludes unchanged consumer; direct setting stays default | `retained_configuration_top_change_and_setting_default` |
| `cquery_deps_uses...`, `dice.rs:9531-9619` | Request rejection without `--noimplicit_deps`, depth-zero/full ordered results, null nodes/kinds and distinct ordinary configurations | `cquery_deps_preflight_depth_null_and_identity` |
| same, `9621-9696` | Structural kind/filter projections, zero induced edges and duplicate configured identity | `cquery_deps_structural_filter_and_duplicate_identity` |
| same, `9698-9839` | Normalized topology plus reverse depth negative/0/1/full nodes, edges and formats | `cquery_deps_reverse_depth_and_topology` |
| same, `9840-9932` | Direct/normalized and filtered rdeps preserve labels, kinds, Starlark, graph and configured keys; selected subgraph keeps induced edges | `cquery_deps_rdeps_composition_and_selected_subgraph` |
| same, `9933-10083` | Empty filter, kind depth boundaries, aliases, inner-depth normalization and negative composition | `cquery_deps_filter_kind_and_inner_depth_boundaries` |
| same, `10084-10169` | Broken/unreachable/missing/regex/universe/default-seed precedence, transitioned seed and BUILD edit/restore | `cquery_deps_errors_transition_and_edit_restore` |
| same, `10171-10237` | Deps depth 1/2/max topology closure and include-tool equivalence | `cquery_deps_depth_closure_and_tool_flag_equivalence` |

The frozen 313-pass receipt additionally retains direct discriminators such as
`observed_multi_reducer_and_selected_superset_preserve_total_order_and_arcs`,
`observed_build_replays_lifecycle_and_cancellation_without_parent_publication`,
`prepared_command_configuration_keeps_distinct_transitioned_children`,
`cquery_executables_uses_rule_capability_order_and_full_key_dedupe`,
`cquery_evaluator_terminal_classification_is_narrow`, and the focused cquery
Need/error/cancellation selectors. They support the owner mapping but do not
replace the real public-command selectors named above.

## First replacement checkpoint and second partition

Unaccepted implementation checkpoint `678ff2e7c` materialized the 34-row
table within the first caps. Its final Core binary
`66ce3e7472d9c9982139e6ad6c5c38cf4a46ee8c8abcaae9f24e25d899de9b16`
compiled in 58.81 seconds. Atomic approved-host receipt
`3d2c7f0706c0760da702117ed47f9d9cb5a4f8e79be5e43be2630e28b5c59656`
records 25 exact passes, nine exact 12.004-second timeouts and zero assertion
failures. Freeze the 25 passing bodies and all shared fixture behavior; do not
rerun them.

The nine timed first replacements are
`cquery_executables_reverse_depth_and_edges`,
`cquery_executables_filter_kind_composition`,
`cquery_executables_depth_boundaries_and_empty_results`,
`cquery_set_ordered_operators_filter_and_some`,
`cquery_deps_reverse_depth_and_topology`,
`cquery_deps_rdeps_composition_and_selected_subgraph`,
`cquery_deps_filter_kind_and_inner_depth_boundaries`,
`cquery_deps_errors_transition_and_edit_restore`, and
`cquery_deps_depth_closure_and_tool_flag_equivalence`. Remove and replace only
these bodies. Do not rerun their names.

The final second partition compiled in 32.60 seconds as Core binary
`af7fa9c80829856b544d03cfb668df61d83592208044f8342f70ee3fd643396b`.
Preflight found all 53 exact selectors once and nonignored. Atomic approved-host
receipt `35c469ecd891903220ebde8bb7a13d16edf691155d6f9acfdb22c7e4ae6d3bef`
records 53 passes, zero failures and zero timeouts under the unchanged 12-second
command and 15-second absolute ceilings. Independent final review ACCEPT
confirmed the frozen 25 passing bodies and shared fixtures remain unchanged,
all original expressions, result classes, assertion values and edit order are
preserved, and gross additions are 1,219 of the 1,250-line cap. Do not rerun
these 53 selectors; resume the deferred unchanged-main attribution and the
remaining predecessor gates.

Every second-partition selector gets a fresh fixture and executes no more than
three real public command evaluations. Preserve the original expression
strings, outer and terminal result classes, assertion values, edit order and
returned-evaluation comparisons. The 53 exact second-partition names are:

- executable reverse: `cquery_executables_reverse_self_zero_and_empty`,
  `cquery_executables_reverse_full_identity_and_upper_bounds`,
  `cquery_executables_reverse_negative_and_zero_bounds`;
- executable composition: `cquery_executables_chained_and_named_kind_composition`,
  `cquery_executables_upper_depths_match_full`,
  `cquery_executables_direct_filter_and_kind_match_full`;
- executable depth/empty: `cquery_executables_filter_depth_zero`,
  `cquery_executables_filter_depth_one`,
  `cquery_executables_filter_upper_depths`,
  `cquery_executables_kind_depth_zero`,
  `cquery_executables_kind_depth_one`,
  `cquery_executables_kind_upper_depths`,
  `cquery_executables_chained_depth_zero`,
  `cquery_executables_chained_depth_one`,
  `cquery_executables_chained_upper_depths`,
  `cquery_executables_named_kind_depth_zero`,
  `cquery_executables_named_kind_depth_one`,
  `cquery_executables_named_kind_upper_depths`,
  `cquery_executables_empty_compositions`;
- ordered sets: `cquery_set_union_set_and_let`,
  `cquery_set_intersect_and_except`,
  `cquery_set_exact_prefix_and_missing_filters`,
  `cquery_set_default_counted_and_filtered_some`;
- reverse topology: `cquery_deps_depth_zero_and_full_topology`,
  `cquery_deps_zero_and_one_reverse_topology`,
  `cquery_deps_full_and_upper_reverse_bounds`,
  `cquery_deps_negative_reverse_is_empty`;
- direct/normalized rdeps: `cquery_deps_direct_normalized_default`,
  `cquery_deps_direct_normalized_negative`,
  `cquery_deps_direct_normalized_zero`,
  `cquery_deps_direct_normalized_one`,
  `cquery_deps_direct_normalized_max`;
- filtered rdeps/subgraph: `cquery_deps_filtered_default_and_max`,
  `cquery_deps_filtered_negative`, `cquery_deps_filtered_zero`,
  `cquery_deps_filtered_one`, `cquery_deps_selected_subgraph`;
- kind/inner boundaries: `cquery_deps_empty_kind_empty_and_aliases`,
  `cquery_deps_kind_zero_and_bounded_zero`,
  `cquery_deps_kind_full_one_and_max`,
  `cquery_deps_negative_bounds`,
  `cquery_deps_inner_zero_and_one`,
  `cquery_deps_inner_two_and_max`, `cquery_deps_composed_zero`;
- error/edit boundaries: `cquery_deps_broken_unreachable_and_missing`,
  `cquery_deps_filter_regex_precedence`,
  `cquery_deps_kind_regex_precedence`,
  `cquery_deps_universe_default_and_transitioned_seed`,
  `cquery_deps_build_edit_restore_lifecycle`; and
- deps closure/tool: `cquery_deps_depth_one_topology`,
  `cquery_deps_full_and_depth_two`, `cquery_deps_full_and_depth_max`,
  `cquery_deps_full_and_without_tools`.

## Scope, caps and validation

Production additions are exactly zero. Test edits are limited to:

- `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`;
- `app/slug_core_v2/src/runtime/tests/cquery_command_tests.rs`; and
- the `#[cfg(test)]` module in `app/slug_core_v2/src/runtime/dice.rs`.

Plan receipts may update this packet, Stage 6 and the canonical plan. No
analysis, loading, configuration, CLI, REAPI, server, query, Cargo/BUILD,
fixture or production Core code may change.

The second partition requires 53 replacement selectors. Shared test-only
fixture builders may reduce repeated setup but may not cache semantic state
across selectors. Cap gross proof additions from design commit `6a60b4271` at
1,250 Rust lines. Physical caps are 4,350 lines for
`build_command_tests.rs`, 1,500 for `cquery_command_tests.rs` and 13,350 for
`dice.rs`. These large files remain cohesive for this packet because the first
two are owner-specific included test modules and the third may change only its
existing test module. A new production owner, new semantic helper, fixture
file, or cap excess requires replan.

Remove and replace only the nine timed first-partition bodies without changing
any assertion value. Compile one final Core test binary under the 60-second
preparation ceiling. Preflight all 53 exact second-partition names against that
binary, then run each once with the standing 12-second command deadline and
15-second absolute ceiling. A timeout or assertion failure replans. Preserve
the frozen 313 broad passes and 25 first-partition passes; do not rerun any of
their names or broaden into another full-Core sweep.

Run `cargo fmt --all -- --check`, `git diff --check`, scope/growth checks and
`python3 scripts/v2_plan_status.py`. Independent final review must compare the
new selectors against every table row and confirm that public root/DICE claims
still execute the real owner. After that proof passes, resume the eight-selector
unchanged-main attribution and the predecessor's remaining direct
REAPI/server/compile/final-review gates. Only complete predecessor acceptance
may merge the atomic stack, create an acceptance receipt on `main` and push
`main` to the already authorized ZeromatterOSS remote.

## Stops

Return `REPLAN` for any production change, weakened/removed assertion, mock or
fresh-graph substitute for a public lifecycle claim, new semantic owner,
timeout, cap excess or rejected independent coverage review. Do not run the
nine frozen selectors, the completed diagnostic chain, an authentic configured
CLI conflict selector or F3. Do not merge a partial stack or push the feature
branch.
