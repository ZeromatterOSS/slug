# Current Slug V2 Packet

Packet: `WP-6-m2-toolchain-rule-provider-loading-implementation`
Milestone: M2 successful toolchain/platform selection
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: retain fixture-bounded rule toolchain requirements and load the
`platform_common.ToolchainInfo` symbol without implementing its provider
Predecessor: accepted native toolchain target loading `6a457406` and the
accepted two-packet declaration-loading design.

Add `required_toolchains: Arc<[CanonicalLabel]>` to both the frozen rule
definition and `StarlarkRuleImplementation`, including structural equality and
one read-only accessor. The field is separate from ordinary rule dependencies
and must never create a package/query dependency edge.

Extend `rule()` with the exact accepted fixture subset for `toolchains`: an
omitted or empty list retains an empty slice, while a one-element list of a
plain string label retains exactly one canonical toolchain-type label. Resolve
that label relative to the package containing the defining `.bzl` file, not
the package instantiating the rule. Preserve canonical identity and input
order. Reject wrong element/container types, patterns, external labels,
duplicates, and more than one requirement without claiming Bazel diagnostic
text.

Bind a frozen `platform_common` namespace in the existing loading globals. Its
`ToolchainInfo` attribute must be the existing analysis-builtin callable shape
so the accepted `defs.bzl` function body can freeze. Calling that symbol during
loading must fail explicitly as an unsupported analysis builtin. Do not add a
ToolchainInfo value, provider identity, returned-provider decoder, or user
provider surrogate.

Tests must load the exact accepted `defs.bzl`/BUILD shape and prove that the
requesting `probe_rule` retains only `@@//:demo_type`, while the implementation
rule retains none. Prove definition-package-relative resolution with a rule
instantiated from another package, structural package equality, warm reuse,
requirement edit and A-to-B-to-A restoration, marker edit/restoration, and
load/freeze success for an uncalled `platform_common.ToolchainInfo`. Prove
wrong shapes, external labels, multiple requirements, and direct loading-time
invocation fail closed.

Production allowlist:

- `app/slug_loading_v2/src/package.rs`

Test allowlist:

- `app/slug_loading_v2/tests/build_file_loading.rs`
- `app/slug_loading_v2/tests/bzl_invalidation.rs`

Caps are 340 formatted production net lines, 310 test lines, and 650 total.

Stop and return `REPLAN` for changes to `provider.rs`, ProviderValue or
ProviderCollection, ToolchainInfo invocation/decoding, selected implementation
analysis, registered-target lookup, target existence/kind/provider validation,
constraint normalization or selection, `ctx.toolchains`, query graph
projection, public commands, configuration identity, actions, REAPI, external
mapping/materialization, optional/multiple required types, exec groups, a new
DICE key/digest/cache/interner, or process-global state.

After acceptance, design one integrated real DICE selection/prepared-context
vertical that consumes the root registration anchor and both accepted loading
values. Do not create a dormant resolver-only key.
