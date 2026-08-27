# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-private-cc-shared-library-hint-info-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete dependency-free 56-line rules_cc
`cc/private/cc_shared_library_hint_info.bzl` producer freezes its declared
provider identity. Add no production behavior and invoke no provider.

## Learned facts and decision

Base commit is `badf5844a` (`Prove complete launcher info freeze`). It adds 80
proof lines and no production, embeds/hash-checks all 31 launcher-info lines,
rebuilds the accepted recursive helper, retains the imported wrapper identity,
and proves initialized provider/raw/private-constructor identities and types
without invocation. Focused proof, 244 library tests, 24 invalidation tests, 31
BUILD-loading tests, locked analysis/core checks, CLI build, formatting and
hygiene pass. Independent review accepts caps and compatibility boundaries.

Private `cc_common.bzl` source order now has complete helper, private CcInfo,
`cc_internal`, and launcher-info children. Its next child is rules_cc 0.2.17
`cc/private/cc_shared_library_hint_info.bzl`: 56 lines, SHA-256
`7d067aad7862af26ee701dfa32c611d608fd606aaba06ca7dc232b6d7291d415`.
It has no loads or lazy functions and eagerly declares only public
`CcSharedLibraryHintInfo` with its authenticated two-field schema. Existing
ordinary-provider loading support covers its complete evaluator surface.
Toolchain config remains the broader later generated-proxy child.

Therefore run only
`WP-4-7A-rules-cc-private-cc-shared-library-hint-info-complete-loading-proof`.
Do not claim provider invocation/instances, private/public `cc_common`, the
generated proxy, toolchain config, or configured C++.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Bazel's
`StarlarkRuleClassFunctions.provider` and accepted provider-schema regressions
supply the existing evaluator contract; no fresh oracle is needed. Clean
`../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only
defining-module ownership and freeze before reexport. Copy no Zig code,
representation or behavior.

- **Exact:** complete source/hash and canonical owner; dependency-free load
  shape; provider callable source/export identity, public visibility and frozen
  type.
- **Slug-native:** proof composition in Slug's frozen heap.
- **Unsupported/deferred:** provider invocation or instances; private/public
  `cc_common`, generated proxy, toolchain config, configured C++ semantics or
  actions.

The frozen producer heap owns the provider callable; no evaluator borrow
escapes. No production, DICE, request, cache, async, fixture, oracle, hot-path,
fallback or utility-reuse decision is introduced. Instance behavior is skipped
because invocation is an unsupported later phase.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result.

At base `badf5844a` the Rust authority is 11,731 lines, SHA-256
`91725663f66a5eab8259cf7d55d3dc02b317e33511f6e7c2f5e63bb4ab1113ae`.
Its final ceiling is 11,831 lines. The new proof function must remain at most
120 physical lines. The oversized test module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 100 proof and 100 total additions; deletions do not buy
budget. Embed/hash all 56 lines; evaluate at exact owner
`@@rules_cc+//cc/private:cc_shared_library_hint_info.bzl`; prove exact
`CcSharedLibraryHintInfo` callable source/export identity, type and public
visibility. Invoke no binding and add no fixture or fresh oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceiling and obtain independent review of
bytes, dependency-free boundary, provider identity, compatibility split, and
Zabel's guidance-only role.

STOP and `REPLAN` for production change, source/hash mismatch, missing evaluator
shape, provider invocation, copied/narrowed source, wrong provider identity,
evaluator-borrowed value, parent/proxy claim, unpinned source, copied Zabel
content, dirty authority, allowlist escape or cap/function violation. Stop after
shared-library hint info and re-audit private `cc_common` source order against
the toolchain-config branch.

## Immediate predecessor

Commit `badf5844a` completes private launcher info. It does not complete private
`cc_common`, the generated compatibility proxy, or any public C++ route.
