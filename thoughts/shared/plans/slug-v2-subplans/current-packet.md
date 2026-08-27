# Current Slug V2 Packet

Packet: `WP-4-7A-advertised-provider-family-and-rules-rust-allocator-loading`

Milestone: M7A command/ruleset bootstrap closure.

Result: admit the complete Bazel `rule(provides = [...])` declaration family,
share its normalized provider-ID owner with `aspect(provides = [...])`, and
freeze authenticated rules_rust `rust/private/rust_allocator_libraries.bzl`
over five complete real children without invoking allocator behavior.

## Learned facts and decision

Commit `01920f594` freezes all 120 authenticated lines of
`rust/private/lto.bzl` over the complete utility child. The next direct load of
`rust/private/toolchain.bzl` is `rust/private/rust_allocator_libraries.bzl`.
Its five children are now complete: public rules_cc `cc_common` and `CcInfo`,
rules_rust `utils.bzl`, `common.bzl`, and `providers.bzl`.

Rules_rust 0.73.0 `rust/private/rust_allocator_libraries.bzl` is 302 lines,
SHA-256 `ae4acb50ac6a1b922254a07346d97b4649810d33836f2be4824fd0b7a81e536e`.
Its eager tail first exposes an unsupported generic declaration argument:
`rust_allocator_libraries = rule(provides = [AllocatorLibrariesInfo], ...)`.
The other eager shapes—`attr.label` provider constraints/default, two
toolchain requirements, functions and rule freezing—already have admitted
owners.

Pinned Bazel 9.2 `StarlarkRuleFunctionsApi.PROVIDES_DOC`,
`StarlarkRuleClassFunctions.buildRule`, and
`StarlarkAttrModule.getStarlarkProviderIdentifiers` establish that `provides`
is a list of exported provider constructors, normalized to a provider set, and
retained as the rule's advertised providers. The configured-target check in
`StarlarkRuleConfiguredTargetUtil.checkDeclaredProviders` is a later analysis
consumer. `StarlarkRuleImplementationFunctionsTest` covers invalid elements and
missing advertised user/native providers.

Run only this packet. Parse absent, empty, singleton, multi-provider and
duplicate `provides` lists through one shared helper for rules and aspects;
reject non-lists, non-provider elements and unexported constructors. Retain the
first-seen normalized `ProviderId` slice on the frozen rule definition and every
invoked loading target so it participates in equality/invalidation. Then prove
the complete allocator module over its real children. Do not invoke any source
function, provider, rule, `cc_common` method, toolchain, or configured target.

## Generic architecture, authorities and compatibility

This is one general Starlark advertised-provider capability exercised by a
rules_rust module, not Rust allocator or C++ semantics implemented in Rust.
The producer-owned frozen declaration retains detached provider IDs; package
targets clone the same immutable semantic slice. Future configured analysis
may validate returned providers from that owner without reparsing source or
inventing a side registry.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_rust 0.73.0 bytes are sole exact authority. Reuse the
accepted source evidence and add no fixture or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its producer-owned declaration/value split may
guide lifetime placement, but no Zig code, representation, algorithm, cache,
provider behavior, or allocator behavior is copied and Zabel is not
compatibility authority.

- **Exact:** Bazel list/type/export validation and stable duplicate
  normalization for `rule(provides)` and `aspect(provides)`; complete 302-line
  source/hash; canonical parent/child owners and seven imported identities;
  allocator attribute/rule/toolchain declarations; three functions and exact
  ten-public/twelve-all inventories; complete freeze without invocation.
- **Slug-native:** compact immutable `Arc<[ProviderId]>` storage,
  starlark-rust parse/evaluate/freeze, declaration/test representations and
  internal diagnostics beyond tested Bazel message shape.
- **Unsupported/deferred:** configured-target advertised-provider enforcement;
  native advertised providers; rule inheritance; every allocator function,
  provider, rule and `cc_common` invocation; configured allocator behavior,
  actions, ActionKeys and execution.

The immutable provider slice is DICE-retained semantic memory through the
loaded package. It is published only after module/package freeze, participates
in package equality and invalidation through `StarlarkRuleImplementation`, and
is released with that graph value; it borrows no evaluator heap. No new cache,
interner, async task, request overlay or fallback is introduced. The Buck2
utility review selects the existing deterministic `SmallSet`, `Arc` slice,
`ProviderId`, `Dupe` and `Allocative` patterns; no utility import or Stage 9
ledger revision is needed.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/package.rs` and
`app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling documents may
change only before implementation selection and after terminal acceptance.

At base `01920f594`, the Rust test authority is 32,174 lines, SHA-256
`389628bbe68fae63d3f884832c345c9efee9e6271f83e312926e55f1c22f4495`.
Its final ceiling is 33,024 lines. Each new proof/helper function must remain at
most 120 physical lines. `package.rs` exceeds the complexity trigger but remains
the cohesive owner of `rule()`/`aspect()` loading declarations and package
publication; split no parallel provider registry or semantic side store.

Caps are 100 production, 850 proof and 950 total additions; deletions do not
buy budget. Prove exact advertised-provider acceptance, normalization,
retention, freeze and target equality sensitivity. Embed/hash all 302
authenticated allocator lines. Build its five complete real children at their
canonical owners, evaluate the parent at
`@@rules_rust+//rust/private:rust_allocator_libraries.bzl`, path
`/rules_rust/rust/private/rust_allocator_libraries.bzl`, with rules_cc mapping
and same-package loads. Prove child identity, eager attribute/rule/toolchain
values, function visibility and exact inventories. Invoke nothing.

Run focused declaration and allocator proofs, full `slug_loading_v2` library,
protected BUILD-loading and invalidation integrations, locked
`slug_analysis_v2` and `slug_core_v2` checks, and locked `slug_cli_v2` build.
Run formatting, diff, caps/function-size and archive hygiene, then root review
of source authority, provider ownership/equality, real-child identities,
declarations/inventories, no-invocation scope, generic architecture, Buck2
utility choice and Zabel's peer-guidance role.

STOP and `REPLAN` for source/hash mismatch, incomplete or stubbed child,
provider storage outside the declaration/loaded target, borrowed evaluator
value, lost equality/invalidation input, native-provider admission, configured
validation, production file outside the allowlist, any source function or rule
invocation, Rust/C++ semantic claim, copied Zabel content, unreviewed retained
collection, dirty authority, cap/function violation, or missing evaluator shape.
Stop after this module and resume toolchain source order.

## Immediate predecessor

Commit `01920f594` accepts only complete rules_rust LTO loading. Existing
aspect proofs retain one advertised provider, but deliberately reject empty,
duplicate and multiple lists; rules discard the argument entirely. This packet
replaces that narrow aspect slice with the shared exact category before using
the rule form required by allocator loading.
