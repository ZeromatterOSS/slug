# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-target-types-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 131-line rules_cc
`cc/private/link/target_types.bzl` producer over its two accepted children.
Prove its complete imported/string/struct/function surface without invocation.

## Learned facts and decision

Base implementation commit is `2c1706e70` (`Prove complete C++ linker input
freeze`). It byte-verifies all 69 linker-input lines over the actual complete
`cc_internal.bzl` child and proves exact import visibility, the private provider,
the public function and exact one-public/three-all inventories without
invocation or callable-default inspection. Focused, all 261 loading-library,
24/31 integration, locked analysis/core, CLI, format/diff/source and
archive-baseline gates pass within 0/142/142; independent review returned
`ACCEPT`.

The private `cc_common.bzl` source-order audit reaches complete 137-line
`create_linking_context_from_compilation_outputs.bzl`, whose first incomplete
child is 675-line `cc_linking_helper.bzl`. Recursive source order through its
273-line `cpp_link_action.bzl`, 469-line `finalize_link_action.bzl` and 479-line
`collect_solib_dirs.bzl` reaches the shared dependency
`cc/private/link/target_types.bzl`, 131 lines, SHA-256
`12110c7dce405cd2ba4253d694502f08cc97a95bd0004444054ae8aa689da8fd`.

Both target-type children are accepted complete producers: `action_names.bzl`
with empty mapping and `cc_helper_internal.bzl` with exact
`bazel_skylib -> bazel_skylib+` mapping. The eager surface retains their public
`ACTION_NAMES` and `artifact_category_names` exports, two strings, a two-field
linking-mode struct, a ten-row/six-field nested target-type table and one public
lazy function.

Run only `WP-4-7A-rules-cc-target-types-complete-loading-proof`. Do not invoke
`is_dynamic_library`, create link actions or claim any consumer.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Accepted complete
children plus exact string/struct/lazy-function regressions cover every eager
shape; no fresh oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is a peer implementation. Its
defining-module ownership and shared type-vocabulary placement inform proof
architecture only. Copy no Zig code, representation, algorithm, diagnostic or
behavior.

- **Exact:** complete source/hash/owner/mapping; both child labels/mappings;
  imported pointer identities/visibility; both strings; two-field linking-mode
  struct name/value mappings; complete ten-row/six-field target-type name sets
  and named value mappings; public function type/visibility; exact public and
  all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining both child heaps and owning its strings, nested structs and
  lazy function; constructor-order struct iteration rather than Bazel's sorted
  struct-field iteration.
- **Unsupported/deferred:** function invocation and result; every link target
  consumer; collector/finalizer/LTO/link action; private/public `cc_common`,
  configured C++, actions and execution.

The frozen producer retains child heaps and owns its eager table/function. No
evaluator borrow or invocation value escapes. No production, DICE, request,
cache, async, fixture, oracle, hot-path, fallback or utility-reuse decision is
introduced.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current and Stage 4 documents may change only after terminal acceptance to roll
the result and next packet.

At base `2c1706e70` the Rust test authority is 20,387 lines, SHA-256
`35c23350be7ead51047147abd339766d5c53006089b636f64d7bd4009aad73aa`.
Its final ceiling is 20,887 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 500 proof and 500 total additions; deletions do not buy
budget. Embed/hash all 131 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/link:target_types.bzl`, path
`/rules_cc/cc/private/link/target_types.bzl`, with empty mapping and the two
accepted complete children in source order. Reuse their actual defining
identities and assert exact labels/mappings.

Prove public `ACTION_NAMES` and `artifact_category` pointer-identical to child
exports. Prove exact `USE_LINKER = "linker"` and `USE_ARCHIVER = "archiver"`.
Prove `LINKING_MODE` exactly `STATIC = "static"`, `DYNAMIC = "dynamic"`.
Prove every named `LINK_TARGET_TYPE` row and each `_name`,
`linker_or_archiver`, `action_name`, `is_pic`, `linker_output`, `executable`
field against the imported action/artifact values and exact scalars. Prove
public `is_dynamic_library` is type `function`. Assert exact seven-public and
seven-all name sets. Invoke nothing and add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete child/import/string/struct/function inventory, defining
identities, no-invocation/consumer boundary, compatibility split, recursive
branch selection and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete table or
binding coverage, invocation, lost imported/eager identity, evaluator-borrowed
value, consumer claim, unpinned source, copied Zabel content, dirty authority,
allowlist escape or cap/function violation. Stop after this producer and
re-audit `collect_solib_dirs.bzl`.

## Immediate predecessor

Commit `2c1706e70` accepts only complete linker-input defining-module freezing.
`ace75573b` accepts only create-library and `cb71a302d` accepts only the
universal environment and bounded set subset. None accepts target-type callable
behavior, any consumer, configured C++ or actions.
