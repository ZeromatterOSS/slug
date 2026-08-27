# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-toolchain-config-info-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 143-line rules_cc
`cc/private/toolchain_config/cc_toolchain_config_info.bzl` module over its
three accepted children. Prove every imported identity, initialized-provider
identity and complete function/export inventory without invocation.

## Learned facts and decision

Commit `1a3f543e2` byte-verifies and freezes all 1,387 authenticated
`legacy_features.bzl` lines over accepted action-name and toolchain-config
library children. It proves ten imported pointer identities, three public plus
one private lazy functions and exact thirteen-public/fourteen-all inventories
without invocation. Focused 1/1, all 285 loading-library, 25/25 invalidation
and 32/32 BUILD-loading tests, locked analysis/core checks and CLI build,
format/diff/source and archive-baseline gates pass within 0/1,495/1,495; root
review returned `ACCEPT`.

Rules_cc 0.2.17
`cc/private/toolchain_config/cc_toolchain_config_info.bzl` is 143 lines,
SHA-256 `8c522773214e202b426ae43589f59a8bdbf3af19d2e595ba8ec7ac125fef5d39`.
Its three children are accepted complete Skylib `paths.bzl`, rules_cc
`cc_internal.bzl` and `legacy_features.bzl`. Top-level evaluation binds five
imports, defines private `_init`, creates public initialized provider
`CcToolchainConfigInfo` with private raw constructor, and defines public
`create_cc_toolchain_config_info`. No imported or source-defined function is
invoked during defining-module evaluation.

Run only `WP-4-7A-rules-cc-toolchain-config-info-complete-loading-proof`.
Add a complete defining-module regression. Do not call the provider, raw
constructor, `_init`, `create_cc_toolchain_config_info`, any legacy helper, or
any `cc_common`, toolchain, configuration, rule or action consumer.

## Generic architecture, authorities and compatibility

This remains generic Starlark loading/evaluation of BCR-owned rule sources,
not a C++ parser or native toolchain implementation. Slug's Buck2-derived Rust
Starlark parser and general frozen-module/provider infrastructure evaluate the
complete source at its real owner and retain actual child exports.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc 0.2.17 bytes are sole exact authority.
`CcToolchainConfigInfo` source and Bazel's Starlark toolchain tests establish
the defining identity and configured consumer; configured tests are skipped
because they invoke provider/toolchain behavior from a later unsupported
phase. Existing initialized-provider regressions cover the generic callable
shape. Add no fixture or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its producer-module/exported-name provider
identity and frozen defining-module ownership support the same boundary, but
its Zig provider implementation, C++ primitives, representations and behavior
are not copied and are not compatibility authority.

- **Exact:** complete source bytes/hash/line count; canonical owner and actual
  `bazel_skylib -> bazel_skylib+` mapping; exact three child identities;
  five imported pointer identities and visibility; public
  `CcToolchainConfigInfo` callable with exact defining identity; public
  constructor function, private initializer and raw constructor; exact
  six-public/nine-all name inventories; complete freeze without invocation.
- **Slug-native:** starlark-rust parse/freeze, initialized-provider and test
  representations; one frozen module retaining three accepted child heaps.
- **Unsupported/deferred:** provider/raw/initializer/constructor or imported
  function invocation; provider instances, fields, feature/action configs,
  paths and diagnostics; `configure_features.bzl`, private/public `cc_common`,
  C++ configuration/toolchain/rule/action, ActionKey and execution behavior.

The frozen defining module is the natural producer and retained owner. Loaded
bindings remain pointer-identical to child exports, while provider/function
identities remain defining-module-owned. No evaluator borrow or invocation
value escapes. Request/revision, DICE, filesystem, cache, async and fallback
concerns are inapplicable because this is test-only proof over existing owners.
There is no temporary fallback.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling
documents may change only after terminal acceptance to roll the result and
successor.

At base `1a3f543e2`, the Rust test authority is 27,455 lines, SHA-256
`9ea3021334b193c982bd061101685f2f3f88c80b952bcd9e7db3bd76331b3667`.
Its final ceiling is 27,805 lines. Each new proof/helper function must remain
at most 120 physical lines. The oversized test module remains cohesive as the
sole private loading harness and authenticated rules_cc source-proof ledger;
add no production responsibility or generic source archive.

Caps are 0 production, 350 proof and 350 total additions; deletions do not buy
budget. Embed and hash all 143 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/toolchain_config:cc_toolchain_config_info.bzl`, path
`/rules_cc/cc/private/toolchain_config/cc_toolchain_config_info.bzl`, with
`bazel_skylib -> bazel_skylib+` mapping and all three actual loads.

Build accepted children at `@@bazel_skylib+//lib:paths.bzl`,
`@@rules_cc+//cc/private:cc_internal.bzl`, and
`@@rules_cc+//cc/private/toolchain_config:legacy_features.bzl`. Prove `paths`,
private `_cc_internal`, and public `get_features_to_appear_last`,
`get_legacy_action_configs`, `get_legacy_features` pointer-identical to the
actual child exports with correct public/private lookup. Prove
`CcToolchainConfigInfo` type and exact printable defining identity, private raw
constructor `_new_cc_toolchain_config_info`, private `_init`, and public
`create_cc_toolchain_config_info` function types. Assert exact
six-public/nine-all name sets. Invoke nothing and inspect no callable defaults
or provider-instance fields.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceiling/function sizes and perform root
review of bytes, child/import/provider/function inventories, identities,
no-invocation boundary, generic architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, missing parser/
global/evaluator/provider shape, copied or narrowed source/child, incomplete
binding coverage, invocation/default/output inspection, lost identity,
evaluator-borrowed value, C++ semantic/consumer claim, unpinned source, copied
Zabel content, dirty authority, allowlist escape, or cap/function violation.
Stop after this parent and re-audit complete `configure_features.bzl`.

## Immediate predecessor

Commit `1a3f543e2` accepts only complete `legacy_features.bzl` defining-module
freezing. `7bb8d670f` accepts only complete `cc_toolchain_info.bzl` freezing.
Neither accepts toolchain-config provider instances, function behavior,
private `cc_common`, configured rules or actions.
