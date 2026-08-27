# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-finalize-link-action-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 469-line rules_cc
`cc/private/link/finalize_link_action.bzl` producer over its eight accepted
children. Prove its complete imported/function surface without invocation.

## Learned facts and decision

Base implementation commit is `3b82f098c` (`Prove complete C++ link build
variables freeze`). It byte-verifies all 392 link-build-variable lines over two
actual complete children and proves exact child mappings, four imported
identities, all 24 named struct mappings, all four named dictionary mappings,
five function types/visibility and exact eight-public/eleven-all inventories
without invocation. Focused, all 265 loading-library, 24/31 integration, locked
analysis/core, CLI, format/diff/source and archive-baseline gates pass within
0/530/530; independent review returned `ACCEPT`.

The recursive 469-line `finalize_link_action.bzl`, SHA-256
`adc6ea3b355d0c5e5fbf1b1e9eaa7d7dd7c0c095234a0cff7fdb4fc72eb167c9`,
now has all eight source-order children accepted: semantics, `cc_internal`,
linkstamp compile, solib directories, library-to-link values, link-build
variables, target types and native `cc_common`.

The module retains twelve public and two private imported identities, one
public lazy function and five private lazy functions. It has no other top-level
binding shape and invokes nothing at top level.

Run only `WP-4-7A-rules-cc-finalize-link-action-complete-loading-proof`. Do not
invoke a function, inspect a callable default, create a link/LTO action or claim
a consumer.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Source bytes plus
accepted complete-child and lazy-function regressions cover every top-level
binding shape; invocation-oriented upstream tests are skipped because all
configured toolchain/action behavior remains deferred. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept-only peer guidance for
defining-module ownership and retained frozen values. Copy no Zig code,
representation, algorithm, diagnostic or behavior.

- **Exact:** complete source/hash/owner/mapping; all eight child labels and
  mappings; fourteen imported pointer identities/visibility; one public plus
  five private function types/visibility; exact public and all-visibility name
  sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining all eight child heaps and owning its functions.
- **Unsupported/deferred:** every function invocation, signature/default-value
  inspection and result; link/LTO/linkstamp variables, inputs, depsets,
  toolchain/configuration/path behavior; action registration, ActionKey and
  execution; private/public `cc_common` and configured C++ consumers.

The frozen defining module is the natural producer and retained owner. No
evaluator borrow or invocation value escapes. Request/revision, DICE,
filesystem, cache, async and fallback concerns are inapplicable because this is
test-only source freezing with no production or retained-service change.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current and Stage 4 documents may change only after terminal acceptance to roll
the result and next packet.

At base `3b82f098c` the Rust test authority is 22,297 lines, SHA-256
`23b7e8c61ff16b7863aac24ef62d10e3af18d9b7440a24c58c9bb20c82507d0e`.
Its final ceiling is 23,097 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 800 proof and 800 total additions; deletions do not buy
budget. Embed/hash all 469 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/link:finalize_link_action.bzl`, path
`/rules_cc/cc/private/link/finalize_link_action.bzl`, with empty mapping and all
accepted children at their actual defining identities.

Prove exact child mappings: `semantics.bzl` has
`platforms -> platforms+`; `collect_solib_dirs.bzl` has
`bazel_skylib -> bazel_skylib+`; `cc_internal.bzl`,
`linkstamp_compile.bzl`, `create_libraries_to_link_values.bzl`,
`link_build_variables.bzl`, `target_types.bzl`, and `native_cc_common.bzl` have
empty mappings.

Prove these public imports pointer-identical to their actual child exports:
`semantics`, `register_linkstamp_compile_action`, `collect_solib_dirs`,
`add_libraries_to_link`, `add_object_files_to_link`,
`process_objects_for_lto`, `setup_common_linking_variables`, `LINKING_MODE`,
`LINK_TARGET_TYPE`, `USE_ARCHIVER`, `USE_LINKER`, and
`is_dynamic_library`. Prove private `_cc_internal` pointer-identical to
`cc_internal`, private `_cc_common_internal` pointer-identical to
`native_cc_common`, and both absent from public visibility.

Prove public `finalize_link_action` and private `_create_action`,
`_can_split_command_line`, `_need_whole_archive`, `_quote_replacement`, and
`_resource_set` are type `function` with exact visibility. Assert exact
thirteen-public/twenty-all name sets. Invoke nothing and add no fixture/oracle.

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

Commit `3b82f098c` accepts only complete link-build-variable defining-module
freezing. `955e2204f` accepts only library-to-link values and `cb71a302d`
accepts only the universal environment and bounded set subset. None accepts
link-finalizer function behavior, any link consumer, configured C++ or actions.
