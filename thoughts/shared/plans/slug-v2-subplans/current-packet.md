# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-lto-indexing-action-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 288-line rules_cc
`cc/private/link/lto_indexing_action.bzl` producer over its seven accepted
children. Prove its complete imported/function surface without invocation.

## Learned facts and decision

Commit `8daf80a2c` accepts the complete 273-line `cpp_link_action.bzl` source
over eight actual complete children. It proves all eleven imports, both lazy
functions and exact ten-public/thirteen-all inventories without invocation.
Focused, all 267 loading-library, 24/31 integration, locked analysis/core, CLI,
format/diff/source and archive-baseline gates pass within 0/454/454;
independent review returned `ACCEPT`.

The direct parent is rules_cc 0.2.17
`cc/private/link/cc_linking_helper.bzl`. Its source-order children through
`cpp_link_action.bzl` and `create_library_to_link.bzl` are accepted; the first
unresolved child is complete `lto_indexing_action.bzl`, 288 lines, SHA-256
`03cb57e972bb7503d665ca56340a34fff3e6289f9c7a168ca87a427e57c66863`.
All seven of that module's children are accepted: helper, `cc_internal`, LTO
compilation context, link finalizer, link-build variables, LTO backends and
target types.

The module retains eight public and one private imported identities, one public
lazy function and one private lazy function. It has no other top-level binding
shape and invokes nothing at top level.

Run only `WP-4-7A-rules-cc-lto-indexing-action-complete-loading-proof`. Do not
invoke a function, inspect a callable default, register an action or claim the
parent consumer.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Source bytes plus
accepted complete-child and lazy-function regressions cover every top-level
binding shape; invocation-oriented upstream tests are skipped because their
configured toolchain/action behavior is an unsupported later phase. Clean
`../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept-only
peer guidance: its generic-evaluator/Bazel-host split and producer-owned frozen
module lifetime support this architecture, but no Zig code, representation,
algorithm, diagnostic or behavior may be copied.

- **Exact:** complete source/hash/owner/mapping; all seven child labels and
  mappings; nine imported pointer identities/visibility; one public plus one
  private function types/visibility; exact public and all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining all seven child heaps and owning its functions.
- **Unsupported/deferred:** every function invocation, signature/default-value
  inspection and result; LTO/link variables, inputs, outputs, paths,
  toolchain/configuration behavior; action registration, ActionKey and
  execution; `cc_linking_helper.bzl` and configured C++ consumers.

The frozen defining module is the natural producer and retained owner. No
evaluator borrow or invocation value escapes. Request/revision, DICE,
filesystem, cache, async and fallback concerns are inapplicable because this is
test-only source freezing with no production or retained-service change.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current and Stage 4 documents may change only after terminal acceptance to roll
the result and next packet.

At base `8daf80a2c` the Rust test authority is 23,429 lines, SHA-256
`fcf2592b332c98da3ad341212a2a9519a3da88f2f4a9d81fc305ba442338b283`.
Its final ceiling is 24,054 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 625 proof and 625 total additions; deletions do not buy
budget. Embed/hash all 288 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/link:lto_indexing_action.bzl`, path
`/rules_cc/cc/private/link/lto_indexing_action.bzl`, with empty owner mapping
and all accepted children at their actual defining identities.

Prove exact child mappings: `cc_helper_internal.bzl` and `lto_backends.bzl`
carry `bazel_skylib -> bazel_skylib+`; `cc_internal.bzl`,
`lto_compilation_context.bzl`, `finalize_link_action.bzl`,
`link_build_variables.bzl` and `target_types.bzl` have empty mappings.

Prove these public imports pointer-identical to their actual child exports:
`root_relative_path`, `get_minimized_bitcode_or_self`, `finalize_link_action`,
`setup_lto_indexing_variables`, `create_lto_backends`, `LINKING_MODE`,
`LINK_TARGET_TYPE` and `is_dynamic_library`. Prove private `_cc_internal`
pointer-identical to `cc_internal` and absent from public visibility.

Prove public `create_lto_artifacts_and_lto_indexing_action` and private
`_lto_indexing_action` are type `function` with exact visibility. Assert exact
nine-public/eleven-all name sets. Invoke nothing and add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete child/import/function inventory, defining identities,
no-invocation/action/consumer boundary, compatibility split, recursive branch
selection and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, invocation, callable-default inspection, lost imported identity,
evaluator-borrowed value, action/consumer claim, unpinned source, copied Zabel
content, dirty authority, allowlist escape or cap/function violation. Stop after
this producer and re-audit `cc_linking_helper.bzl` in source order.

## Immediate predecessor

Commit `8daf80a2c` accepts only complete C++ link-action defining-module
freezing. `aa797d082` accepts only the link finalizer and `cb71a302d` accepts
only the universal environment and bounded set subset. None accepts LTO-index
function behavior, the parent helper, any configured C++ consumer or actions.
