# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-semver-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete rules_rust `rust/private/semver.bzl`
module and its sole public function without invocation.

## Learned facts and decision

Commit `a9610a724` admits the full advertised-provider declaration family and
freezes all 302 authenticated `rust_allocator_libraries.bzl` lines over five
complete real children. The next direct load of `rust/private/toolchain.bzl` is
`rust/private/semver.bzl`.

Rules_rust 0.73.0 `rust/private/semver.bzl` is dependency-free, 51 lines, and
has SHA-256
`966fe4b90082dd92bac60398b2801824f41906eea71364b64f630f0c175250ab`.
Its only top-level declaration is the public `semver` function. String
partitioning, integer conversion, validation and result struct construction
are all lazy and require no new loading builtin.

Run only `WP-4-7A-rules-rust-semver-complete-loading-proof`. Freeze the exact
source at its canonical owner, prove the public function binding and exact
one-public/one-all inventory, and invoke nothing. Do not implement or claim
semantic-version parsing behavior and do not continue into the toolchain parent
or following incompatible-settings child in this packet.

## Generic architecture, authorities and compatibility

This is generic BCR Starlark source loading and function freezing, not a Rust
semantic-version parser. Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` and authenticated rules_rust
0.73.0 bytes are sole exact authority. Reuse accepted evidence; add no fixture
or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its generic evaluator/host split may guide module
ownership, but no Zig code, representation, parser, algorithm, cache or semver
behavior is copied and Zabel is not compatibility authority.

- **Exact:** complete 51-line source/hash; canonical owner/path/mapping; sole
  public function binding; exact one-public/one-all inventory; complete freeze
  without invocation.
- **Slug-native:** starlark-rust parse/evaluate/freeze and test representation.
- **Unsupported/deferred:** every `semver` invocation, parse result, error,
  comparison or configured toolchain consumer.

No retained semantic collection, evaluator borrow or invocation result is
added. DICE, request/revision, filesystem, cache, async, memory-ledger and
fallback concerns are inapplicable to this test-only proof. There is no
fallback and no Buck2 utility change.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling
documents may change only after terminal acceptance.

At base `a9610a724`, the Rust test authority is 32,794 lines, SHA-256
`0cb11e4170e33e999bab8e1cf35728b1f11a7986c865cafa8ce87ccf3bc97168`.
Its final ceiling is 32,944 lines. Each new proof/helper function must remain at
most 120 physical lines. Add no production responsibility or generic archive.

Caps are 0 production, 150 proof and 150 total additions; deletions do not buy
budget. Embed/hash all 51 authenticated lines. Evaluate at
`@@rules_rust+//rust/private:semver.bzl`, path
`/rules_rust/rust/private/semver.bzl`, with empty mapping and no children.
Prove the function type, public visibility and exact inventories. Invoke
nothing.

Run the focused proof and its direct compile dependent. Because this follows a
green full loading/integration/dependent checkpoint and changes only the same
source-freeze test pattern, do not repeat broad suites unless focused evidence
is suspect. Run formatting, diff, caps/function-size and archive hygiene, then
root review of source authority, owner/inventory, no-invocation scope, generic
architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, unexpected
dependency or eager evaluation, any function invocation, semantic-version
behavior claim, evaluator-borrowed value, unpinned source, copied Zabel content,
dirty authority, allowlist escape, or cap/function violation. Stop after this
module and continue toolchain source order at
`rust/settings/incompatible.bzl`.

## Immediate predecessor

Commit `a9610a724` accepts generic advertised-provider declaration retention
and complete allocator-module loading without invoking allocator behavior.
