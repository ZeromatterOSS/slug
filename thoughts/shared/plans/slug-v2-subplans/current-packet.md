# Current Slug V2 Packet

Packet: `WP-6-m2-implicit-default-info-decoder-implementation`
Milestone: M2 configured Starlark provider normalization
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: bounded analysis-decoder correction
Evidence: accepted Bazel 9.2 oracle `d4e7e47e`; strict existing provider
collection; independently accepted synthesis design.

Implement Bazel's implicit empty `DefaultInfo` normalization in exactly one
production function. In
`app/slug_analysis_v2/src/starlark_rule.rs::evaluate_loaded_rule`, decode the
returned provider list exactly as today. After successful decoding and before
strict collection construction, append exactly
`ProviderValue::DefaultInfo(slug_build_api_v2::DefaultInfo::empty())` only when
the decoded list contains no `DefaultInfo`. If an explicit default is present,
append nothing. Always call the unchanged strict `ProviderCollection::new`.

This preserves the collection's always-default invariant and makes a
custom-only Starlark rule expose an empty default exactly as the oracle proves.
Do not use permissive `ProviderCollection::from_values(..., false)`, change the
build API, or represent implicit absence. Existing explicit `DefaultInfo`,
declared files, actions, duplicate rejection, direct dependencies, events, and
evaluator ownership remain unchanged.

Exact production allowlist:

- `app/slug_analysis_v2/src/starlark_rule.rs`

Exact test allowlist:

- `app/slug_analysis_v2/tests/starlark_rule.rs`

Caps are 20 formatted production net lines, 120 test lines, and 140 total.
Tests must prove a custom-only rule succeeds, its exported custom provider is
retrievable, `default_info()` is present and empty, and declared outputs and
actions are empty. Preserve and run the existing explicit-default/write-action
regression. Run the existing strict build-API provider test unchanged to prove
generic `ProviderCollection::new` still rejects custom-only input. No new
duplicate-provider failure claim is required because the constructor and its
existing tests remain unchanged.

Run serially: formatting; `slug_build_api_v2 --test providers`; focused and
full `slug_analysis_v2 --test starlark_rule`; configured-target tests; full
analysis; GNU-Windows analysis no-run; archive/diff/scope/cap checks. No daemon
or binary smoke is required.

There is no retained utility change: reuse the existing empty default and
`SmallMap`-backed strict collection. Stop for any build-API/provider-
representation edit, permissive configured-target absence, synthetic nonempty
files/runfiles/executable state, output inference, broader rule-return syntax,
builtin/output-group/aspect/alias/query-provider breadth, failure-diagnostic
claim, outside file, or cap breach.

After acceptance, restore
`WP-6-m2-positive-string-build-setting-transition-implementation` as the
current packet under its previously accepted graph, allowlists, caps, and
public-surface stops.
