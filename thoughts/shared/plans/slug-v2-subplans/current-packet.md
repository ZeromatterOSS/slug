# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-platform-triple-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the authenticated complete dependency-free rules_rust
`rust/platform/triple.bzl` module and prove its exact visibility inventory
without invoking host-sensitive or pure functions.

## Learned facts and decision

Commit `60a0e5630` completes the second direct rules_rust toolchain load: public
`CcInfo` freezes over the complete rules_cc compatibility chain with exact
re-export identity. The next direct load in authenticated
`rust/private/toolchain.bzl` source order is `//rust/platform:triple.bzl`.

Rules_rust 0.73.0 `rust/platform/triple.bzl` is 172 lines, SHA-256
`19fd04c62b3a50057ffc8ab9b831f5182bc531e4307f6065065ce214de4129e6`.
It has no loads or eager function calls. It defines public `triple` and
`get_host_triple` plus private `_validate_cpu_architecture`. Function bodies use
ordinary Starlark control flow and the accepted `struct` builtin lazily; loading
does not inspect host OS/architecture or invoke repository behavior.

Run only `WP-4-7A-rules-rust-platform-triple-complete-loading-proof`. Do not
invoke any function, inspect returned structs, construct a repository context,
or continue into `rust/private/common.bzl` in the same packet.

## Generic architecture, authorities and compatibility

This is generic BCR Starlark parsing/evaluation/freezing, not Rust-toolchain or
platform parsing implemented in Rust. Slug's Buck2-derived Starlark evaluator
owns the module and its functions; later consumers will invoke them through the
same general callable path. No special `triple` parser is added.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_rust 0.73.0 bytes are sole exact authority. Add no
fixture or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its producer-owned frozen-module model supports
this boundary, but no Zig code, representation, parser, cache or platform
behavior is copied and Zabel is not compatibility authority.

- **Exact:** complete 172-line source/hash; canonical owner; three function
  bindings and their public/private visibility; exact two-public/three-all
  inventories; complete freeze without invocation.
- **Slug-native:** starlark-rust parse/evaluate/freeze and test representation.
- **Unsupported/deferred:** all function invocation/output, triple parsing and
  validation behavior, repository host observations, rule implementations,
  toolchains, actions, ActionKeys and execution.

No evaluator borrow or invocation result escapes. DICE, request/revision,
filesystem, cache, async and fallback concerns are inapplicable to this
test-only proof. There is no fallback.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling
documents may change only after terminal acceptance.

At base `60a0e5630`, the Rust test authority is 29,788 lines, SHA-256
`b9d46e2e37a3a9cb706d1219e6991069400a16b72c3239f0f64e14f68cb2634b`.
Its final ceiling is 30,188 lines. Each new proof/helper function must remain at
most 120 physical lines. Add no production responsibility or generic archive.

Caps are 0 production, 400 proof and 400 total additions; deletions do not buy
budget. Embed/hash all 172 authenticated lines. Evaluate at owner
`@@rules_rust+//rust/platform:triple.bzl`, path
`/rules_rust/rust/platform/triple.bzl`, with empty repository mapping and no
children. Prove all three values are functions, private lookup behavior, exact
inventories and complete freeze. Invoke nothing.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/function sizes and perform root review of
source, owner, visibility/inventories, no-invocation scope, generic architecture
and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, missing parser/
global/evaluator shape, any invocation/output inspection, host observation,
evaluator-borrowed value, Rust-toolchain semantic claim, unpinned source, copied
Zabel content, dirty authority, allowlist escape, or cap/function violation.
Stop after this child and continue the authenticated toolchain direct-load audit
at `rust/private/common.bzl`.

## Immediate predecessor

Commit `60a0e5630` accepts only the complete public rules_cc `CcInfo` wrapper.
It does not accept this rules_rust child, any triple function behavior, the
rules_rust toolchain module, configured rules or actions.
