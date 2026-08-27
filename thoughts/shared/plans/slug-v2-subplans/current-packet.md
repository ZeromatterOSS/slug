# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-extra-link-library-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete 192-line rules_cc
`cc/private/link/create_extra_link_time_library.bzl` producer loads its two
accepted-complete children, evaluates its provider declarations/empty instance,
and freezes all bindings without invoking a lazy helper. Add no production.

## Learned facts and decision

Base commit is `bb6b7356a` (`Prove complete rules_cc helper freeze`). It adds
exactly 480 proof lines and no production, embeds/hashes all 383 helper lines,
loads three exact-complete children with the actual Skylib mapping, retains
their pointer identities, evaluates exactly 22 source-owned initializer calls,
and freezes the complete helper. Focused proof, 241 library tests, 24
invalidation tests, 31 BUILD-loading tests, locked analysis/core checks, CLI
build, formatting and hygiene pass. Independent review accepts bytes, caps,
eager/lazy boundary and compatibility classes.

Source order now reaches private `cc_info.bzl`'s fourth load,
`cc/private/link/create_extra_link_time_library.bzl`: 192 lines, SHA-256
`522312ac48567566725f0768a6961fcaa78577fa24ac8007d5b1b8ca19698e82`.
Its two loads are now exact complete:

1. `cc/common/cc_helper_internal.bzl` (383, `793ab429...`);
2. `cc/private/cc_internal.bzl` (17, `8241ced5...`).

The producer eagerly declares free-field `ExtraLinkTimeLibraryInfo` and
`ExtraLibraryInfo`, private three-field `_KeyInfo`, and documented one-field
`ExtraLinkTimeLibrariesInfo`, then makes the single `_EMPTY` instance at lines
87-89. Lines 45-78 and 91-192 are lazy function bodies. Existing source-shaped
proof accepts these exact provider schemas, optional fields and empty instance,
but does not own the complete producer or loaded identities. No unsupported
eager expression remains.

Therefore run only
`WP-4-7A-rules-cc-extra-link-library-complete-loading-proof`. Do not claim
private CcInfo, `cc_common`, the generated proxy, or any lazy operation.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Clean `../zabel`
commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only the
architecture that defining child modules own loaded functions/tokens, the
parent retains those identities, and recursive freeze closes before CcInfo
imports the producer. Copy no Zig code, representation, traversal or behavior.

- **Exact:** all 192 source lines/hash, load order/canonical owners and child
  pointer identity; four provider callable identities/schemas; private
  visibility; exact source-owned `_EMPTY` call, value and frozen bindings.
- **Slug-native:** only proof composition in Slug's frozen module heaps.
- **Unsupported/deferred:** manual or lazy provider/helper/internal invocation;
  create/merge/build behavior; complete private CcInfo, `cc_common`, toolchain
  config or proxy; configured C++ semantics, actions and analysis.

The three frozen heaps own all loaded functions, provider callables, `_EMPTY`
and lazy closures with no evaluator borrow. No production, DICE, request,
cache, async, fixture, oracle, hot-path, fallback or utility-reuse decision is
introduced.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result.

At base `bb6b7356a` the Rust authority is 10,443 lines, SHA-256
`9afb6a9e696890816b38014b7482f64c521133c66ab8d04566a6cb0f7d7837e0`.
Its final ceiling is 10,763 lines. The new test function must remain at most
120 physical lines; a file-scope exact-source constant is exempt from that
function ceiling but counts against the packet cap. The oversized test module
remains cohesive around its private evaluator/load harness and adjacent exact
rules-source constants; add no production responsibility or generic archive.

Caps are 0 production, 320 proof and 320 total additions; deletions do not buy
budget. Embed/hash all 192 lines; build the exact frozen helper/internal child
closure; evaluate at owner
`@@rules_cc+//cc/private/link:create_extra_link_time_library.bzl`; prove both
loaded binding identities, public/private callable types/visibility, four
distinct provider identities, `_EMPTY`'s matching provider identity and empty
libraries list, and lazy exported function types. Permit only the exact
source-owned `_EMPTY` provider call. Add no fixture or fresh oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review
of exact bytes, child/provider identities, eager/lazy boundary, ownership,
Zabel's guidance-only role and compatibility classes.

STOP and `REPLAN` for production change, source/hash mismatch, missing evaluator
shape, manual/lazy/internal invocation, copied/narrowed source, lost identity,
evaluator-borrowed frozen value, parent/proxy claim, unpinned source, copied
Zabel content, dirty authority, allowlist escape or cap violation. Stop after
this producer and re-audit private CcInfo's own eager expressions.

## Immediate predecessor

Commit `bb6b7356a` completes the first `cc_common` child and the third load of
private CcInfo. It does not complete private CcInfo or any remaining proxy root.
