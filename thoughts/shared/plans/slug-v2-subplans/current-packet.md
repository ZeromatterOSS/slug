# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-incompatible-settings-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete rules_rust
`rust/settings/incompatible.bzl` provider, rule and function declarations
without invocation.

## Learned facts and decision

Commit `f7a3a3f10` freezes all 51 authenticated lines of dependency-free
`rust/private/semver.bzl`. The next unresolved direct load of
`rust/private/toolchain.bzl` is `rust/settings/incompatible.bzl`.

Rules_rust 0.73.0 `rust/settings/incompatible.bzl` is dependency-free, 27
lines, and has SHA-256
`534d5103680dc47634b93ed160f639a88495707fe2c27b551defbb3c6765f040`.
It eagerly declares documented two-field `IncompatibleFlagInfo`, one private
implementation function, and `incompatible_flag`: a Boolean flag build-setting
rule with one mandatory string attribute. Every eager constructor shape is
already admitted.

Run only this packet. Freeze the exact source at its canonical owner and prove
the provider identity/schema, private function visibility, rule class,
build-setting descriptor, mandatory attribute and exact two-public/three-all
inventories. Invoke nothing. Do not inspect build-setting values or continue
into the toolchain parent.

## Generic architecture, authorities and compatibility

This is generic BCR Starlark declaration loading, not incompatible-flag
semantics implemented in Rust. Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` and authenticated rules_rust
0.73.0 bytes are sole exact authority. Reuse accepted evidence; add no fixture
or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its declaration ownership may guide identity
assertions, but no Zig code, representation, algorithm, cache, flag behavior or
diagnostic is copied and Zabel is not compatibility authority.

- **Exact:** complete 27-line source/hash; canonical owner/path/mapping;
  provider owner/name/schema; private function binding; rule/build-setting/
  attribute declarations; exact two-public/three-all inventory; complete freeze
  without invocation.
- **Slug-native:** starlark-rust parse/evaluate/freeze and declaration/test
  representations.
- **Unsupported/deferred:** provider/rule/function invocation, flag value and
  CLI/configuration behavior, configured providers and consumers.

No retained semantic collection, evaluator borrow or invocation result is
added. DICE, request/revision, filesystem, cache, async, memory-ledger and
fallback concerns are inapplicable to this test-only proof. There is no
fallback and no Buck2 utility change.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling
documents may change only after terminal acceptance.

At base `f7a3a3f10`, the Rust test authority is 32,877 lines, SHA-256
`fb4c98afe3a30425de81d32df7b4e8770a3135fca8b5b9e485ab14d62055bd0b`.
Its final ceiling is 33,027 lines. Each new proof/helper function must remain at
most 120 physical lines. Add no production responsibility or generic archive.

Caps are 0 production, 150 proof and 150 total additions; deletions do not buy
budget. Embed/hash all 27 authenticated lines. Evaluate at
`@@rules_rust+//rust/settings:incompatible.bzl`, path
`/rules_rust/rust/settings/incompatible.bzl`, with empty mapping and no
children. Prove the eager declarations, visibility and exact inventories.
Invoke nothing.

Run the focused proof and its direct compile dependent. Because this follows a
green full loading/integration/dependent checkpoint and another green
proof-only source freeze, do not repeat broad suites unless focused evidence is
suspect. Run formatting, diff, caps/function-size and archive hygiene, then
root review of source authority, declarations/inventory, no-invocation scope,
generic architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, unexpected
dependency/eager behavior, any provider/rule/function invocation, configured
flag semantic claim, evaluator-borrowed value, unpinned source, copied Zabel
content, dirty authority, allowlist escape, or cap/function violation. Stop
after this module and re-audit the now-complete direct child set of
`rust/private/toolchain.bzl`.

## Immediate predecessor

Commit `f7a3a3f10` accepts only complete semver module loading without invoking
its parser function.
