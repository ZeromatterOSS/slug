# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-link-build-variables-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 392-line rules_cc
`cc/private/link/link_build_variables.bzl` producer over its two accepted
children. Prove its complete imported/struct/dictionary/function surface
without invocation.

## Learned facts and decision

Base implementation commit is `955e2204f` (`Prove complete C++ link values
freeze`). It byte-verifies all 363 library-to-link-value lines over its actual
complete child and proves exact child mapping, three imported identities, the
six-field private type struct, three private provider identities, five function
types/visibility and exact six-public/twelve-all inventories without
invocation. Focused, all 264 loading-library, 24/31 integration, locked
analysis/core, CLI, format/diff/source and archive-baseline gates pass within
0/498/498; independent review returned `ACCEPT`.

The 469-line `finalize_link_action.bzl` source-order audit now passes accepted
semantics, `cc_internal`, linkstamp, solib-directory, and library-to-link-value
children, then reaches rules_cc 0.2.17
`cc/private/link/link_build_variables.bzl`, 392 lines, SHA-256
`bdf030361c5a199f6c0fd1bbe5e3b1ce68d041141626a6b0242639b13eab33f0`.
Its only children, complete `cc_helper_internal.bzl` and `cc_internal.bzl`, are
accepted.

The module retains three public and one private imported identity, public
24-field `LINK_BUILD_VARIABLES`, private four-entry
`_DONT_GENERATE_INTERFACE_LIBRARY`, four public lazy functions and one private
lazy function. It invokes no top-level function and constructs no provider or
action value.

Run only `WP-4-7A-rules-cc-link-build-variables-complete-loading-proof`. Do not
invoke a function, inspect a callable default, construct toolchain variables or
claim a consumer.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Source bytes plus
accepted complete-child, struct, dictionary and lazy-function regressions cover
every top-level binding shape; invocation-oriented upstream tests are skipped
because toolchain/configuration/action behavior remains deferred. Clean
`../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept-only
peer guidance for defining-module ownership and retained frozen values. Copy no
Zig code, representation, algorithm, diagnostic or behavior.

- **Exact:** complete source/hash/owner/mapping; both child labels/mappings;
  four imported pointer identities/visibility; 24 `LINK_BUILD_VARIABLES`
  name/value mappings; four `_DONT_GENERATE_INTERFACE_LIBRARY` key/value
  mappings; four public plus one private function types/visibility; exact public
  and all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining both child heaps and owning its struct, dictionary and
  functions; struct/dictionary iteration order rather than an exact Bazel order
  claim.
- **Unsupported/deferred:** every function invocation, signature/default-value
  inspection and result; feature/toolchain/configuration behavior,
  `CcToolchainVariables`, LTO/path/link-variable semantics; finalizer/link
  action; private/public `cc_common`, configured C++, actions and execution.

The frozen defining module is the natural producer and retained owner. No
evaluator borrow or invocation value escapes. Request/revision, DICE,
filesystem, cache, async and fallback concerns are inapplicable because this is
test-only source freezing with no production or retained-service change.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current and Stage 4 documents may change only after terminal acceptance to roll
the result and next packet.

At base `955e2204f` the Rust test authority is 21,767 lines, SHA-256
`40f9b64c0f57adc1c79171658fbc1cca1f8a7ac2eafed907e8f7471ddd47558d`.
Its final ceiling is 22,467 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 700 proof and 700 total additions; deletions do not buy
budget. Embed/hash all 392 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/link:link_build_variables.bzl`, path
`/rules_cc/cc/private/link/link_build_variables.bzl`, with empty mapping and the
accepted children at their actual defining identities. Prove
`//cc/common:cc_helper_internal.bzl` has exact
`bazel_skylib -> bazel_skylib+` mapping and `//cc/private:cc_internal.bzl` has
empty mapping.

Prove public `get_relative_path`, `should_create_per_object_debug_info` and
`artifact_category` pointer-identical to child exports `get_relative_path`,
`should_create_per_object_debug_info` and `artifact_category_names`; prove
private `_cc_internal` pointer-identical to `cc_internal` and not publicly
visible.

Prove public `LINK_BUILD_VARIABLES` has exactly these named string mappings,
without claiming iteration order:

`OUTPUT_EXECPATH=output_execpath`,
`LIBRARIES_TO_LINK=libraries_to_link`,
`RUNTIME_LIBRARY_SEARCH_DIRECTORIES=runtime_library_search_directories`,
`LIBRARY_SEARCH_DIRECTORIES=library_search_directories`,
`RUNTIME_SOLIB_NAME=runtime_solib_name`,
`GENERATE_INTERFACE_LIBRARY=generate_interface_library`,
`INTERFACE_LIBRARY_BUILDER=interface_library_builder_path`,
`INTERFACE_LIBRARY_INPUT=interface_library_input_path`,
`INTERFACE_LIBRARY_OUTPUT=interface_library_output_path`,
`USER_LINK_FLAGS=user_link_flags`, `FORCE_PIC=force_pic`,
`STRIP_DEBUG_SYMBOLS=strip_debug_symbols`, `IS_CC_TEST=is_cc_test`,
`IS_USING_FISSION=is_using_fission`,
`LINKER_PARAM_FILE=linker_param_file`,
`THINLTO_PARAM_FILE=thinlto_param_file`,
`THINLTO_OPTIONAL_PARAMS_FILE=thinlto_optional_params_file`,
`THINLTO_INDEXING_PARAM_FILE=thinlto_indexing_param_file`,
`THINLTO_PREFIX_REPLACE=thinlto_prefix_replace`,
`THINLTO_OBJECT_SUFFIX_REPLACE=thinlto_object_suffix_replace`,
`THINLTO_MERGED_OBJECT_FILE=thinlto_merged_object_file`,
`FDO_INSTRUMENT_PATH=fdo_instrument_path`,
`CS_FDO_INSTRUMENT_PATH=cs_fdo_instrument_path`, and
`PROPELLER_OPTIMIZE_LD_PATH=propeller_optimize_ld_path`.

Prove private `_DONT_GENERATE_INTERFACE_LIBRARY` has exactly the four keys
selected by `GENERATE_INTERFACE_LIBRARY`, `INTERFACE_LIBRARY_BUILDER`,
`INTERFACE_LIBRARY_INPUT`, and `INTERFACE_LIBRARY_OUTPUT`, mapped respectively
to `no`, `ignored`, `ignored`, and `ignored`, by key rather than iteration
order. Prove public `create_link_variables`,
`setup_common_linking_variables`, `setup_linking_variables`, and
`setup_lto_indexing_variables`, plus private `_remove_pie`, are type `function`
with exact visibility. Assert exact eight-public/eleven-all name sets. Invoke
nothing and add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete child/import/struct/dictionary/function inventory, defining
identities, no-invocation/consumer boundary, compatibility split, recursive
branch selection and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, invocation, callable-default inspection, lost imported/eager
identity, evaluator-borrowed value, consumer claim, unpinned source, copied
Zabel content, dirty authority, allowlist escape or cap/function violation.
Stop after this producer and re-audit `finalize_link_action.bzl`.

## Immediate predecessor

Commit `955e2204f` accepts only complete library-to-link-value defining-module
freezing. `6833c72de` accepts only solib directories and `cb71a302d` accepts
only the universal environment and bounded set subset. None accepts link-build
variable function behavior, any link consumer, configured C++ or actions.
