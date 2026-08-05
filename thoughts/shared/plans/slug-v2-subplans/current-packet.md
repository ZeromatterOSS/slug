# Current Slug V2 Packet

Packet: `WP-6-m2-native-toolchain-target-loading-implementation`
Milestone: M2 successful toolchain/platform selection
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: retain the fixture's five native platform/toolchain target classes
Predecessor: accepted ordered root registration retention `4a3af8df` and
accepted serial declaration-loading design.

Implement exactly one `Allocative`, structurally equal
`NativeToolchainTarget` enum behind `PackageTargetKind::NativeToolchain`:

- `ConstraintSetting`;
- `ConstraintValue { constraint_setting: CanonicalLabel }`;
- `Platform { constraint_values: Arc<[CanonicalLabel]> }`;
- `ToolchainType`; and
- `Toolchain { toolchain_type: CanonicalLabel, implementation: CanonicalLabel,
  exec_compatible_with: Arc<[CanonicalLabel]> }`.

Add fixture-bounded BUILD globals for `constraint_setting`, `constraint_value`,
`platform`, `toolchain_type`, and `toolchain`. Accept only the attributes and
string/root-label shapes used by the accepted fixture. Resolve through the
existing BUILD package context; reject patterns and external labels. Preserve
ordinary package target order and duplicate-name behavior plus input list order.
The toolchain implementation is retained as NODEP data and must not enter
ordinary dependencies. Give every subtype its exact fixed native rule
capability name.

Use the existing `RootPackageLoadKey` without a new key, digest, cache, scanner,
or filesystem owner. Tests must prove exact declaration values/order,
capabilities, canonical labels, list order, duplicate-name behavior,
wrong-type/pattern/external/unmodeled rejection, cold/warm semantic reuse,
declaration edit and A→B→A restoration, delete/recreate, sole Host event
ownership, and the unchanged anchor dependency.

Root query graph construction must return one explicit Slug-owned deferred
boundary before projecting any package graph containing the new native targets.
It must never silently omit or partially project them. External loading must
classify them through its existing unsupported-kind boundary.

Production allowlist:

- `app/slug_loading_v2/src/package.rs`
- `app/slug_loading_v2/src/bzl_module.rs`
- `app/slug_query_v2/src/graph.rs`

Test allowlist:

- `app/slug_loading_v2/tests/build_file_loading.rs`
- inline tests in `app/slug_loading_v2/src/host_package_load_tests.rs`
- inline tests in `app/slug_query_v2/src/graph.rs`
- `app/slug_query_v2/tests/loading_query.rs`

Caps are 360 formatted production net lines, 520 test lines, and 880 total.

Stop and return `REPLAN` for Starlark `rule(toolchains=)`, `platform_common`,
ToolchainInfo, registered-target lookup, target existence/kind/provider
validation, duplicate constraint-setting validation, constraint normalization
or resolution, command-line registration, external mapping/materialization,
aliases, host fallback, optional/multiple types, target constraints/settings,
exec groups, public query projection, cquery formatting, Bazel diagnostic
claims, configuration identity, actions, REAPI, a new DICE key/digest/cache/
interner, or process-global state.

After acceptance, implement the separately reviewed frozen rule-requirement and
load-only `platform_common.ToolchainInfo` symbol packet before designing the
integrated real DICE resolution/prepared-context vertical.
