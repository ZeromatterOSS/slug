# Current Slug V2 Packet

Packet: `WP-6-m2-positive-string-build-setting-transition-implementation`
Milestone: M2 semantic target configuration inputs and transitions
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: bounded internal implementation
Evidence: accepted positive Bazel 9.2 fixture `b12774b9`; accepted semantic
configuration/transition design; implicit empty-default normalization in
`7c6eeae5`; existing root configured-analysis graph.

Implement only the successful root string build-setting and outgoing user-
transition vertical proven by
`tests/v2_oracle/fixtures/string-build-setting-transition/`. Public build and
cquery behavior, command flags, wire requests, output, and diagnostics remain
unchanged. Tests observe typed configuration equality, existing provider
values, and rich key activations; the Bazel Starlark cquery formatter remains
oracle-only.

The implementation boundary is frozen as follows:

1. Add a compact `RootStringSettingValue` and a bounded semantic overlay to
   target `ConfigurationKey`. Effective string value owns equality, ordinary
   derived hashing, and `Allocative`; no flag and an explicit default-
   equivalent value are equal. Preserve the existing opaque `first-build`
   token only as a private legacy base discriminator. Do not derive, parse,
   display, or claim a Bazel checksum, and do not use stable serialization to
   distinguish the new semantic configurations.
2. Keep the analysis graph exclusively in `RootConfiguredTargetAnalysisKey`.
   Its internal input is either the existing resolved `ConfiguredTargetKey` or
   a root string-setting request carrying the requested target's
   `CanonicalLabel` and an optional explicit value. Request mode uses the
   packet-fixed `@@//:setting` label, computes the
   existing `RootPackageLoadKey`, resolves explicit value or loaded
   `build_setting_default`, constructs the semantic configured key, and
   computes resolved root mode. Resolved mode retains the existing root
   package loading, Needs, event, error, recursion, and deduplication owners.
   `ConfiguredTargetAnalysisKey` receives no change and no new activation.
3. Accept exactly the fixture language shape: `config.string(flag = True)`,
   `rule(build_setting = ...)`, one string `build_setting_default`,
   `ctx.build_setting_value`, one implicit default label `//:setting`, and two
   `attr.label(cfg = transition)` declarations. Each user transition has empty
   inputs and only the `//:setting` output and must return exactly one string
   value for it. Retain the frozen callable and semantic metadata under the
   existing loaded-module lifetime and package source fingerprint.
4. Retain named dependency declarations and their shape. `_setting` is a
   direct configured dependency supporting `[SettingInfo]`; `left` and `right`
   are declared-order singleton sequences supporting `[0][ConsumerInfo]`.
   Apply each transition before constructing its child configured key by
   overlaying the parent value. Deduplicate only identical full keys with the
   existing `SmallSet`; the same child label under `left` and `right` remains
   two distinct computations. Join through recursively resolved root keys.
5. Use `CompactString` for retained setting/transition strings, immutable
   `Arc` slices for schemas and values, and existing `SmallMap`/`SmallSet` for
   small ordered association and deduplication. Retained values are
   `Allocative`. Add no duplicated setting label, global interner/cache,
   default `HashMap`/`BTreeMap`, `Hashed`, SHA, or new hasher. No Stage 9
   extraction is required.
6. Focused evidence must prove exact equality/restoration: default equals an
   explicit default-equivalent value; default differs from command; restored
   default equals the original; `left` differs from `right` for the same
   label; an edited left differs while right remains equal; restored left
   equals the original. One retained runtime and rich activation audit must
   prove default-to-command-to-default, unchanged warm reuse, transition edit
   and restoration, BUILD default edit and restoration, exact direct/parent
   provider values, separately counted request and resolved root activations,
   zero legacy `ConfiguredTargetAnalysisKey` activations, zero action
   execution, and zero REAPI reach. Freeze the observed exact identities and
   counts before acceptance; do not guess them.

Exact production allowlist:

- `app/slug_analysis_v2/Cargo.toml`
- `app/slug_analysis_v2/src/key.rs`
- `app/slug_analysis_v2/src/dice.rs`
- `app/slug_analysis_v2/src/starlark_rule.rs`
- `app/slug_loading_v2/src/attrs.rs`
- `app/slug_loading_v2/src/package.rs`
- `app/slug_core_v2/src/runtime/dice.rs`

Exact test allowlist:

- `app/slug_loading_v2/src/host_package_load_tests.rs`
- `app/slug_analysis_v2/tests/starlark_rule.rs`
- the existing test module in `app/slug_core_v2/src/runtime/dice.rs`

Caps are 850 formatted production net lines, 450 test lines, and 1,300 total.
The Cargo change is only the workspace `compact_str` dependency for analysis.
Do not edit CLI, server, commands, fixtures, expected oracle artifacts,
harnesses, the lockfile, or any other file.

Run formatting, focused loading host-package, analysis Starlark-rule, and core
runtime-DICE tests serially; then full loading, analysis, and core suites. Run
GNU-Windows no-run checks for those crates, archive/diff/scope/cap/forbidden-
boundary checks, and rebuild `slug_cli_v2` before any binary smoke. No daemon
smoke is required; clean stale `slugd` before and after any daemon-sensitive
work.

Return `REPLAN` on any general/native option or Bazel-checksum requirement;
multiple/general build settings; transition inputs, settings/attr reads,
multiple outputs, broader split cardinality, or exec/host/repository
transitions; `select`/`config_setting`, platform/toolchain/action/REAPI work;
public cquery flag, provider, Starlark-file, default/label output, or exact
failure-diagnostic work; a second graph/key family; direct filesystem or
global cache state; a lock across DICE/evaluator execution; an outside file;
or a cap breach.
