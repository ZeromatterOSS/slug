# Current Slug V2 Packet

Packet: `WP-4-7A-complete-cc-toolchain-info-freeze-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete dependency-free 255-line rules_cc
`cc/private/rules_impl/cc_toolchain_info.bzl` defining module at its real BCR
owner. Prove its visibility, initialized-provider identity, lazy functions and
complete export inventory without invoking any toolchain behavior.

## Learned facts and decision

Commit `e14652d22` implements the accepted generic Bazel 9.2 default-enabled
`.bzl` load-visibility family. One evaluation-scratch declaration slot
publishes one immutable policy in `FrozenBzlModule`; semantic equality includes
that policy, and one checker rejects denied direct loads before importer
evaluation at all five live Bzl/BUILD composition sites. Focused 9/9, all 283
loading-library, 25/25 invalidation and 32/32 BUILD-loading tests, locked
analysis/core checks and CLI build, format/diff/source and archive-baseline
gates pass within 412/540/952. Independent correction rereview returned
`ACCEPT`.

Rules_cc 0.2.17 `cc/private/rules_impl/cc_toolchain_info.bzl` is 255 lines,
SHA-256 `f19589572147b7dc8f1b16ab96791b7651923c36821aed70868a74bbfce963f5`,
and has no loads. At line 18 it declares `visibility(["//cc/..."])`; lines
20-163 define four lazy functions, including nested functions and a
keyword-only lambda; lines 165-255 eagerly create one documented initialized
provider and its private raw constructor. No toolchain initializer or lazy
function is invoked during defining-module evaluation.

Run only `WP-4-7A-complete-cc-toolchain-info-freeze-proof`. Add a complete
authenticated defining-module regression. Do not invoke `CcToolchainInfo`, its
raw constructor, `_create_cc_toolchain_info`, any nested closure, or any
`cc_common`, toolchain, rule, configured-target or action consumer.

## Generic architecture, authorities and compatibility

This is a generic Starlark loader/evaluator proof using authenticated BCR
rules_cc as a demanding integration corpus. It is not a C++ parser, a native
replacement for the BCR module, or authorization for C++ analysis semantics.
Slug continues to use the Buck2-derived Rust Starlark parser and general frozen
module/provider infrastructure; later low-level Bazel host capabilities remain
a separate Rust-native boundary.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority.
`CcToolchainProvider.RulesCcCcToolchainInfoProvider` and
`BazelCcToolchainInfoProvider` anchor the provider to
`//cc/private/rules_impl:cc_toolchain_info.bzl` and
`@rules_cc+//cc/private/rules_impl:cc_toolchain_info.bzl`, respectively, under
exported name `CcToolchainInfo`. `StarlarkCcCommonTest.testCcToolchainInfoFromStarlark`
is relevant configured-consumer evidence but is deliberately skipped here
because it invokes toolchain/configuration behavior from a later unsupported
phase. Existing accepted initialized-provider and visibility regressions cover
the generic callable and declaration families; add no oracle fixture.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
concept-only peer guidance. Its provider authentication by defining-module
identity plus exported name supports the same ownership boundary, while its
C++ primitives, Zig implementation, representations and behavior are not
copied and are not compatibility authority.

- **Exact:** complete source bytes/hash/line count and real canonical owner;
  normalized `//cc/...` declaring-repository visibility; one public
  `CcToolchainInfo` initialized-provider callable with exact module/export
  identity; four private function bindings plus the private raw constructor;
  exact one-public/six-all visibility inventories; successful complete
  defining-module evaluation and freeze without invocation.
- **Slug-native:** starlark-rust parsing/evaluation, test harness, frozen heap
  and provider representation; reuse of the accepted compact visibility
  policy and generic initialized-provider implementation.
- **Unsupported/deferred:** provider/raw/init or lazy-function invocation;
  constructor arguments, return fields and diagnostics; `cc_common` access;
  C++ toolchain/configuration/rule/configured-target/provider-instance/action,
  ActionKey, execution or BCR consumer behavior.

The frozen defining module is the natural producer and retained owner. No
evaluator borrow or invocation value escapes. Request/revision, DICE,
filesystem, cache, async and fallback concerns are inapplicable because this
packet adds only a source-authenticated test over existing production owners.
There is no temporary fallback.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical
and current scheduling documents may change only after terminal acceptance to
roll the result and next packet.

At base `e14652d22`, the Rust test authority is 25,605 lines, SHA-256
`05a407d7012759f92fb0e127e2d006278932712b744c777ba71dd016fc57eeac`.
Its final ceiling is 26,055 lines. Each new proof/helper function must remain
at most 120 physical lines. The oversized test module remains cohesive as the
sole private loading harness and authenticated rules_cc source-proof ledger;
add no production responsibility or generic source archive.

Caps are 0 production, 450 proof and 450 total additions; deletions do not buy
budget. Embed and hash all 255 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/rules_impl:cc_toolchain_info.bzl`, path
`/rules_cc/cc/private/rules_impl/cc_toolchain_info.bzl`, with empty repository
mapping and no children.

Prove the complete source publishes normalized subtree visibility that admits
`@@rules_cc+//cc`, `@@rules_cc+//cc/private` and deeper `cc` packages while
denying sibling, root and other-repository packages. Prove public
`CcToolchainInfo` has type `provider_callable` and exact printable identity
`provider[@@rules_cc+//cc/private/rules_impl:cc_toolchain_info.bzl%CcToolchainInfo]`.
Prove `_`, `_create_cc_toolchain_info`, `_dynamic_runtime_lib`,
`_needs_pic_for_dynamic_libraries` and `_static_runtime_lib` are private
functions. Assert exact one-public/six-all name sets. Invoke nothing and do not
inspect callable defaults or provider-instance fields.

Run the focused proof, all `slug_loading_v2` library tests,
`bzl_invalidation`, `build_file_loading`, locked analysis/core checks, locked
CLI build, formatting, diff and archive hygiene. Measure caps/ceiling/function
sizes and perform root review of bytes, complete binding/visibility inventory,
real provider identity, no-invocation boundary, generic architecture and
Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, missing parser,
global, evaluator or provider shape, copied or narrowed source, incomplete
binding/visibility coverage, invocation or callable-default inspection, lost
provider identity, evaluator-borrowed value, C++ semantic/consumer claim,
unpinned source, copied Zabel content, dirty authority, allowlist escape, or
cap/function violation. Stop after this defining module and re-audit private
`cc_common.bzl` in source order.

## Immediate predecessor

Commit `e14652d22` accepts only generic default-enabled `.bzl` visibility
capture, retention, equality and five-site direct-edge enforcement. It accepts
no complete `cc_toolchain_info.bzl`, C++ provider instance, toolchain,
configured target, rule, action or execution behavior.
