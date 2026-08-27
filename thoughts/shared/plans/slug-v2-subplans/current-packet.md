# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-legacy-features-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 1,387-line rules_cc
`cc/private/toolchain_config/legacy_features.bzl` module over its two accepted
children. Prove every imported identity and its complete lazy-function export
inventory without invoking legacy-feature or toolchain behavior.

## Learned facts and decision

Commit `7bb8d670f` byte-verifies and freezes all 255 authenticated
`cc_toolchain_info.bzl` lines at the canonical rules_cc owner. It proves the
normalized `//cc/...` policy, exact `CcToolchainInfo` provider identity, four
private lazy functions, private raw constructor and exact one-public/six-all
inventories without invocation. Focused 1/1, all 284 loading-library, 25/25
invalidation and 32/32 BUILD-loading tests, locked analysis/core checks and CLI
build, format/diff/source and archive-baseline gates pass within 0/356/356;
root review returned `ACCEPT`.

Private `cc_common.bzl` next loads the already accepted 18-line
`native_cc_common.bzl`, then 143-line `cc_toolchain_config_info.bzl`. The
latter's first unaccepted child is rules_cc 0.2.17
`cc/private/toolchain_config/legacy_features.bzl`, 1,387 lines, SHA-256
`9a6cafe57d4f8564b8eea39a41e66e372471f4d3dcb2cb0e52c970d45f7695dd`.
Its only children are accepted complete `//cc:action_names.bzl` and
`//cc:cc_toolchain_config_lib.bzl`. Top-level evaluation binds ten imported
values and defines public `get_legacy_features`, `get_features_to_appear_last`,
`get_legacy_action_configs` plus private `_platform_specific_value`; it invokes
nothing and creates no other value.

Run only `WP-4-7A-rules-cc-legacy-features-complete-loading-proof`. Add a
complete defining-module regression. Do not call any of the four functions,
construct a feature/action config, evaluate `cc_toolchain_config_info.bzl`, or
claim `cc_common`, toolchain, configured-rule or action semantics.

## Generic architecture, authorities and compatibility

This remains generic Starlark loading/evaluation of BCR-owned rule sources,
not a C++ parser or a native copy of the legacy feature tables. The complete
source is parsed by Slug's Buck2-derived Rust Starlark parser and frozen by the
same general module/import-identity infrastructure used for other rulesets.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc 0.2.17 bytes are sole exact authority. The complete
legacy-features source plus accepted complete child modules covers every
top-level binding shape. Bazel/rules_cc toolchain integration tests that call
`create_cc_toolchain_config_info` or consume configured C++ features are
skipped because function invocation, feature construction and configured
analysis are later unsupported phases. Add no fixture or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its general evaluator/Bazel-host split and frozen
defining-module ownership support retaining source-defined functions and
provider identities in their producer module; it has no applicable
legacy-features leaf to reuse. Copy no Zig code, C++ primitive, table,
representation, diagnostic or behavior.

- **Exact:** complete source bytes/hash/line count; canonical owner and empty
  mapping; exact two child labels/owners; ten imported pointer identities and
  public visibility; three public and one private source-defined function;
  exact thirteen-public/fourteen-all name inventories; complete evaluation and
  freeze without invocation.
- **Slug-native:** starlark-rust parse/freeze and test-harness representation;
  one frozen module retaining the two accepted child heaps; no claim about
  callable bytecode, pointer bytes or heap layout.
- **Unsupported/deferred:** all four function invocations and outputs;
  feature/action-config/profile/tool construction, ordering and diagnostics;
  `cc_toolchain_config_info.bzl`, `configure_features.bzl`, private/public
  `cc_common`, C++ configuration/toolchain/rule/provider-instance/action,
  ActionKey, execution and downstream BCR consumer behavior.

The frozen defining module is the natural producer and retained owner. Loaded
bindings remain pointer-identical to their child exports and function values
remain owned by the defining heap. No evaluator borrow or invocation value
escapes. Request/revision, DICE, filesystem, cache, async and fallback concerns
are inapplicable because this packet adds only a source-authenticated test over
existing production owners. There is no temporary fallback.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical
and current scheduling documents may change only after terminal acceptance to
roll the result and next packet.

At base `7bb8d670f`, the Rust test authority is 25,960 lines, SHA-256
`ad9622edc282b868a8150c267d0809919890eb1e745eba1ddaa6e083b93393c5`.
Its final ceiling is 27,560 lines. Each new proof/helper function must remain
at most 120 physical lines. The oversized test module remains cohesive as the
sole private loading harness and authenticated rules_cc source-proof ledger;
add no production responsibility or generic source archive.

Caps are 0 production, 1,600 proof and 1,600 total additions; deletions do not
buy budget. Embed and hash all 1,387 authenticated lines. Evaluate at exact
owner `@@rules_cc+//cc/private/toolchain_config:legacy_features.bzl`, path
`/rules_cc/cc/private/toolchain_config/legacy_features.bzl`, with empty owner
mapping and the two real loads.

Build the action-names child at `@@rules_cc+//cc:action_names.bzl` and the
toolchain-config library child at
`@@rules_cc+//cc:cc_toolchain_config_lib.bzl`, both with empty mappings. Prove
`ACTION_NAMES` is pointer-identical to the first child. Prove `action_config`,
`feature`, `feature_set`, `flag_group`, `flag_set`,
`get_profile_correction_flags`, `tool`, `variable_with_value` and
`with_feature_set` are pointer-identical to the second child. Prove public
`get_legacy_features`, `get_features_to_appear_last` and
`get_legacy_action_configs` and private `_platform_specific_value` have type
`function`, with the private binding absent from public lookup. Assert exact
thirteen-public/fourteen-all name sets. Invoke nothing and inspect no callable
defaults or function body outputs.

Run the focused proof, all `slug_loading_v2` library tests,
`bzl_invalidation`, `build_file_loading`, locked analysis/core checks, locked
CLI build, formatting, diff and archive hygiene. Measure caps/ceiling/function
sizes and perform root review of authenticated bytes, complete child/import and
function inventories, defining identities, no-invocation boundary, generic
architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
parser/global/evaluator shape, copied or narrowed source/child, incomplete
binding coverage, invocation or callable-default/output inspection, lost child
identity, evaluator-borrowed value, C++ semantic/consumer claim, unpinned
source, copied Zabel content, dirty authority, allowlist escape, or cap/function
violation. Stop after this child and re-audit complete
`cc_toolchain_config_info.bzl` separately.

## Immediate predecessor

Commit `7bb8d670f` accepts only complete dependency-free
`cc_toolchain_info.bzl` defining-module freezing. `e14652d22` accepts only
generic default-enabled `.bzl` visibility. Neither accepts legacy-feature
function behavior, toolchain configuration, private `cc_common`, configured
rules or actions.
