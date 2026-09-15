# Current Slug V2 Work Packet

Packet: WP-4-6-7A-r2-execution-group-source-invariant-reconcile-r2
Status: Phase B ready; independent architecture review ACCEPT

## Result and acceptance boundary

Reconcile the preserved selected-toolchain request/configured-action-conflict R2
candidate and named/automatic execution-group implementation against pinned
Bazel 9.2 source invariants, correct the bounded mismatches below, and accept
the two owners as one atomic checkpoint through ordinary correctness proofs.
Do not resume the completed invalidation diagnostic chain and do not repeat its
authentic CLI selector.

The preserved stack remains unaccepted on
`integration/selected-request-output-conflict-r2`. Its R2 base is
`f3c90ea46`; execution-group implementation commits are `868d30a0f`,
`7a2a149af`, `7b2fccb76` and `498ea2f49`. Loading prerequisites after that
stack are independently accepted, but none accepts R2 or execution groups.
Current `main` is `4824a0861`. No feature-branch commit may be pushed.

If the corrected stack passes this packet and independent final review, merge it
to `main` atomically, run main checks, commit the acceptance receipt, and push
`main` to the authorized ZeromatterOSS Slug remote. Authentic configured CLI F3
is the next separate packet after activation; it is not an input to this
checkpoint.

Exact behavior is limited to the pinned named/automatic declaration,
resolution, dependency-transition, provider-view, property and action-routing
category plus R2's selected-platform request and configured-action closure
contract. Slug-native behavior remains DICE key shape, immutable configured
owners, closure ordering and typed error storage. Configured aspects, inherited
or built-in test-runner groups, unrelated action families, complete C++/Java
semantics and exact Bazel configuration/output/ActionKey bytes remain deferred.

## Source authority and completed audit

Primary authority is Bazel commit
`8220c6198837d5c13d53fea211cf3282aa12408a` in `/home/wgray/bazel`. Its working
HEAD is different and is not authority. The audit used these pinned objects:

- `packages/DeclaredExecGroup.java:43-149`;
- `skyframe/toolchains/ToolchainContextUtil.java:169-244`;
- `analysis/ExecGroupCollection.java:52-215`;
- `analysis/RuleContext.java:457-476,1069-1142,1216-1229`;
- `analysis/TargetContext.java:92-99`;
- `analysis/starlark/StarlarkActionFactory.java:405-472,740-895`;
- `analysis/starlark/StarlarkRuleContext.java:860-910,979-992`;
- `analysis/starlark/StarlarkExecGroupCollection.java:45-159`;
- `analysis/starlark/StarlarkToolchainContext.java:99-147`;
- `analysis/starlark/StarlarkSubrule.java:434-475`;
- `analysis/AutoExecGroupsTest.java:303-431,488-705,1243-1304` and
  `analysis/StarlarkExecGroupTest.java` named transition, property and action
  tests.

The preserved configured-target owner, independent group resolution,
constraint-carrying resolution key, selected execution configurations,
provider materialization, group property merge, raw/effective platform split,
runfiles union and all-or-nothing publication agree with that source model.
They remain unchanged. The audit found the following bounded mismatches in
normalization and the synchronous action adapter.

### Automatic identity has two source spellings

Bazel stores an automatic runtime group under
`toolchainType.toolchainType().toString()`: a main-repository label is
`//pkg:type`. `exec_group_compatible_with` instead canonicalizes a parsed key
to `getUnambiguousCanonicalForm()`: the same label is `@@//pkg:type`.
The preserved stack uses `CanonicalLabel::to_string()` (`@@//pkg:type`) for
both roles and its public proof expects that spelling.

Keep `ConfiguredExecGroup::Automatic(Arc<CanonicalLabel>)` as the sole
structural identity. Add one projection owned beside that enum for Bazel's
runtime name: strip only the leading `@@` of the main repository and preserve
external canonical names. Use it for `ctx.exec_groups`, retained `ActionSpec`
group names, action-context lookup and target/platform `exec_properties` row
matching. Use label equality, not a stored second string, for constraint
normalization and action routing.

### Constraint keys use the target package context

Pinned `ToolchainContextUtil` treats a valid identifier exclusively as a public
named group. An absent named group fails even if the same text could parse as a
relative toolchain label. A nonidentifier may denote an automatic group only
when automatic policy is active; Bazel parses it with the target package and
the target package's repository mapping, verifies membership in the rule's
default toolchain requirements, then stores its unambiguous canonical form.

Split the preserved shared qualifier helper. `exec_group_compatible_with`
gets the source algorithm above and may not address Default. `exec_properties`
uses exact runtime row names: `default-exec-group`, declared public names and
the runtime automatic projection. Unknown target rows fail. Platform rows are
parsed identically but unknown rows remain unused. An empty property suffix is
retained because pinned `parseExecProperties` accepts it. Property precedence
stays platform-default < platform-group < target-default < target-group.

Pass the already retained `LoadedPackage::runfiles_package()` package identity
and mapping into normalization. This is a borrowed/Arc-backed input to phase
scratch, not a new retained semantic owner. Source/package/mapping changes are
already DICE inputs to the loaded package and configured parent.

### Action toolchain strings use the target package context

Pinned `determineExecGroup` parses every string `toolchain=` value with
`RuleContext.getPackageContext()`, which is the analyzed target's package and
repository mapping. This differs intentionally from string indexing of
`ctx.toolchains`, whose `LabelConverter.forBzlEvaluatingThread` uses the
executing Bzl definition context. The preserved action adapter incorrectly
uses the definition context for both.

Give the action sink the same already retained target package/mapping view.
Parse a `toolchain=` string before automatic-policy routing, so malformed
strings fail even when automatic groups are disabled. Typed Label values remain
unchanged. Do not alter `ctx.toolchains` string lookup.

Pinned root action handling performs subrule overrides first, then lowers the
supported `executable` and `tools` fields, and only then calls
`determineExecGroup`. Preserve that outer order. Parsing `toolchain=` precedes
group existence, public-name and membership checks only inside
`determineExecGroup`; it does not precede executable/tools conversion. A
combined malformed-toolchain/invalid-tools proof must therefore report the
invalid-tools error.

### Root omitted-toolchain provenance preserves Starlark shape

Pinned action code allows an omitted `toolchain` with an executable File when
that File maps to an executable dependency's `FilesToRunProvider`, and with a
`FilesToRunProvider` only when the complete provider equals one from an
executable dependency in the current root scope. A provider or File from
`ctx.toolchains` is unassociated. A top-level `tools=depset(...)` is flattened
and each element is checked for executable-dependency association, so an
associated root executable File may omit `toolchain`. A depset nested in a
tools sequence is opaque and unconditionally invokes the omitted-toolchain
check; with multiple automatic contexts it requires an explicit toolchain or
explicit `None` even when its artifacts came from executable dependencies.

Extend the existing attempt-local `ExecutableArtifactProvenance` with provider
equality lookup; add no retained registry. The root ambiguity classifier must
keep the original Starlark shape long enough to flatten a top-level depset but
classify a sequence-nested depset as opaque, compare direct Files and providers
against root executable dependencies, and remain inactive when automatic mode
has only Default or when `toolchain` is present.

### Explicit group validation preserves source precedence

Pinned `determineExecGroup` parses `toolchain=` first, then verifies that an
explicit group row exists, then applies public identifier validation, then—only
under automatic policy—checks toolchain membership. Thus an existing automatic
runtime name or `default-exec-group` supplied as `exec_group=` fails public-name
validation, while an absent invalid spelling fails as unknown before name
validation. The preserved adapter validates identifier syntax before row
existence.

Lookup explicit strings across the complete collection by runtime name, then
validate public syntax, and finally require `Named` identity. Preserve unknown
group failure without Default fallback and the existing automatic-mode
membership rule. Do not expose automatic groups through explicit
`exec_group=`.

### Subrule action overrides remain closed

Pinned `StarlarkSubrule` permits omitted or explicitly `None` `exec_group` and
rejects only a non-`None` value. It rejects every supplied `toolchain`, including
explicit `None`, and checks `exec_group` before `toolchain`, so a call with both
invalid reports the execution-group error. With configured subrule toolchains
absent, an omitted toolchain is overridden to explicit `None`, selects Default
and never consults provider association for ambiguity. Slug already defers
configured subrule toolchains but currently sends the unmodified request
through the root action adapter.

Carry the existing schema executable bit into subrule configured-attribute
scratch so executable lookup has the correct scope. A matching executable File
must fail before routing with `expected FilesToRunProvider, got File`; the
matching `FilesToRunProvider` form is admitted, but its association does not
control ambiguity. Use `AnalysisActionCallScope::Subrule` in the existing sink
to enforce these conversion and override distinctions and Default selection.
Add no subrule group or toolchain semantics.

## Implementation scope and limits

Production edits are limited to:

- `app/slug_analysis_v2/src/exec_group.rs`;
- `app/slug_analysis_v2/src/execution_groups.rs`;
- `app/slug_analysis_v2/src/dice.rs`;
- `app/slug_analysis_v2/src/starlark_rule.rs`;
- `app/slug_analysis_v2/src/files_to_run_spawn.rs`;
- `app/slug_analysis_v2/src/result.rs` only for the shared runtime-name
  projection.

Proof edits are limited to
`app/slug_analysis_v2/tests/starlark_rule.rs` and unit tests beside the changed
analysis owners. Plan receipts may update this packet, Stage 6, bootstrap
readiness, the canonical plan and configured CLI ledger. No loading,
configuration, Core, CLI, REAPI, server, query, Cargo/BUILD, fixture or Bazel
source may change.

The correction cap is 210 gross production and 520 gross proof Rust additions.
Existing Phase B physical caps become 820 lines for `execution_groups.rs`,
7,150 for `dice.rs`, 2,550 for `starlark_rule.rs` and 13,500 for its integration
test. If the first three files need a new retained owner, if a mapping must be
copied into a new semantic carrier, or if any cap is exceeded, stop and replan.

## Ordinary correctness proofs

Before execution, preflight every exact selector against its produced binary.
Prepare one target at a time with the pinned nightly and shared target directory;
each compile has a 60-second ceiling. Each exact test has a 12-second deadline
and 15-second absolute ceiling. No diagnostic observer, supervisor,
instrumentation, authentic configured CLI conflict replay or F3 is admitted.

Correct and run these execution-group selectors exactly:

- `execution_groups::tests::normalization_preserves_named_constraint_only_and_automatic_groups`;
- `execution_groups::tests::automatic_constraint_keys_use_target_mapping_and_reject_identifier_aliases`;
- `execution_groups::tests::runtime_names_and_property_rows_follow_bazel_canonical_form`;
- `execution_groups::tests::property_precedence_target_default_beats_platform_group`;
- `execution_groups::tests::unknown_target_group_prefixes_fail_and_unknown_platform_prefixes_are_unused`;
- `automatic_exec_group_policy_obeys_attribute_over_flag_aba`;
- `automatic_exec_group_projects_default_labels_from_automatic_rows_and_indexes_all_nondefault_rows`;
- `automatic_exec_group_routes_run_and_run_shell_across_the_complete_parameter_matrix`;
- `automatic_action_toolchain_strings_use_target_package_and_repository_mapping`;
- `automatic_action_group_validation_preserves_existence_and_name_precedence`;
- `subrule_action_group_and_toolchain_parameters_preserve_bazel_override_boundary`;
- `named_exec_group_resolves_independent_toolchains_constraints_and_provider_view`;
- `named_exec_transition_preserves_ordinary_and_configured_rows_uses_selected_platform_and_rejects_unknown`;
- `configured_action_context_distinguishes_default_named_and_automatic_groups`;
- `configured_action_specs_route_every_group_without_fallback`.

The action matrix must discriminate run and run_shell, automatic off/on,
omitted/None/Label/string toolchain, target versus definition package context,
apparent target mapping, valid/unknown/malformed toolchain labels, associated
and unassociated executable Files and FilesToRunProviders, direct tools,
associated and unassociated top-level depsets, opaque sequence-nested depsets,
string executables, valid/unknown/invalid/default/automatic explicit groups,
and compatible/mismatched group-toolchain pairs. A combined malformed
toolchain/invalid-tools call must prove that supported-field lowering precedes
group selection. The subrule matrix must separately prove omitted and explicit
`None` `exec_group`, non-`None` `exec_group`, omitted and explicit `None`
`toolchain`, both-invalid error precedence, rejection of a matching executable
File, admission of its FilesToRunProvider, Default selection without provenance
ambiguity, and continued deferral of configured subrule toolchains.

Re-run these selected-request selectors exactly:

- `selected_toolchain_request_keys_keep_configuration_and_distinct_structural_identity`;
- `selected_toolchain_request_keeps_parent_options_and_nonfirst_platform`;
- `selected_toolchain_request_interleaves_full_keys_and_restores_source_properties`;
- `selected_toolchain_request_clears_ordinary_edges_and_alias_actual`;
- `selected_toolchain_request_preserves_preference_through_incoming_transition`;
- `selected_toolchain_request_selection_priority_fallback_and_alias_lookup`;
- `selected_toolchain_request_known_target_outside_registration_empty_and_optional`;
- `selected_toolchain_request_observed_need_cancellation_and_error_recover`.

Run all exact tests in Core's
`runtime::tests::configured_action_conflicts_tests`. Run the two direct REAPI
selectors
`configured_file_write_reapi_plan_reads_retained_platform_properties` and
`selected_toolchain_request_reapi_payload_is_not_owner_identity`, the direct
server selector
`reapi_materialization_uses_distinct_and_restored_structural_configuration_roots`,
and query/server/CLI compile checks.

The earlier Core reconciliation compiled one library binary, preflighted 329
active selectors, passed 309, and attributed 13 identical assertion failures to
current main. Seven bounded commands hit the common 12-second resource ceiling
on both lines, but their exact membership was not retained as an acceptance
receipt. Replace the complete Core batching receipt: using the compiled final
binary, run all 329 active selectors as separate preflighted exact commands
once. Compare any semantic failure exactly to unchanged current main. A single
exact timeout blocks acceptance and causes a source/test-boundary replan; it
does not authorize more instrumentation or a longer deadline.

The completed diagnostic chain showed that authentic CLI conflict selectors
enter `RootCompute` and exhaust the deadline before their command assertions.
Repeating them cannot establish R2 correctness. Their bounded replacements are
the complete direct configured-action-conflict module, the selected-request
analysis proofs, the direct REAPI/server consumers, CLI compile linkage and a
source review of the two-line CLI closure-consumer call site. This replacement
set and the final evidence require independent acceptance review.

Run pinned `cargo fmt --all -- --check`, `git diff --check`, scope/growth checks,
`python3 scripts/v2_plan_status.py`, clean-worktree checks and independent final
invariant review. Commit the reviewed design before runtime edits, commit the
correction after focused proofs, and commit the acceptance receipt separately.

## Stops

Return `REPLAN` if source comparison reveals another semantic owner or key
family, a correction changes selected-request or FileWrite equivalence, an exact
ordinary proof times out, the target package mapping is unavailable without a
new carrier, or independent review rejects the replacement acceptance boundary.
Do not infer performance work from the completed diagnostics, repeat an
activation/path/invalidation measurement, run an authentic conflict selector,
run F3, merge a partial stack or push the feature branch.
