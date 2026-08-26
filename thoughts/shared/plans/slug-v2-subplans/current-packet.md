# Current Slug V2 Packet

Packet: `WP-4-7A-rustfmt-test-aspect-provides-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: docs-only loading/aspect-provider architecture audit
Base: `275e0b24`

Result: audit accepted rules_rust 0.73.0
`rust/private/rustfmt.bzl:194-216`, identify the exact first unsupported
expression after the accepted second rustfmt aspect, and select one bounded
declaration-only implementation packet or `REPLAN`. Make no Rust, fixture,
oracle or behavior change.

## Accepted starting point and source order

Commit `275e0b24` freezes `rustfmt_aspect` with its two fixed private Label
schemas and complete `rustfmt_srcs_aspect` producer edge. Both implementations
remain lazy and no aspect is applied.

Source order then reaches `RustfmtTestInfo = provider(...)` at lines 194-200,
whose documented two-field provider declaration is already accepted.
`_RUSTFMT_OUTPUT_GROUPS` at line 202 is an ordinary string list, and the two
implementation functions at lines 204-208 remain lazy. Evaluation reaches
`_rustfmt_test_aspect = aspect(...)` at lines 210-216. Its implementation,
three ordered `attr_aspects`, one exported `requires = [rustfmt_aspect]` edge
and documentation fit accepted surfaces. `provides = [RustfmtTestInfo]` at
line 214 is the first unknown argument. This audit must verify that order and
the flat advertised-provider identity against the live evaluator.

## Required authorities

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
The accepted rules_rust archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Use only those pinned objects and archive.

Inspect at minimum:

- `StarlarkRuleFunctionsApi.aspect`'s `provides` parameter contract;
- `StarlarkRuleClassFunctions.aspect`,
  `StarlarkAttrModule.getStarlarkProviderIdentifiers` and their handling of
  exported/unexported, native, duplicate and invalid provider values;
- `StarlarkDefinedAspect` retention, equality/hash, export identity and later
  transfer into advertised providers;
- focused `StarlarkRuleClassFunctionsTest` and
  `StarlarkDefinedAspectsTest` coverage for advertised providers and provider
  identity; and
- Slug's existing `ProviderId`, transient/frozen provider callable,
  `AspectDefinitionGen`/`FrozenAspectDefinition`, required-provider
  conversion and fail-closed configured-analysis boundary.

Pinned Zabel commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Inspect `build_rule_declaration.zig`'s complete `AspectDefinition.provides`,
provider export ownership and distinct `AspectExportIdentity`, plus only
directly relevant advertised-provider consumers. Determine whether Slug can
reuse `ProviderId` and its frozen aspect lifetime without a side registry or
consumer rebinding. Do not copy Zig code, representation, evaluator behavior,
cache, analysis algorithm or compatibility conclusions; Bazel remains sole
behavior authority.

## Audit questions and compatibility classification

Answer all of these before selecting implementation:

1. Does `provides` retain provider constructor objects, exported provider
   identifiers or another projection, and at what phase is each validated?
2. Does the imported or same-module provider keep its first-export producer
   module/name, and are duplicate/order semantics observable at declaration?
3. Can the existing frozen aspect owner retain the fixed singleton advertised
   provider without applying the aspect or wiring provider matching?
4. Which invalid values and unsupported breadth must fail during loading, and
   can the fixed call reuse the existing provider-ID projection exactly?
5. What is the first unsupported expression after this aspect if `provides`
   is admitted, and does reaching it require any public or cross-crate consumer?

Classify every selected behavior as **exact**, **Slug-native**, or
**unsupported/deferred**. At minimum, aspect application, advertised-provider
production/matching, required-aspect propagation, configured dependencies,
fragments/toolchains, actions, the later `rustfmt_test` rule, M8/M7B and exact
Bazel configuration/output identity remain deferred unless the audit proves a
smaller declaration-only boundary.

## Ownership, utility and scope gates

Prefer the existing producer-owned `ProviderId`, frozen provider callable,
`AspectDefinitionGen`/`FrozenAspectDefinition`, compact strings and Arc
slices. An importer, consuming aspect/rule, BUILD package or configured-analysis
command must not reconstruct the provider identity. Retained values must freeze
with the recursive Bzl module and borrow no evaluator heap or request scratch.

Read the Buck2 utility ledger only if the selected design changes a retained
collection, identity, hashing, clone cost or memory accounting. Record the
reuse decision in the implementation packet. No DICE key, mapping, source
observer, cache, I/O, interner, global registry, lock or async owner is
authorized by this audit.

## Docs-only allowlist and validation

Only these files may change from base `275e0b24`:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

Validate pinned object existence, accepted archive provenance, source line
order, canonical/manifest packet-ID agreement, `git diff --check`, document
structure and file allowlist. A selected implementation packet must include
exact proof, file hashes/line caps, production/test addition caps, function
caps, serial validation, residual risk and STOP conditions. Independent review
is mandatory before commit.

## STOP / `REPLAN`

STOP and `REPLAN` if a bounded declaration-only design cannot preserve the
advertised provider's producer identity in the existing frozen aspect lifetime;
if the fixed call requires aspect application, provider matching, configured
analysis or actions; if it requires a new DICE key, registry, cache, I/O path,
mapping, interner, hash or lifetime owner; if proof requires a new
oracle/fixture/network request; if behavior depends on Zabel; or if any
Java/JVM component would enter Slug.
