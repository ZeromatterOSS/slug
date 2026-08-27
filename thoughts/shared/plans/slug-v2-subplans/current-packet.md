# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-lto-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete rules_rust `rust/private/lto.bzl`
module over the accepted complete utility child, proving its import, mode list,
provider, rule declaration, functions and visibility without invocation.

## Learned facts and decision

Commit `22db26f19` freezes all 1,032 authenticated lines of
`rust/private/utils.bzl` over five complete real children. The next direct load
of `rust/private/toolchain.bzl` is `rust/private/lto.bzl`; its only child is the
now-complete utility module.

Rules_rust 0.73.0 `rust/private/lto.bzl` is 120 lines, SHA-256
`9907a2411a51f0acd36131d7a695ac4fb244c4c5482c3f8bb6a5d0194abef924`.
It loads `is_exec_configuration`, freezes a five-entry private mode list,
declares `RustLtoInfo`, two private and one public function, and the
`rust_lto_flag` rule. Top-level evaluation invokes only the already-admitted
provider, config-string descriptor and rule declaration constructors; it does
not invoke an implementation or the imported utility.

Run only `WP-4-7A-rules-rust-lto-complete-loading-proof`. Do not invoke any
function, provider or rule, inspect configured targets, or continue into the
toolchain parent in the same packet.

## Generic architecture, authorities and compatibility

This is generic BCR Starlark loading and declaration freezing, not Rust LTO
semantics implemented in Rust. The actual complete utility function is retained
by identity and normal shared host constructors represent declarations.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_rust 0.73.0 bytes are sole exact authority. Reuse
accepted evidence; add no fixture or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its producer-owned composition model may guide
identity assertions, but no Zig code, representation, algorithm, cache or LTO
behavior is copied and Zabel is not compatibility authority.

- **Exact:** complete 120-line source/hash; canonical parent/child owners;
  imported function identity; ordered five-mode list; provider source identity;
  rule class/declaration; three functions and exact four-public/seven-all
  inventories; complete freeze without invocation.
- **Slug-native:** starlark-rust parse/evaluate/freeze, declaration and test
  representations.
- **Unsupported/deferred:** every function/provider/rule invocation and output;
  configured LTO behavior, actions, ActionKeys and execution.

No evaluator borrow or invocation result escapes. DICE, request/revision,
filesystem, cache, async and fallback concerns are inapplicable to this
test-only proof. There is no fallback.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling
documents may change only after terminal acceptance.

At base `22db26f19`, the Rust test authority is 31,939 lines, SHA-256
`605e533178b419f1ad8f5e76d8855854f747f3dceb81fe82ee0020133b39947d`.
Its final ceiling is 32,289 lines. Each new proof/helper function must remain at
most 120 physical lines. Add no production responsibility or generic archive.

Caps are 0 production, 350 proof and 350 total additions; deletions do not buy
budget. Embed/hash all 120 authenticated lines. Build the complete utility
child at `@@rules_rust+//rust/private:utils.bzl` with its five real children,
then evaluate the parent at `@@rules_rust+//rust/private:lto.bzl`, path
`/rules_rust/rust/private/lto.bzl`, with empty mapping and the actual
same-package load. Prove child identity, eager values, declarations, visibility
and exact inventories. Invoke nothing.

Run focused proof and its direct compile dependent. Because this follows an
unchanged test-only loading pattern immediately after the complete utility
checkpoint, do not repeat broad suites unless focused evidence is suspect. Run
formatting, diff, caps/function-size and archive hygiene, then root review of
source, child identity, declarations/inventories, no-invocation scope, generic
architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, incomplete or
stubbed child, missing parser/global/evaluator shape, any implementation or
provider/rule invocation, evaluator-borrowed value, Rust LTO semantic claim,
unpinned source, copied Zabel content, dirty authority, allowlist escape, or
cap/function violation. Stop after this child and continue toolchain source
order at `rust/private/rust_allocator_libraries.bzl`.

## Immediate predecessor

Commit `22db26f19` accepts only complete rules_rust utility loading. Existing
synthetic tests cover selected LTO declaration shapes but not this complete
recursive module or any LTO behavior.
