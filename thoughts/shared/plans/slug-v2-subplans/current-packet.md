# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-cpp-link-action-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 273-line rules_cc
`cc/private/link/cpp_link_action.bzl` producer over its eight accepted children.
Prove its complete imported/function surface without invocation.

## Learned facts and decision

Base implementation commit is `aa797d082` (`Prove complete C++ link finalizer
freeze`). It byte-verifies all 469 finalizer lines over eight actual complete
children and proves exact child mappings, fourteen imported identities, six
function types/visibility and exact thirteen-public/twenty-all inventories
without invocation. Focused, all 266 loading-library, 24/31 integration, locked
analysis/core, CLI, format/diff/source and archive-baseline gates pass within
0/678/678; independent review returned `ACCEPT`.

The first direct consumer is rules_cc 0.2.17
`cc/private/link/cpp_link_action.bzl`, 273 lines, SHA-256
`0cbe9d6b0ce0f6bea5abe1d9783b79435f495ba93bdaf402ad9539513a82223f`.
All eight source-order children are accepted: skylib paths, helper,
`cc_internal`, finalizer, link-build variables, LTO backends, target types and
native `cc_common`.

The module retains nine public and two private imported identities, one public
lazy function and one private lazy function. It has no other top-level binding
shape and invokes nothing at top level.

Run only `WP-4-7A-rules-cc-cpp-link-action-complete-loading-proof`. Do not
invoke a function, inspect a callable default, create a link action or claim a
consumer.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Source bytes plus
accepted complete-child and lazy-function regressions cover every top-level
binding shape; invocation-oriented upstream tests are skipped because their
configured toolchain/action behavior is an unsupported later phase. Clean
`../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept-only
peer guidance for defining-module ownership and retained frozen values. Copy no
Zig code, representation, algorithm, diagnostic or behavior.

- **Exact:** complete source/hash/owner/mapping; all eight child labels and
  mappings; eleven imported pointer identities/visibility; one public plus one
  private function types/visibility; exact public and all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining all eight child heaps and owning its functions.
- **Unsupported/deferred:** every function invocation, signature/default-value
  inspection and result; link/linkstamp/LTO variables, inputs, outputs, paths,
  toolchain/configuration behavior; action registration, ActionKey and
  execution; private/public `cc_common` and configured C++ consumers.

The frozen defining module is the natural producer and retained owner. No
evaluator borrow or invocation value escapes. Request/revision, DICE,
filesystem, cache, async and fallback concerns are inapplicable because this is
test-only source freezing with no production or retained-service change.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current and Stage 4 documents may change only after terminal acceptance to roll
the result and next packet.

At base `aa797d082` the Rust test authority is 22,975 lines, SHA-256
`5879fbaf18c62fa186453ec1c271a9b7c84e1893ed980f10427107e702249f85`.
Its final ceiling is 23,575 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 600 proof and 600 total additions; deletions do not buy
budget. Embed/hash all 273 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/link:cpp_link_action.bzl`, path
`/rules_cc/cc/private/link/cpp_link_action.bzl`, with exact
`bazel_skylib -> bazel_skylib+` mapping and all accepted children at their
actual defining identities.

Prove exact child mappings: skylib `paths.bzl`, `cc_internal.bzl`,
`finalize_link_action.bzl`, `link_build_variables.bzl`, `target_types.bzl`, and
`native_cc_common.bzl` have empty mappings; `cc_helper_internal.bzl` and
`lto_backends.bzl` have `bazel_skylib -> bazel_skylib+`.

Prove these public imports pointer-identical to their actual child exports:
`paths`, `artifact_category` to `artifact_category_names`,
`finalize_link_action`, `setup_linking_variables`,
`create_shared_non_lto_artifacts`, `LINK_TARGET_TYPE`, `USE_ARCHIVER`,
`USE_LINKER`, and `is_dynamic_library`. Prove private `_cc_internal`
pointer-identical to `cc_internal`, private `_cc_common_internal`
pointer-identical to `native_cc_common`, and both absent from public visibility.

Prove public `link_action` and private `_map_linkstamps_to_outputs` are type
`function` with exact visibility. Assert exact ten-public/thirteen-all name
sets. Invoke nothing and add no fixture/oracle.

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
this producer and re-audit its first consumer.

## Immediate predecessor

Commit `aa797d082` accepts only complete link-finalizer defining-module
freezing. `3b82f098c` accepts only link-build variables and `cb71a302d` accepts
only the universal environment and bounded set subset. None accepts link-action
function behavior, any configured C++ consumer or actions.
