# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-create-library-to-link-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 291-line rules_cc
`cc/private/link/create_library_to_link.bzl` producer over its five accepted
children. Prove its complete imported/eager/provider/function surface without
invocation.

## Learned facts and decision

Base implementation commit is `ccab93d4c` (`Prove complete C++ LTO backends
freeze`). It byte-verifies all 540 LTO-backend lines over four actual complete
children and proves child mappings/native alias, four imported identities, the
public provider, four public plus six private functions and exact
seven-public/fifteen-all inventories without invocation. Focused, all 259
loading-library, 24/31 integration, locked analysis/core, CLI,
format/diff/source and archive gates pass within 0/657/657; independent review
returned `ACCEPT`.

The private `cc_common.bzl` source-order audit returns to rules_cc 0.2.17
`cc/private/link/create_library_to_link.bzl`, 291 lines, SHA-256
`5f57423312f24392f106aeb5959485c4f30c54ee2d8e926a45934de51a2455d1`.
All five loaded children now have accepted complete proofs: Skylib paths,
`cc_helper_internal.bzl`, `cc_internal.bzl`, LTO compilation context and LTO
backends.

The eager surface retains public `paths`, `is_versioned_shared_library`,
`path_contains_up_level_references` and `create_shared_non_lto_artifacts`,
private `_cc_internal` and `_EMPTY_LTO`, private `_warning`, public
`LibraryToLinkInfo`, two public functions and two private functions. It creates
no top-level instance, set, native result or invocation value.

Run only `WP-4-7A-rules-cc-create-library-to-link-complete-loading-proof`. Do
not invoke a function/provider, inspect callable ABI/results, create libraries,
symlinks or LTO backends, or claim private/public `cc_common`, proxy, configured
C++, actions or execution.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc/Skylib bytes are sole exact authority. Accepted
complete-child, provider/string and lazy-function regressions cover every eager
shape; no fresh oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is a peer implementation. Its
defining-module ownership and separation from later link/action values inform
proof architecture only. Copy no Zig code, representation, algorithm,
diagnostic or behavior.

- **Exact:** complete source/hash/owner/mapping; all five child labels/mappings;
  all six imported pointer identities/visibility; exact warning string;
  provider source/export identity; two public and two private function
  types/visibility; exact public and all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining imported child heaps, warning, provider and lazy functions.
- **Unsupported/deferred:** every function/provider invocation and callable
  ABI/result; library/symlink/LTO-backend construction; native C++ methods;
  private/public `cc_common`, proxy, configured C++, actions and execution.

The frozen producer retains child heaps and owns its warning/provider/functions.
No evaluator borrow or invocation value escapes. No production, DICE, request,
cache, async, fixture, oracle, hot-path, fallback or utility-reuse decision is
introduced.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current, Stage 4 and Stage 5 documents may change only after terminal acceptance
to roll the result and next packet.

At base `ccab93d4c` the Rust test authority is 19,782 lines, SHA-256
`62f289bd9311726c672f5c8b695440707c7e41dd4607abd706055c3c6d6da18c`.
Its final ceiling is 20,382 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 600 proof and 600 total additions; deletions do not buy
budget. Embed/hash all 291 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/link:create_library_to_link.bzl` with
`bazel_skylib -> bazel_skylib+` mapping and the five accepted complete children
in source order. Reuse their actual defining identities and assert every child
label/mapping.

Prove all six imports pointer-identical to child exports with exact visibility.
Prove exact `_warning` string. Prove public `LibraryToLinkInfo` has exact
provider source/export identity. Prove public `make_library_to_link` and
`create_library_to_link`, and private `_validate_symlink_path` and
`_validate_extension`, all of type `function`. Assert exact
seven-public/twelve-all name sets. Invoke nothing and add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete imported/eager/provider/function inventory, defining identities,
no-invocation boundary, compatibility split, branch selection and Zabel's
peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, invocation, lost imported/eager/provider identity,
evaluator-borrowed value, consumer claim, unpinned source, copied Zabel content,
dirty authority, allowlist escape or cap/function violation. Stop after this
producer and re-audit private `cc_common` at `create_linker_input.bzl`.

## Immediate predecessor

Commit `ccab93d4c` accepts only complete LTO-backends defining-module freezing.
`78acfe43f` accepts only linkstamp, `d32e2602d` accepts only `compile.bzl`, and
`cb71a302d` accepts only the universal environment and bounded set subset. None
accepts this producer, callable behavior, configured C++ or actions.
