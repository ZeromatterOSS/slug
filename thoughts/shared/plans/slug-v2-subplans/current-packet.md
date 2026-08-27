# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-private-lto-compilation-context-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: prove the authenticated complete 97-line rules_cc
`cc/private/compile/lto_compilation_context.bzl` producer loads its two
accepted-complete children, declares both providers, freezes every lazy function,
and constructs the exact empty context. Add no production behavior and invoke no
lazy binding.

## Learned facts and decision

Base commit is `9b44f0352` (`Prove complete shared library hint freeze`). It adds
88 proof lines and no production, embeds/hash-checks all 56 dependency-free
source lines and proves exact public provider identity without invocation.
Focused proof, 245 library tests, 24 invalidation tests, 31 BUILD-loading tests,
locked analysis/core checks, CLI build, formatting and hygiene pass. Independent
review accepts caps and compatibility boundaries.

Private `cc_common.bzl` source order now reaches
`cc/private/compile/cc_compilation_outputs.bzl`. Its helper and `cc_internal`
children are complete; the first incomplete child is rules_cc 0.2.17
`cc/private/compile/lto_compilation_context.bzl`: 97 lines, SHA-256
`a17435cd56fa165c71081e99f9af73407f7b4cc1dc086e53771dcf74df81b3f4`.
It loads only complete helper/internal producers, eagerly declares
`LtoCompilationContextInfo` and `BitcodeInfo`, freezes three lazy public
functions, and constructs one empty LTO context with an empty dictionary. No
unsupported eager expression remains. Toolchain config is still the broader
later generated-proxy branch.

Therefore run only
`WP-4-7A-rules-cc-private-lto-compilation-context-complete-loading-proof`. Do
not claim lazy function behavior, compilation outputs, private/public
`cc_common`, generated proxy, toolchain config, or configured C++.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc bytes are sole exact authority. Existing accepted
provider, dictionary and recursive-load regressions cover the evaluator shapes;
no fresh oracle is needed. Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only defining-module
ownership, retained child identity and recursive freeze before reexport. Copy
no Zig code, representation or behavior.

- **Exact:** complete source/hash and two-load source order/canonical owners;
  imported list/token pointer identities; both provider callable identities,
  types and visibility; all lazy public function types; exact empty-context
  provider identity and empty-dictionary shape.
- **Slug-native:** composition through the accepted helper/internal closure and
  Slug frozen heaps.
- **Unsupported/deferred:** lazy/provider invocation beyond the source-owned
  empty instance; LTO merge/create/query behavior; complete compilation outputs,
  private/public `cc_common`, generated proxy, toolchain config, configured C++
  semantics or actions.

Frozen child/parent heaps own every callable, imported value and empty context;
no evaluator borrow escapes. No production, DICE, request, cache, async,
fixture, oracle, hot-path, fallback or utility-reuse decision is introduced.

## Allowlist, caps and proof

Change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- the three scheduling documents when rolling the accepted result.

At base `9b44f0352` the Rust authority is 11,819 lines, SHA-256
`28dce9691247dad5f8eadb22cd3c358b434e845582eb4c556da502f919981190`.
Its final ceiling is 12,039 lines. The new proof function must remain at most
120 physical lines. The oversized test module remains cohesive around its
private load harness and adjacent authenticated source constants; add no
production responsibility or generic source archive.

Caps are 0 production, 220 proof and 220 total additions; deletions do not buy
budget. Embed/hash all 97 lines; reuse the accepted recursive helper/internal
closure; evaluate at exact owner
`@@rules_cc+//cc/private/compile:lto_compilation_context.bzl`; prove both
imported pointer identities, exact distinct provider source/export identities,
three public lazy function types, and exact `EMPTY_LTO_COMPILATION_CONTEXT`
provider ID with empty `lto_bitcode_inputs` dictionary. Invoke no binding beyond
the exact source-owned empty-instance construction; add no fixture or oracle.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/ceiling and obtain independent review of
bytes, recursive identities, eager/lazy boundary, compatibility split, and
Zabel's guidance-only role.

STOP and `REPLAN` for production change, source/hash mismatch, missing evaluator
shape, manual/lazy invocation, copied/narrowed source, lost identity,
evaluator-borrowed value, parent/proxy claim, unpinned source, copied Zabel
content, dirty authority, allowlist escape or cap/function violation. Stop after
the LTO context and re-audit complete compilation-outputs eager evaluation
against the toolchain-config branch.

## Immediate predecessor

Commit `9b44f0352` completes shared-library hint info. It does not complete LTO
compilation context, compilation outputs, private `cc_common`, or the generated
compatibility proxy.
