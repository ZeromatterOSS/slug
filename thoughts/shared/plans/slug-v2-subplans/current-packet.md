# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-create-libraries-to-link-values-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 363-line rules_cc
`cc/private/link/create_libraries_to_link_values.bzl` producer over its accepted
child. Prove its complete imported/struct/provider/function surface without
invocation.

## Learned facts and decision

Base implementation commit is `6833c72de` (`Prove complete C++ solib directory
freeze`). It byte-verifies all 479 solib-directory lines over three actual
complete children and proves exact child mappings, all five imported identities,
seven function types/visibility and exact six-public/twelve-all inventories
without invocation. Focused, all 263 loading-library, 24/31 integration, locked
analysis/core, CLI, format/diff/source and archive-baseline gates pass within
0/599/599; independent review returned `ACCEPT`.

The 469-line `finalize_link_action.bzl` source-order audit now passes accepted
semantics, `cc_internal`, linkstamp and solib-directory children, then reaches
rules_cc 0.2.17 `cc/private/link/create_libraries_to_link_values.bzl`, 363
lines, SHA-256
`7d8df512d6b0df2178a2ca9cd30cb36d1a22c96877dd8e69f49bd3cf739a3764`.
Its sole child, complete `cc_helper_internal.bzl`, is accepted.

The module retains three public helper imports, private six-field `_TYPE`,
three private provider callables, three public and two private lazy functions.
It creates no top-level provider instance or invoked value.

Run only
`WP-4-7A-rules-cc-create-libraries-to-link-values-complete-loading-proof`.
Do not invoke a function/provider, construct a library-to-link value or claim a
consumer.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Source bytes plus
accepted complete-child, struct, direct-provider and lazy-function regressions
cover every eager shape; invocation-oriented upstream tests are skipped because
their action/path behavior remains deferred. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept-only peer guidance for
defining-module ownership and provider/value separation. Copy no Zig code,
representation, algorithm, diagnostic or behavior.

- **Exact:** complete source/hash/owner/mapping; child label/mapping; three
  imported pointer identities/visibility; six `_TYPE` name/value mappings;
  three provider source/export identities; three public plus two private
  function types/visibility; exact public and all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining the child heap and owning its struct, providers and functions;
  constructor-order struct iteration rather than an exact Bazel order claim.
- **Unsupported/deferred:** every function/provider invocation and result;
  library-to-link value construction, provider instance fields, LTO/object/path
  behavior; finalizer/link action; private/public `cc_common`, configured C++,
  actions and execution.

The frozen defining module is the natural producer and retained owner. No
evaluator borrow or invocation value escapes. Request/revision, DICE,
filesystem, cache, async and fallback concerns are inapplicable because this is
test-only source freezing with no production or retained-service change.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current and Stage 4 documents may change only after terminal acceptance to roll
the result and next packet.

At base `6833c72de` the Rust test authority is 21,269 lines, SHA-256
`443da95d9947203af24e349a6e9403bffd254d32a18df2eaba5df9a5343dbdfe`.
Its final ceiling is 21,919 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 650 proof and 650 total additions; deletions do not buy
budget. Embed/hash all 363 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/link:create_libraries_to_link_values.bzl`, path
`/rules_cc/cc/private/link/create_libraries_to_link_values.bzl`, with empty
mapping and accepted `//cc/common:cc_helper_internal.bzl` at its actual defining
identity and exact `bazel_skylib -> bazel_skylib+` mapping.

Prove public `is_shared_library`, `is_versioned_shared_library` and
`root_relative_path` pointer-identical to child exports. Prove private `_TYPE`
has exactly `STATIC_LIBRARY`, `DYNAMIC_LIBRARY`, `INTERFACE_LIBRARY`,
`OBJECT_FILE`, `OBJECT_FILE_GROUP`, `VERSIONED_DYNAMIC_LIBRARY` mapped to their
lowercase values by name, with iteration order Slug-native. Prove private
`_NamedLibraryInfo`, `_ObjectFileGroupInfo` and `_VersionedLibraryInfo` have
exact provider source/export identities and are pairwise distinct. Prove public
`add_object_files_to_link`, `add_libraries_to_link`, `process_objects_for_lto`
and private `_add_static_library_to_link`, `_add_dynamic_library_to_link` are
type `function` with exact visibility. Assert exact six-public/twelve-all name
sets. Invoke nothing and add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete child/import/struct/provider/function inventory, defining
identities, no-invocation/consumer boundary, compatibility split, recursive
branch selection and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, invocation, lost imported/eager/provider identity, evaluator-borrowed
value, consumer claim, unpinned source, copied Zabel content, dirty authority,
allowlist escape or cap/function violation. Stop after this producer and
re-audit `finalize_link_action.bzl`.

## Immediate predecessor

Commit `6833c72de` accepts only complete solib-directory defining-module
freezing. `49e139212` accepts only target types and `cb71a302d` accepts only the
universal environment and bounded set subset. None accepts library-value
construction, any link consumer, configured C++ or actions.
