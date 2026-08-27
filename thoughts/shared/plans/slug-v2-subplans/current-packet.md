# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-configure-features-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 232-line rules_cc
`cc/private/toolchain_config/configure_features.bzl` module over its two
accepted children. Prove both imported identities, all six ordered action-name
lists and the complete function/export inventory without invocation.

## Learned facts and decision

Commit `c4d19156d` byte-verifies and freezes all 143 authenticated
`cc_toolchain_config_info.bzl` lines over accepted Skylib paths, `cc_internal`
and legacy-features children. It proves five imported identities, exact
`CcToolchainConfigInfo` identity, public constructor, private initializer/raw
constructor and six-public/nine-all inventories without invocation. Focused,
all 286 loading-library, protected integration, locked analysis/core and CLI,
format/diff/source and archive-baseline gates pass within 0/337/337; root
review returned `ACCEPT`.

Private `cc_common.bzl` next loads rules_cc 0.2.17
`cc/private/toolchain_config/configure_features.bzl`, 232 lines, SHA-256
`d950aa9acda68b999c452178f8ccf49860eac910a8c28551c547c3725198b977`.
Its only children are accepted complete `//cc:action_names.bzl` and
`//cc/common:semantics.bzl`. Top-level evaluation binds both exports, eagerly
constructs six ordered lists (`ALL_COMPILE_ACTIONS`, `ALL_LINK_ACTIONS`,
`ALL_ARCHIVE_ACTIONS`, `ALL_OTHER_ACTIONS`, their concatenated
`DEFAULT_ACTION_CONFIGS`, and `OBJC_ACTIONS`), then defines private
`_get_coverage_features` and public `configure_features`. It invokes no
function and creates no configured feature value.

Run only `WP-4-7A-rules-cc-configure-features-complete-loading-proof`. Add a
complete defining-module regression. Do not call either function, inspect a
C++ configuration/toolchain, or evaluate private/public `cc_common`.

## Generic architecture, authorities and compatibility

This is generic Starlark loading/evaluation of BCR-owned rule sources, not a
C++ parser or Rust implementation of feature configuration. Slug's
Buck2-derived Rust Starlark parser and general frozen-module/list infrastructure
evaluate the complete source and retain actual child exports.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc 0.2.17 bytes are sole exact authority. Complete
source, accepted children and ordered frozen lists cover every eager binding.
Bazel/rules_cc configured feature tests are skipped because they invoke
toolchain/configuration behavior from a later unsupported phase. Add no
fixture or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its generic evaluator/Bazel-host split and
producer-owned frozen collection/function lifetime support this boundary, but
its Zig code, C++ primitives, feature algorithms and representations are not
copied and are not compatibility authority.

- **Exact:** complete source bytes/hash/line count; canonical owner and empty
  mapping; exact two child owners/mappings and pointer-identical imports; all
  six frozen list values and source order, including 23-entry default
  concatenation; public/private function visibility; exact nine-public/ten-all
  inventories; complete freeze without invocation.
- **Slug-native:** starlark-rust parse/freeze and list/test representation; one
  frozen module retaining both accepted child heaps.
- **Unsupported/deferred:** either function invocation or output; feature-set
  semantics, deduplication, configuration fragments, diagnostics and ordering
  beyond the six eager source lists; private/public `cc_common`, C++ toolchain,
  rule, action, ActionKey and execution behavior.

The frozen defining module is the natural producer and retained owner. Imports
remain pointer-identical to child exports; lists/functions remain defining-
module-owned. No evaluator borrow or invocation value escapes. Request,
revision, DICE, filesystem, cache, async and fallback concerns are inapplicable
because this is test-only proof over existing owners. There is no fallback.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling
documents may change only after terminal acceptance.

At base `c4d19156d`, the Rust test authority is 27,792 lines, SHA-256
`df3bc9b0e5574e8388fa28a692bc741df0efaa4f7a144e75cd424a60fc94b8ba`.
Its final ceiling is 28,342 lines. Each new proof/helper function must remain
at most 120 physical lines. The oversized test module remains cohesive as the
sole private loading harness and authenticated rules_cc source-proof ledger;
add no production responsibility or generic source archive.

Caps are 0 production, 550 proof and 550 total additions; deletions do not buy
budget. Embed/hash all 232 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/toolchain_config:configure_features.bzl`, path
`/rules_cc/cc/private/toolchain_config/configure_features.bzl`, with empty
owner mapping and actual children at `@@rules_cc+//cc:action_names.bzl` (empty
mapping) and `@@rules_cc+//cc/common:semantics.bzl` (`platforms -> platforms+`).

Prove `ACTION_NAMES` and aliased `cc_semantics` pointer-identical to their child
exports. Prove every eager list contains the exact corresponding action-name
fields in source order: 15 compile, 6 link, 1 archive, 1 other, 23 concatenated
default and 4 ObjC entries. Prove public `configure_features` and private
`_get_coverage_features` function types/visibility. Assert exact
nine-public/ten-all name sets. Invoke nothing and inspect no callable defaults.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/functions and perform root review of
bytes, children/imports, ordered lists, inventories, no-invocation boundary,
generic architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, missing parser/
global/evaluator/list shape, copied/narrowed source/child, incomplete ordered
list or binding coverage, invocation/default/output inspection, lost identity,
evaluator-borrowed value, C++ semantic claim, unpinned source, copied Zabel
content, dirty authority, allowlist escape, or cap/function violation. Stop
after this child and re-audit complete private `cc_common.bzl`.

## Immediate predecessor

Commit `c4d19156d` accepts only complete `cc_toolchain_config_info.bzl`
defining-module freezing. `1a3f543e2` accepts only complete legacy-features
freezing. Neither accepts feature-configuration function behavior, private
`cc_common`, configured rules or actions.
