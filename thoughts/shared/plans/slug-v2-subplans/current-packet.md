# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-public-cc-info-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 18-line rules_cc
`cc/common/cc_info.bzl` wrapper over the accepted complete compatibility
symbols child. Prove exact re-export identity and inventory without invocation.

## Learned facts and decision

Commit `be1562848` closes the first direct dependency of rules_rust
`rust/private/toolchain.bzl`: the complete public `cc_common` wrapper now
freezes over the complete generated compatibility module. The rules_rust source
next loads public `CcInfo`; its first `cc_common` method calls occur only inside
the lazy `_rust_toolchain_impl` body and are not part of top-level module
evaluation.

Rules_cc 0.2.17 `cc/common/cc_info.bzl` is 18 lines, SHA-256
`bac2bc3024fb0bacdfa2ca8d7ac3af946f447fe397c76b29fea959a35271f3da`.
It loads only `CcInfo` from the already-complete generated compatibility
`symbols.bzl` under private alias `_CcInfo`, then publicly assigns the same
value. That symbols module already freezes the complete private `cc_info.bzl`
producer and all five other children.

Run only `WP-4-7A-rules-cc-public-cc-info-complete-loading-proof`. Do not invoke
the provider, inspect its initializer, evaluate a rules_rust implementation,
invoke a `cc_common` field, or configure any toolchain or action.

## Generic architecture, authorities and compatibility

This is generic BCR Starlark loading/re-export, not a C++ parser or Rust rules
implementation. Slug's Buck2-derived Rust Starlark evaluator freezes the exact
BCR wrapper at its canonical owner and retains the generated child's value.
It advances a concrete rules_rust dependency while preserving the architecture
needed for all BCR-defined rules and builtins.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc 0.2.17 bytes are sole exact authority. The complete
compatibility-symbols child is accepted; add no fixture or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its producer-owned re-export identity supports
this boundary, but no Zig code, representation, algorithm or C++ behavior is
copied and Zabel is not compatibility authority.

- **Exact:** complete 18-line source/hash; canonical owner and
  `cc_compatibility_proxy` mapping; exact child owner and `rules_cc` mapping;
  private import and public export pointer identity; exact one-public/two-all
  inventories; complete freeze without invocation.
- **Slug-native:** starlark-rust parse/evaluate/freeze and test representation;
  the wrapper retains the accepted generated child heap.
- **Unsupported/deferred:** provider construction/invocation outputs; every
  façade method; rules_rust implementation bodies; configured C++/Rust rules,
  rule contexts, configurations, features, toolchains, actions, ActionKeys and
  execution.

No evaluator borrow or invocation result escapes. DICE, request/revision,
filesystem, cache, async and fallback concerns are inapplicable to this
test-only proof. There is no fallback.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling
documents may change only after terminal acceptance.

At base `be1562848`, the Rust test authority is 29,705 lines, SHA-256
`777da4bb96c34417c9f207c72523b509972f688c700395188cf7ae1d1a2eaae4`.
Its final ceiling is 29,955 lines. Each new proof/helper function must remain at
most 120 physical lines. Add no production responsibility or generic archive.

Caps are 0 production, 250 proof and 250 total additions; deletions do not buy
budget. Embed/hash all 18 authenticated lines. Evaluate at owner
`@@rules_cc+//cc/common:cc_info.bzl`, path
`/rules_cc/cc/common/cc_info.bzl`, mapping
`cc_compatibility_proxy -> rules_cc++compatibility_proxy+cc_compatibility_proxy`.
Build the actual complete symbols child at its accepted owner/mapping. Prove
private `_CcInfo`, public `CcInfo`, pointer identity through both layers, and
exact one-public/two-all inventories. Invoke nothing.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/function sizes and perform root review of
source, child/import/export identity, inventories, no-invocation scope, generic
architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, missing parser/
global/evaluator shape, incomplete/copied child, missing identity/inventory
coverage, invocation/output inspection, evaluator-borrowed value, C++ or Rust
semantic claim, unpinned source, copied Zabel content, dirty authority,
allowlist escape, or cap/function violation. Stop after this wrapper and resume
the authenticated rules_rust toolchain source-order audit before admitting any
host capability call.

## Immediate predecessor

Commit `be1562848` accepts only the complete public `cc_common` wrapper.
`324de9474` accepts the complete generated compatibility symbols module. Neither
accepts this public `CcInfo` wrapper, provider behavior, configured rules or
actions.
