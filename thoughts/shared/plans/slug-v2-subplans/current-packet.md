# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-common-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete rules_rust
`rust/private/common.bzl` facade over the accepted complete provider family and
prove its imports, eager constants, struct and provider list without invocation.

## Learned facts and decision

Commit `72d68b3dc` freezes all 18 declarations in complete dependency-free
`rust/private/providers.bzl` as one general provider-family proof. The next
direct toolchain child, `rust/private/common.bzl`, now has its sole recursive
dependency complete.

Rules_rust 0.73.0 `rust/private/common.bzl` is 85 lines, SHA-256
`cee50122624c7fd9c9a6545a647062f350dd25bc8cf6dda873944290463d4db6`.
It imports six exact provider callables, declares two version strings and one
lazy private constructor, builds the eight-field `rust_common` struct, and
builds the ordered three-entry `COMMON_PROVIDERS` list. No function or provider
is invoked during loading.

Run only `WP-4-7A-rules-rust-common-complete-loading-proof`. Do not invoke the
private constructor or any provider, inspect instances, or continue into the
toolchain module in the same packet.

## Generic architecture, authorities and compatibility

This is generic BCR Starlark loaded-value composition, struct/list freezing and
visibility, not Rust-provider semantics implemented in Rust. Slug retains the
actual accepted child values by identity through its shared evaluator.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_rust 0.73.0 bytes are sole exact authority. Add no
fixture or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its producer-owned composition model supports this
boundary, but no Zig code, representation, algorithm or cache is copied and
Zabel is not compatibility authority.

- **Exact:** complete 85-line source/hash; canonical parent/child owners; six
  loaded identities; two string values; private function visibility; ordered
  eight-field struct and three-provider list identities; exact inventories;
  complete freeze without invocation.
- **Slug-native:** starlark-rust parse/evaluate/freeze and test representation.
- **Unsupported/deferred:** constructor/provider invocation and instances;
  downstream rule implementations, toolchains, actions, ActionKeys and
  execution.

No evaluator borrow or invocation result escapes. DICE, request/revision,
filesystem, cache, async and fallback concerns are inapplicable to this
test-only proof. There is no fallback.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling
documents may change only after terminal acceptance.

At base `72d68b3dc`, the Rust test authority is 30,304 lines, SHA-256
`b3dd969d9c6de5fd8acb8d7531191bd3cc6f01a6a9867e93649401cc6b1ef1ec`.
Its final ceiling is 30,654 lines. Each new proof/helper function must remain at
most 120 physical lines. Add no production responsibility or generic archive.

Caps are 0 production, 350 proof and 350 total additions; deletions do not buy
budget. Embed/hash all 85 authenticated lines. Evaluate the complete provider
child at `@@rules_rust+//rust/private:providers.bzl`, then the parent at
`@@rules_rust+//rust/private:common.bzl`, with canonical workspace paths, empty
mappings and the actual same-package load. Prove imports, eager values, exact
struct/list identities, visibility and inventories. Invoke nothing.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/function sizes and perform root review of
source, owners, identities/order/inventories, no-invocation scope, generic
architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, incomplete child,
missing parser/global/evaluator shape, any invocation or instance/output
inspection, evaluator-borrowed value, Rust-rule semantic claim, unpinned source,
copied Zabel content, dirty authority, allowlist escape, or cap/function
violation. Stop after this facade and continue the toolchain direct-load audit
at `rust/private/lto.bzl`.

## Immediate predecessor

Commit `72d68b3dc` accepts only the complete rules_rust provider producer. It
does not accept this common facade, provider behavior, configured rules or
actions.
