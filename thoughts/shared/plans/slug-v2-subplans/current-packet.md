# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-utils-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete rules_rust
`rust/private/utils.bzl` utility family over its five complete direct children,
proving all loaded, eager and function bindings without invocation.

## Learned facts and decision

Commit `feb0b204c` replaces the narrowed rules_cc find-toolchain edge with a
complete recursive freeze over the actual public `cc_common` child. Every direct
`utils.bzl` child is now complete: Bazel Skylib paths, rules_cc find-toolchain,
public `cc_common`, public `CcInfo`, and rules_rust providers.

Rules_rust 0.73.0 `rust/private/utils.bzl` is 1,032 lines, SHA-256
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`.
It contains 11 loaded values, 39 functions, and six eager assignments. Existing
accepted slices cover the eager encoding/substitution values and representative
exports; this packet closes the whole utility category rather than adding more
per-function harnesses.

Run only `WP-4-7A-rules-rust-utils-complete-loading-proof`. Do not invoke a
function, provider, `cc_common` field or rule implementation, and do not
evaluate `lto.bzl` in the same packet.

## Generic architecture, authorities and compatibility

This is generic recursive BCR Starlark loading, composition and freezing, not a
Rust or C++ utility implementation in Rust. Slug retains each actual complete
child value through the shared Buck2-derived evaluator. Completing the whole
utility module establishes the reusable architecture for its function family
and prevents leaf-by-leaf churn.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_rust 0.73.0 bytes are sole exact authority. Reuse
accepted evidence; add no fixture or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its producer-owned composition/freeze model may
guide ownership assertions, but no Zig code, representation, algorithm, cache,
ordering or utility behavior is copied and Zabel is not compatibility authority.

- **Exact:** complete 1,032-line source/hash; canonical parent and all five
  child owners/mappings; all 11 loaded identities; exact eager values and alias
  identities already accepted; 39 function bindings; exact 46-public/56-all
  inventories; complete freeze without invocation.
- **Slug-native:** starlark-rust parse/evaluate/freeze and test representation.
- **Unsupported/deferred:** every function/provider/facade invocation and
  output; contexts, files, labels and actions consumed by those functions;
  rules, toolchains, actions, ActionKeys and execution.

No evaluator borrow or invocation result escapes. DICE, request/revision,
filesystem, cache, async and fallback concerns are inapplicable to this
test-only proof. There is no fallback.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling
documents may change only after terminal acceptance.

At base `feb0b204c`, the Rust test authority is 30,604 lines, SHA-256
`6180ddf55436e67cb36a82ccf8d99168dfc0c5ca83b980ae0d5736193f0a82cc`.
Its final ceiling is 32,604 lines. Each new proof/helper function must remain at
most 120 physical lines. Add no production responsibility or generic archive.

Caps are 0 production, 2,000 proof and 2,000 total additions; deletions do not
buy budget. Embed/hash all 1,032 authenticated lines. Build the five actual
complete children at their canonical owners, workspace paths and mappings.
Evaluate the parent at `@@rules_rust+//rust/private:utils.bzl`, path
`/rules_rust/rust/private/utils.bzl`, mapping `bazel_skylib -> bazel_skylib+`
and `rules_cc -> rules_cc+`. Prove child/import identity, all eager values and
aliases, function visibility/types and exact inventories. Invoke nothing.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/function sizes and perform root review of
source, child closure, identities/eager values/inventories, no-invocation scope,
generic architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, incomplete or
stubbed child, missing parser/global/evaluator shape, any invocation/output
inspection, evaluator-borrowed value, Rust/C++ semantic claim, unpinned source,
copied Zabel content, dirty authority, allowlist escape, or cap/function
violation. Stop after this complete utility family and then return to complete
`lto.bzl`.

## Immediate predecessor

Commit `feb0b204c` accepts the final complete direct child. Earlier commits
accept selected utility slices, but not the complete `utils.bzl` module or any
utility behavior.
