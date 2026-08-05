# Current Slug V2 Packet

Packet: `WP-6-m2-integrated-toolchain-resolution-context-implementation`
Milestone: M2 successful toolchain/platform selection
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: implement the accepted fixture-bounded real selection and prepared
`ctx.toolchains` vertical
Predecessor: accepted evidence `ed4baf08`, registrations `4a3af8df`, native
loading `6a457406`, rule/provider-symbol loading `1d6106bd`, and reserved
integration design review.

Keep selection inline in `RootConfiguredTargetAnalysisKey`. For a root
Starlark requester with zero requirements, preserve the existing path. For
exactly one requirement, compute the existing root registration anchor and
all required `RootPackageLoadKey` values, unioning Needs before semantic
errors. Canonicalize root-only apparent registrations, validate exact native
type/platform/constraint/toolchain/reference kinds, reject duplicate settings,
and select in execution-platform outer/toolchain inner MODULE order.

Analyze the selected NODEP implementation with the existing root configured
analysis key and unchanged configuration. It must be a leaf Starlark rule with
no ordinary dependencies, requirements, transition/build-setting role,
actions, outputs, or providers beyond builtin ToolchainInfo plus implicit empty
DefaultInfo. Add no DICE key, digest, cache, global, lock, direct file read, or
second source graph.

Add builtin `ProviderValue::ToolchainInfo` with exactly one compact string
marker and builtin-specific ProviderCollection lookup. Phase-gate the existing
loading callable through analysis evaluator state: loading invocation remains
unsupported; analysis accepts no positional arguments and exactly one named
string marker. Add only string `ctx.attr.marker` and a one-entry
`ctx.toolchains` index accepting the exact root-apparent required-type string
and exposing `.marker`. User providers remain distinct.

Production allowlist:

- `app/slug_analysis_v2/Cargo.toml`
- `app/slug_analysis_v2/src/dice.rs`
- `app/slug_analysis_v2/src/starlark_rule.rs`
- `app/slug_build_api_v2/src/providers/mod.rs`
- `app/slug_loading_v2/src/provider.rs`

Test allowlist:

- `app/slug_analysis_v2/tests/starlark_rule.rs`
- `app/slug_build_api_v2/tests/providers.rs`

Caps are 540 formatted production net lines, 700 test lines, and 1,240 total.

Required evidence covers the exact six accepted marker observations; warm,
registration reorder/restoration, marker edit/restoration, BUILD
delete/recreate, A-to-B-to-A equality, root/anchor/package/selected-child event
and activation ownership, no legacy analysis activation, every exact native
kind/reference and leaf-guard failure, builtin/user separation, unchanged
loading-time invocation failure, and zero actions/outputs/manifests.

Stop and return `REPLAN` for optional/multiple types, external repositories or
mapping, patterns, command-line registrations, aliases, host fallback,
target-platform constraints, exec groups, general scalar attrs or
ToolchainInfo fields, non-leaf implementations, public query/cquery/aquery or
diagnostic formatting, actions/execution/REAPI, configuration expansion,
dormant resolver scaffolding, or any new DICE key/cache/global/lock.
