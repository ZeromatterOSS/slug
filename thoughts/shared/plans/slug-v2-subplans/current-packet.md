# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-remaining-compatibility-proxy-closure-audit`

Milestone: M7A command/ruleset bootstrap closure.

Result: authenticate the recursive freeze closure of the three remaining eager
generated-proxy children and select exactly one smallest coherent successor, or
record `REPLAN`. Add no Rust, fixture or oracle evidence.

## Accepted base and audit question

Base commit is `4b2396f0a` (`Prove exact ObjcInfo proxy aliases`). The three
scheduling documents are clean at:

- canonical plan: 4,370 lines, SHA-256
  `f790a0522b1503ef6e33365b465a203a58c9435b6771a0fe92acd9483c32ca16`;
- Stage 4 subplan: 6,747 lines, SHA-256
  `93d6f2a36618e2236cd3c41268ac74ba6119423fc6e561e1d0e306aadbe298da`;
- current packet before replacement: 125 lines, SHA-256
  `9c538a0c263e64d573f2440b5bc988d7af6be90c37961a71af142b5ee585f6fd`.

Commits `0699dffe7`, `498e5efc7` and `4b2396f0a` accept three of the six
generated `symbols.bzl` child families: exact complete `CcSharedLibraryInfo`,
`DebugPackageInfo` and `ObjcInfo`, including their exact narrowed proxy aliases.
They do not make the complete proxy freeze.

The remaining eager children are:

- `cc/private:cc_common.bzl`: 788 lines, SHA-256
  `5e6ab737945b487759c9f039c77a066dc65bbe15cf590b566fe86029cc610762`;
- `cc/private:cc_info.bzl`: 656 lines, SHA-256
  `4424bb876c3f8234d7cfce20652e7ab1a7b2fc34cc2c637b1cb4313590d9f1bc`;
- `cc/private/toolchain_config:cc_toolchain_config_info.bzl`: 143 lines,
  SHA-256
  `8c522773214e202b426ae43589f59a8bdbf3af19d2e595ba8ec7ac125fef5d39`.

`cc_common.bzl` eagerly loads a broad compile/link/toolchain graph. Private
`cc_info.bzl` loads Skylib paths, `cc_helper_internal`, `cc_internal` and
extra-link-time-library helpers before many provider/context declarations.
Toolchain-config info loads exact Skylib paths, `cc_internal` and
`legacy_features.bzl`; its local initialized provider declaration alone is not
a complete-module proof.

## Authorities and audit method

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc 0.2.17/Skylib sources are sole exact behavior and
byte authority. Reuse accepted evidence only when it proves the same complete
producer/binding; source-shaped snippets do not substitute for eager loads.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only recursive
defining-module value reachability, loaded-binding ownership and freeze
closure. Copy no Zig code, representation, traversal order, owner pointer,
algorithm, identity, diagnostic or behavior.

For each remaining child:

1. Authenticate its complete source and every eager `load()` edge in source
   order, recursively through the first unaccepted evaluated expression.
2. Mark every dependency as accepted complete, accepted partial, missing
   bounded, or broad/deferred; cite commit/source hashes and line ranges.
3. Identify eager calls/defaults/comprehensions/constants that execute before
   publication and distinguish them from lazy function bodies.
4. Determine the smallest source-complete child or prerequisite that fits one
   reviewable packet without a stub or narrowed parity claim.

## Compatibility and deliverable

- **Exact:** authenticated files, ranges, load spellings/order and already
  accepted matching producer identities.
- **Slug-native:** only audit organization and any proposed future narrowed
  proof composition, explicitly labeled as such.
- **Unsupported/deferred:** all unaudited behavior, full proxy/public CcInfo,
  provider/helper invocation, configured C++ behavior, actions and analysis.

Change only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`.

Caps are 0 production, 260 documentation and 260 total additions; deletions do
not buy budget. The deliverable must update all three scheduling authorities to
one identical successor ID, with allowlist, measured caps, proof obligations,
validation, compatibility classes and STOP conditions. Independent review must
verify the complete recursive classification and Zabel's guidance-only role.

STOP and `REPLAN` if no bounded source-complete successor exists. STOP for Rust,
fixture/oracle generation, Java/JVM work, provider/helper invocation, full proxy
claim, unpinned network source, copied Zabel content, dirty authority or cap
violation.

## Immediate predecessor

Commit `4b2396f0a` adds 185 proof lines and freezes exact complete ObjcInfo. It
proves public provider-callable/private initializer/raw-function types,
visibility, distinctness, exact repository mapping, and pointer-identical
`ObjcInfo`/`new_objc_provider` public proxy aliases without invocation. All 238
loading-library, 24 invalidation and 31 BUILD-loading tests pass with
analysis/core checks and the CLI build; two independent reviews accept hashes,
scope and compatibility boundaries.
