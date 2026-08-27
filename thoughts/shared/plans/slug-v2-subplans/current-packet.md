# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-lto-backends-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 540-line rules_cc
`cc/private/link/lto_backends.bzl` producer over its four accepted children.
Prove its complete imported/provider/function surface without invocation.

## Learned facts and decision

Base implementation commit is `78acfe43f` (`Prove C++ linkstamp compile
freeze`). It byte-verifies all 111 linkstamp lines over the six actual complete
children, including CcInfo's retained Skylib mapping, and proves all six imported
pointers/visibility, its public lazy function and exact six-public/seven-all
name inventories without invocation. Focused, all 258 loading-library, 24/31
integration, locked analysis/core, CLI, format/diff/source and archive gates
pass within 0/223/223; independent review returned `ACCEPT`.

The private `cc_common.bzl` source-order audit next reaches 291-line
`cc/private/link/create_library_to_link.bzl`. Its first incomplete direct child
is rules_cc 0.2.17 `cc/private/link/lto_backends.bzl`, 540 lines, SHA-256
`078bfb686e85b584745fcea2d9e5535938f9afc1a0066f80cc88aceb699f4226`.
All four loaded children have accepted complete proofs: Skylib paths,
`cc_helper_internal.bzl`, `cc_internal.bzl` and the native `cc_common` wrapper.

The eager surface retains public `paths` and
`should_create_per_object_debug_info`, private `_cc_internal` and
`_cc_common_internal`, public `LtoBackendArtifactsInfo`, four public functions
and six private functions. The source creates no top-level set, instance,
native result or invocation value.

Run only `WP-4-7A-rules-cc-lto-backends-complete-loading-proof`. Do not invoke
a function/provider, inspect callable ABI/results, create backend artifacts or
actions, or claim `create_library_to_link.bzl`, private/public `cc_common`,
proxy, configured C++, actions or execution.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc/Skylib bytes are sole exact authority. Accepted
complete-child, provider and lazy-function regressions cover every eager shape;
no fresh oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is a peer implementation. Its
defining-module ownership and separation from later action values inform proof
architecture only. Copy no Zig code, representation, algorithm, diagnostic or
behavior.

- **Exact:** complete source/hash/owner/mapping; all four child labels/mappings;
  all four imported pointer identities/visibility; provider source/export
  identity; four public and six private function types/visibility; exact public
  and all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining imported child heaps, the provider and lazy functions.
- **Unsupported/deferred:** every function/provider invocation and callable
  ABI/result; ThinLTO backend artifact/action creation; native C++ methods;
  `create_library_to_link.bzl`, private/public `cc_common`, proxy, configured
  C++, actions and execution.

The frozen producer retains child heaps and owns its provider/functions. No
evaluator borrow or invocation value escapes. No production, DICE, request,
cache, async, fixture, oracle, hot-path, fallback or utility-reuse decision is
introduced.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current, Stage 4, Stage 5 and routing documents may change only after terminal
acceptance to roll the result and next packet.

At base `78acfe43f` the Rust test authority is 19,125 lines, SHA-256
`e475afa2849a8ba7f1a4d2f5444af02dbdb3b81fef81e5aa63becd4c342861a7`.
Its final ceiling is 20,025 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 900 proof and 900 total additions; deletions do not buy
budget. Embed/hash all 540 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/link:lto_backends.bzl` with
`bazel_skylib -> bazel_skylib+` mapping and the four accepted complete children
in source order. Reuse their actual defining identities and assert every child
label/mapping.

Prove all four imports pointer-identical to child exports with exact visibility.
Prove public `LtoBackendArtifactsInfo` has exact provider source/export identity.
Prove public `create_lto_backends`, `create_shared_non_lto_artifacts`,
`setup_common_lto_variables` and `create_lto_backend_artifacts`; prove private
`_backend_user_compile_flags`, `_add_profile_for_lto_backend`,
`_create_lto_backend_action`, `_paths_build_variables`,
`_get_lto_backend_action_inputs` and `_get_lto_backend_action_outputs`, all of
type `function`. Assert exact seven-public/fifteen-all name sets. Invoke nothing
and add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete imported/provider/function inventory, defining identities,
no-invocation boundary, compatibility split, branch selection and Zabel's
peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, invocation, lost imported/provider identity, evaluator-borrowed value,
consumer claim, unpinned source, copied Zabel content, dirty authority,
allowlist escape or cap/function violation. Stop after this producer and
re-audit complete `create_library_to_link.bzl` and private `cc_common` source
order.

## Immediate predecessor

Commit `78acfe43f` accepts only complete linkstamp defining-module freezing.
`d32e2602d` accepts only complete `compile.bzl`, and `cb71a302d` accepts only the
universal environment and bounded set subset. None accepts this producer,
callable behavior, configured C++ or actions.
