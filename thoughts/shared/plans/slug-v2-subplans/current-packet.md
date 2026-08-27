# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-create-linking-context-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 137-line rules_cc
`cc/private/link/create_linking_context_from_compilation_outputs.bzl` producer
over its five accepted children. Prove its complete imported/function surface
without invocation.

## Learned facts and decision

Commit `233cdf9ef` accepts the complete 675-line `cc_linking_helper.bzl` source
over eight actual complete children. It proves all fourteen imports, all eight
lazy functions and exact twelve-public/twenty-two-all inventories without
invocation. Focused, all 269 loading-library, 24/31 integration, locked
analysis/core, CLI, format/diff/source and archive-baseline gates pass within
0/862/862; independent review returned `ACCEPT`.

Rules_cc 0.2.17 `cc/private/cc_common.bzl` loads two direct consumers of the
linking helper. Source order first reaches complete
`cc/private/link/create_linking_context_from_compilation_outputs.bzl`, 137
lines, SHA-256
`664a461564abd348111d791aa03da0207fe158620d276b6da1936f8abb23be59`;
`link.bzl` follows later. All five children of the selected module are
accepted: CcInfo, `cc_internal`, linking helper, linker-input creator and target
types.

The module retains six public and one private imported identities plus one
public lazy function. It has no other top-level binding shape and invokes
nothing at top level.

Run only `WP-4-7A-rules-cc-create-linking-context-complete-loading-proof`. Do
not invoke the function, inspect a callable default, create a linking context,
register an action or claim the `cc_common.bzl` consumer.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Source bytes plus
accepted complete-child and lazy-function regressions cover every top-level
binding shape; invocation-oriented upstream tests are skipped because their
configured linking-context/action behavior is an unsupported later phase.
Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
concept-only peer guidance: its generic-evaluator/Bazel-host split and
producer-owned frozen module lifetime support this architecture, but no Zig
code, representation, algorithm, diagnostic or behavior may be copied.

- **Exact:** complete source/hash/owner/mapping; all five child labels and
  mappings; seven imported pointer identities/visibility; one public function
  type/visibility; exact public and all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining all five child heaps and owning its function.
- **Unsupported/deferred:** function invocation, signature/default-value
  inspection and result; linking-context inputs, outputs, paths,
  toolchain/configuration behavior; action registration, ActionKey and
  execution; `cc_common.bzl` and configured C++ consumers.

The frozen defining module is the natural producer and retained owner. No
evaluator borrow or invocation value escapes. Request/revision, DICE,
filesystem, cache, async and fallback concerns are inapplicable because this is
test-only source freezing with no production or retained-service change.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current and Stage 4 documents may change only after terminal acceptance to roll
the result and next packet.

At base `233cdf9ef` the Rust test authority is 24,711 lines, SHA-256
`e49335e7dba855a6991f1c9cb3351064399ea2ff83f3feaf0d0381850c1b3e8c`.
Its final ceiling is 25,111 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 400 proof and 400 total additions; deletions do not buy
budget. Embed/hash all 137 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/link:create_linking_context_from_compilation_outputs.bzl`,
path
`/rules_cc/cc/private/link/create_linking_context_from_compilation_outputs.bzl`,
with empty owner mapping and all accepted children at their actual defining
identities.

Prove exact child mappings: `cc_info.bzl` and `cc_linking_helper.bzl` carry
`bazel_skylib -> bazel_skylib+`; `cc_internal.bzl`,
`create_linker_input.bzl` and `target_types.bzl` have empty mappings.

Prove these public imports pointer-identical to their actual child exports:
`create_linking_context`, `merge_linking_contexts`, `create_cc_link_actions`,
`create_linker_input`, `LINKING_MODE` and `LINK_TARGET_TYPE`. Prove private
`_cc_internal` pointer-identical to `cc_internal` and absent publicly.

Prove public `create_linking_context_from_compilation_outputs` is type
`function`. Assert exact seven-public/eight-all name sets. Invoke nothing and
add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete child/import/function inventory, defining identities,
no-invocation/context/action/consumer boundary, compatibility split, recursive
branch selection and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, invocation, callable-default inspection, lost imported identity,
evaluator-borrowed value, linking-context/action/consumer claim, unpinned
source, copied Zabel content, dirty authority, allowlist escape or cap/function
violation. Stop after this producer and re-audit `cc_common.bzl` in source
order.

## Immediate predecessor

Commit `233cdf9ef` accepts only complete C++ linking-helper defining-module
freezing. `99d9289da` accepts only the LTO-indexing action and `cb71a302d`
accepts only the universal environment and bounded set subset. None accepts
linking-context function behavior, `cc_common.bzl`, configured consumers or
actions.
