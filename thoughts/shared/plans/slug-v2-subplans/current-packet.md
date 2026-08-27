# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-find-toolchain-complete-recursive-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: refreeze the authenticated complete rules_cc
`cc/find_cc_toolchain.bzl` over the actual complete public `cc_common` child,
replacing the accepted narrowed child proof without invoking any function.

## Learned facts and decision

Commit `5e7864995` accepts complete `rust/private/common.bzl`. The next
toolchain load is `lto.bzl`, whose `utils.bzl` child loads five modules. Four
are complete, but audit found that the authenticated full-source proof for
rules_cc `find_cc_toolchain.bzl` used a narrowed `cc_common = struct()` child.
That proves source/eager declarations, not the real recursive load route.

The already-embedded rules_cc 0.2.17 `cc/find_cc_toolchain.bzl` has SHA-256
`3f62d3ea99f59674f71dbc669c80dd0dc5ef14637933d727b74f0bd556334655`.
Its sole direct child, public `cc/common/cc_common.bzl`, is now complete through
commit `be1562848`. This packet recomposes the exact parent over that actual
child and proves retained identity and inventories without invocation.

Run only `WP-4-7A-rules-cc-find-toolchain-complete-recursive-loading-proof`.
Do not invoke a function or facade field, inspect toolchain output, or evaluate
rules_rust `utils.bzl` in the same packet.

## Generic architecture, authorities and compatibility

This is generic recursive BCR Starlark loading and frozen-value retention, not
C++ toolchain discovery implemented in Rust. It removes a narrowed test edge so
the future complete utility module can use only real complete children.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_cc 0.2.17 bytes are sole exact authority. Reuse the
accepted source and complete child; add no fixture or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its producer-owned identity model may guide the
recursive edge assertion, but no Zig code, representation, algorithm, cache or
toolchain behavior is copied and Zabel is not compatibility authority.

- **Exact:** existing full source/hash; canonical parent and complete public
  child owners/paths/mappings; loaded `cc_common` pointer identity; eager label
  and attribute dictionary; three function bindings; exact inventories;
  complete recursive freeze without invocation.
- **Slug-native:** starlark-rust parse/evaluate/freeze and test representation.
- **Unsupported/deferred:** every function/facade invocation and output;
  toolchain resolution, configurations, actions, ActionKeys and execution.

No evaluator borrow or invocation result escapes. DICE, request/revision,
filesystem, cache, async and fallback concerns are inapplicable to this
test-only proof. There is no fallback.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling
documents may change only after terminal acceptance.

At base `5e7864995`, the Rust test authority is 30,512 lines, SHA-256
`d53a095a81ef72c16c72fea5b3b4280c576037e21133eb5d80d3f2cf38f6dd3b`.
Its final ceiling is 30,762 lines. Each new proof/helper function must remain at
most 120 physical lines. Add no production responsibility or generic archive.

Caps are 0 production, 250 proof and 250 total additions; deletions do not buy
budget. Reuse/hash the existing exact parent source. Build the actual complete
public `cc_common` wrapper and its compatibility/private closure. Evaluate the
parent at `@@rules_cc+//cc:find_cc_toolchain.bzl`, path
`/rules_cc/cc/find_cc_toolchain.bzl`, mapping `bazel_tools -> bazel_tools+` and
`rules_cc -> rules_cc+`. Prove child/import identity, eager values, functions,
visibility and exact inventories. Invoke nothing.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/function sizes and perform root review of
source, complete child route, identities/inventories, no-invocation scope,
generic architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, incomplete or
stubbed child, missing parser/global/evaluator shape, any invocation/output
inspection, evaluator-borrowed value, C++ semantic claim, unpinned source,
copied Zabel content, dirty authority, allowlist escape, or cap/function
violation. After acceptance, return to complete rules_rust `utils.bzl`.

## Immediate predecessor

Commit `5e7864995` accepts complete rules_rust common-facade loading. The older
find-toolchain proof authenticates the parent over a narrowed child; it does not
accept this complete recursive route or any toolchain behavior.
