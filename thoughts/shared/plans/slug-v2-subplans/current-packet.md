# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-private-cc-info-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete 656-line rules_cc
`cc/private/cc_info.bzl` producer loads four accepted-complete children,
evaluates its provider/empty-context rows, and freezes all lazy bindings. Add no
production behavior and invoke no lazy helper.

## Learned facts and decision

Base commit is `30ec1de4f` (`Prove complete extra link library freeze`). It adds
316 proof lines and no production, embeds/hashes all 192 source lines, rebuilds
the exact recursive helper/internal closure, retains both loaded identities,
proves four distinct provider callables and private visibility, and verifies
the exact `_EMPTY` provider ID/list. Focused proof, 242 library tests, 24
invalidation tests, 31 BUILD-loading tests, locked analysis/core checks, CLI
build, formatting and hygiene pass. Independent review accepts caps and bounds.

All four loads of rules_cc 0.2.17 `cc/private/cc_info.bzl` are now complete in
source order:

1. Skylib `lib/paths.bzl` (320 lines, `96cce438...`);
2. rules_cc `cc/common/cc_helper_internal.bzl` (383, `793ab429...`);
3. rules_cc `cc/private/cc_internal.bzl` (17, `8241ced5...`);
4. rules_cc extra-link library (192, `522312ac...`).

The parent is 656 lines, SHA-256
`4424bb876c3f8234d7cfce20652e7ab1a7b2fc34cc2c637b1cb4313590d9f1bc`.
Lines 23-153 eagerly declare five ordinary providers and construct three empty
contexts. `EMPTY_COMPILATION_CONTEXT` uses accepted zero-argument `depset()` and
the admitted `cc_internal.create_header_info()` projection; the other two rows
use empty depsets. Lines 247-269 declare initialized `CcInfo` plus private raw
constructor. Every other `def` body is lazy. Existing tests accept all evaluator
and provider/context slices, but no proof owns the complete producer and loaded
identities. No further unsupported eager expression remains.

Therefore run only
`WP-4-7A-rules-cc-private-cc-info-complete-loading-proof`. Do not claim
`cc_common`, toolchain config, public generated proxy, or configured C++.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc/Skylib bytes are sole exact authority. Clean
`../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only
defining-module loaded-value ownership, retained child identity and recursive
freeze before proxy reexport. Copy no Zig code, representation or behavior.

- **Exact:** complete source/hash and load order/canonical owners; imported
  pointer identities; provider callable/schema/visibility identities; exact
  source-owned empty-context and initialized-provider declaration sequence;
  frozen public/private binding types.
- **Slug-native:** the already admitted narrow `cc_internal` bridge/header-info
  backing and proof composition in Slug's frozen heaps.
- **Unsupported/deferred:** manual/lazy/internal/provider invocation beyond the
  exact source-owned eager rows; context create/merge behavior; complete
  `cc_common`, toolchain config or proxy; configured C++ semantics/actions.

Frozen child/parent heaps own all callables, contexts, depsets and closures; no
evaluator borrow escapes. No production, DICE, request, cache, async, fixture,
oracle, hot-path, fallback or utility-reuse decision is introduced.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result.

At base `30ec1de4f` the Rust authority is 10,759 lines, SHA-256
`894e59abcae6cb977567c5f9037fec8cf6cb460dba9acd9793a4d62f307168b0`.
Its final ceiling is 11,659 lines. A recursive closure builder and the new proof
function must each remain at most 120 physical lines; the file-scope exact-source
constant is exempt from function ceilings but counts against the packet cap.
The oversized test module stays cohesive around its private load harness and
adjacent authenticated source constants; add no production responsibility or
generic source archive.

Caps are 0 production, 900 proof and 900 total additions; deletions do not buy
budget. Embed/hash all 656 lines; build the four exact frozen children and
actual Skylib mapping; evaluate at exact owner
`@@rules_cc+//cc/private:cc_info.bzl`; prove all four imported pointer identities,
six provider callable identities/types/visibility, the three empty-context
provider IDs and distinguishing field/list/depset shapes, initialized
`CcInfo`/raw types and lazy exported/private function types. Permit only exact
source-owned eager calls. Add no fixture or fresh oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceilings and obtain independent review
of bytes, recursive identities, eager/lazy boundary, exact/Slug-native split,
Zabel's guidance-only role and compatibility classes.

STOP and `REPLAN` for production change, source/hash mismatch, missing evaluator
shape, manual/lazy invocation, copied/narrowed source, lost identity,
evaluator-borrowed value, parent/proxy claim, unpinned source, copied Zabel
content, dirty authority, allowlist escape or cap/function violation. Stop after
private CcInfo and re-audit `cc_common` plus toolchain-config source order.

## Immediate predecessor

Commit `30ec1de4f` completes private CcInfo's final loaded child. It does not
complete private CcInfo or any remaining generated-proxy root.
