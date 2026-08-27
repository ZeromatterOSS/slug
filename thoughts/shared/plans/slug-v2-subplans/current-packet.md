# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-collect-solib-dirs-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 479-line rules_cc
`cc/private/link/collect_solib_dirs.bzl` producer over its three accepted
children. Prove its complete imported/function surface without invocation.

## Learned facts and decision

Base implementation commit is `49e139212` (`Prove complete C++ target types
freeze`). It byte-verifies all 131 target-type lines over two actual complete
children and proves exact child mappings, imported identities, strings,
two-field linking-mode mappings, all named ten-row/six-field target-type
mappings, the public function and exact seven-public/seven-all inventories.
Constructor-order iteration remains explicitly Slug-native rather than an exact
Bazel claim. Focused, all 262 loading-library, 24/31 integration, locked
analysis/core, CLI, format/diff/source and archive-baseline gates pass within
0/283/283; independent review returned `ACCEPT`.

Recursive source order resumes at rules_cc 0.2.17
`cc/private/link/collect_solib_dirs.bzl`, 479 lines, SHA-256
`f25b0f978bce3a3cf810b36c6897a85adefce7036ec68ba53613352afa218125`.
Its three children are now accepted complete producers: Skylib `paths.bzl`,
rules_cc `cc_helper_internal.bzl` and `target_types.bzl`.

The module retains public `paths`, `is_shared_library`, `LINKING_MODE`,
`LINK_TARGET_TYPE`, `is_dynamic_library`, public `collect_solib_dirs`, and six
private lazy functions. It creates no top-level instance, collection, provider,
native result or invoked value.

Run only `WP-4-7A-rules-cc-collect-solib-dirs-complete-loading-proof`. Do not
invoke a function, construct solib paths or claim a consumer.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc/Skylib bytes are sole exact authority. Accepted
complete children and lazy-function regressions cover every eager shape; no
fresh oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is a peer implementation. Its
defining-module ownership and separation from path/action-time values inform
proof architecture only. Copy no Zig code, representation, algorithm,
diagnostic or behavior.

- **Exact:** complete source/hash/owner/mapping; all three child labels/mappings;
  all five imported pointer identities/visibility; one public plus six private
  function types/visibility; exact public and all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining all child heaps and owning seven lazy functions.
- **Unsupported/deferred:** every function invocation and result; solib/rpath
  calculation; artifact/toolchain/path behavior; finalizer/link action;
  private/public `cc_common`, configured C++, actions and execution.

The frozen producer retains child heaps and owns its functions. No evaluator
borrow or invocation value escapes. No production, DICE, request, cache, async,
fixture, oracle, hot-path, fallback or utility-reuse decision is introduced.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current and Stage 4 documents may change only after terminal acceptance to roll
the result and next packet.

At base `49e139212` the Rust test authority is 20,670 lines, SHA-256
`ee06fb61bc4c72e460aef482acec62e652d8da1f1e48267b6bfdb920c21816bc`.
Its final ceiling is 21,420 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 750 proof and 750 total additions; deletions do not buy
budget. Embed/hash all 479 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/link:collect_solib_dirs.bzl`, path
`/rules_cc/cc/private/link/collect_solib_dirs.bzl`, with exact
`bazel_skylib -> bazel_skylib+` mapping and the three accepted complete children
in source order. Reuse their actual defining identities and assert exact labels
and mappings.

Prove public `paths` and `is_shared_library` pointer-identical to their child
exports. Prove public `LINKING_MODE`, `LINK_TARGET_TYPE` and
`is_dynamic_library` pointer-identical to target-types exports. Prove public
`collect_solib_dirs` and private `_collect_solib_dirs_from_libraries`,
`_collect_toolchain_runtime_library_search_directories`,
`_find_potential_solib_parents`, `_find_toolchain_solib_parents`,
`_get_runfiles_repo_name` and `_get_relative` are type `function` with exact
visibility. Assert exact six-public/twelve-all name sets. Invoke nothing and add
no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete child/import/function inventory, defining identities,
no-invocation/consumer boundary, compatibility split, recursive branch
selection and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, invocation, lost imported identity, evaluator-borrowed value, consumer
claim, unpinned source, copied Zabel content, dirty authority, allowlist escape
or cap/function violation. Stop after this producer and re-audit
`finalize_link_action.bzl`.

## Immediate predecessor

Commit `49e139212` accepts only complete link target-type defining-module
freezing. `2c1706e70` accepts only linker input and `cb71a302d` accepts only the
universal environment and bounded set subset. None accepts solib calculation,
any link consumer, configured C++ or actions.
