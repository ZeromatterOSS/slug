# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-linkstamp-compile-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete 111-line rules_cc
`cc/private/compile/linkstamp_compile.bzl` producer over its six accepted
children. Prove every imported identity and its single lazy function without
invocation.

## Learned facts and decision

Base implementation commit is `d32e2602d` (`Prove complete C++ compile producer
freeze`). It byte-verifies all 2,295 `compile.bzl` lines and freezes the exact
eleven-child defining module. The proof covers 25 imports, four ordered sets,
the initialized provider/raw constructor, one public plus 27 private functions
and exact 25-public/59-all name inventories without invocation. Focused, all
257 loading-library, 24/31 integration, locked analysis/core, CLI,
format/diff/source and archive gates pass within 0/2694/2694; independent review
returned `ACCEPT`.

The required private `cc_common.bzl` source-order audit now reaches its first
incomplete direct child: rules_cc 0.2.17
`cc/private/compile/linkstamp_compile.bzl`, 111 lines, SHA-256
`6f5ceb39f1b6c26b65073867f3435ec01093775edf6129d2b9421bca4c7a70bb`.
Its six children are already accepted completely: action names, helper
internal, semantics, CcInfo, cc_internal and compile build variables. The eager
surface is exactly five public and one private imported aliases plus public
`register_linkstamp_compile_action`; it creates no top-level container,
provider, native value or invocation result.

Run only `WP-4-7A-rules-cc-linkstamp-compile-complete-loading-proof`. Do not
invoke the function, inspect its callable ABI/result, register an action,
implement a native C++ method, or claim private/public `cc_common`, proxy,
configured C++, actions or execution.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Accepted complete
child and lazy-function regressions cover every eager shape; no fresh oracle is
needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is a peer implementation. Its
defining-module ownership and separation from later action-construction values
inform proof architecture only. Copy no Zig code, representation, algorithm,
diagnostic or behavior.

- **Exact:** complete source/hash/owner/mapping; all six child labels/mappings;
  all six imported pointer identities/visibility; public function type and
  exact public/all-visibility name sets.
- **Slug-native:** realization through one starlark-rust frozen defining-module
  heap retaining imported child heaps and the lazy function.
- **Unsupported/deferred:** function invocation and callable ABI/result; native
  C++ methods; linkstamp action registration; private/public `cc_common`, proxy,
  configured C++, actions and execution.

The frozen producer retains child heaps and owns its function. No evaluator
borrow or invocation value escapes. No production, DICE, request, cache, async,
fixture, oracle, hot-path, fallback or utility-reuse decision is introduced.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. The canonical,
current, Stage 4, Stage 5 and routing documents may change only after terminal
acceptance to roll the result and next packet.

At base `d32e2602d` the Rust test authority is 18,902 lines, SHA-256
`e8e3525301dbd09dee255e06a895c21cbc36ea7bf0a2b86221a6a8c5dda93294`.
Its final ceiling is 19,202 lines. Each new proof/helper function must remain at
most 120 physical lines. The oversized module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 300 proof and 300 total additions; deletions do not buy
budget. Embed/hash all 111 authenticated lines. Evaluate at exact owner
`@@rules_cc+//cc/private/compile:linkstamp_compile.bzl` with empty mapping and
the six accepted complete children in source order. Reuse their actual defining
identities and assert every child label/mapping.

Prove `LINKSTAMP_COMPILE_ACTION_NAME`, `should_stamp`, `cc_semantics`,
`EMPTY_COMPILATION_CONTEXT`, private `_cc_internal` and
`get_linkstamp_compile_variables` pointer-identical to their child exports with
exact visibility. Prove `register_linkstamp_compile_action` public and of type
`function`. Assert exact public and all-visibility name sets. Invoke nothing and
add no fixture/oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review of
bytes, complete imported/function inventory, defining identities,
no-invocation boundary, compatibility split, branch selection and Zabel's
peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, another missing
global/evaluator shape, copied/narrowed source or child, incomplete binding
coverage, invocation, lost imported identity, evaluator-borrowed value,
consumer claim, unpinned source, copied Zabel content, dirty authority,
allowlist escape or cap/function violation. Stop after this producer and
re-audit private `cc_common` at its first link-family child.

## Immediate predecessor

Commit `d32e2602d` accepts only complete `compile.bzl` defining-module
freezing. `bb11a1f73`, `97faa6e71` and `3060e4d4d` accept only its action
template, variable and helper children; `cb71a302d` accepts only the universal
environment and bounded set subset. None accepts this producer, callable
behavior, configured C++ or actions.
