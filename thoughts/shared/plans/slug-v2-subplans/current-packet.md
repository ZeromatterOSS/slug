# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-providers-complete-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze all 18 provider declarations in the authenticated complete,
dependency-free rules_rust `rust/private/providers.bzl` module as one coherent
provider-family proof without invoking a provider.

## Learned facts and decision

Commit `4bce1f88e` accepts the complete dependency-free
`rust/platform/triple.bzl`, the third direct rules_rust toolchain load. The next
direct child, `rust/private/common.bzl`, eagerly loads six values from
`rust/private/providers.bzl` and composes them into `rust_common`.

Rules_rust 0.73.0 `rust/private/providers.bzl` is 238 lines, SHA-256
`57a59ec9a60b9709df197333c94bac464b572af63bc78f560ce32570b6d84ac6`.
It has no loads, functions or eager provider invocations. It declares the
complete 18-provider Rust family with documented dictionary schemas. Handling
the entire dependency-free family in one packet validates one general provider
architecture and prevents per-provider follow-up churn.

Run only `WP-4-7A-rules-rust-providers-complete-loading-proof`. Do not invoke a
provider, construct an instance, inspect initializer output, compose
`rust_common`, or continue into toolchain evaluation in the same packet.

## Generic architecture, authorities and compatibility

This is generic BCR Starlark provider declaration/evaluation/freezing, not
Rust-provider semantics implemented in Rust. Slug's shared provider builtin and
Buck2-derived Starlark evaluator own every declaration; later BCR modules retain
these same callables by identity.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_rust 0.73.0 bytes are sole exact authority. Add no
fixture or oracle.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
architectural guidance only. Its producer-owned identity model supports this
boundary, but no Zig code, representation, schema algorithm or cache is copied
and Zabel is not compatibility authority.

- **Exact:** complete 238-line source/hash; canonical owner; all 18 provider
  callable types, exported names and source labels; pairwise distinct identity;
  exact 18-public/18-all inventories; complete freeze without invocation.
- **Slug-native:** starlark-rust parse/evaluate/freeze, provider callable and
  test representations.
- **Unsupported/deferred:** provider invocation/instances and field values;
  `rust_common` composition; rules, toolchains, actions, ActionKeys and
  execution.

No evaluator borrow or invocation result escapes. DICE, request/revision,
filesystem, cache, async and fallback concerns are inapplicable to this
test-only proof. There is no fallback.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling
documents may change only after terminal acceptance.

At base `4bce1f88e`, the Rust test authority is 30,008 lines, SHA-256
`3f7fba5743fd250e10df9839fbd9d4d36c8867d13f8b52aca717c693da7e3a1b`.
Its final ceiling is 30,708 lines. Each new proof/helper function must remain at
most 120 physical lines. Add no production responsibility or generic archive.

Caps are 0 production, 700 proof and 700 total additions; deletions do not buy
budget. Embed/hash all 238 authenticated lines. Evaluate at owner
`@@rules_rust+//rust/private:providers.bzl`, path
`/rules_rust/rust/private/providers.bzl`, with empty repository mapping and no
children. Prove all 18 bindings are provider callables with exact source label
and exported name, pairwise distinct identities, public visibility and exact
inventories. Invoke nothing.

Run focused proof, all `slug_loading_v2` library tests, `bzl_invalidation`,
`build_file_loading`, locked analysis/core checks, locked CLI build, formatting,
diff and archive hygiene. Measure caps/function sizes and perform root review of
source, owner, provider family/identity, inventories, no-invocation scope,
generic architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for production change, source/hash mismatch, missing parser/
provider/evaluator shape, any invocation or instance/output inspection,
evaluator-borrowed value, Rust-rule semantic claim, unpinned source, copied
Zabel content, dirty authority, allowlist escape, or cap/function violation.
Stop after this provider family and then prove complete `rust/private/common.bzl`
composition over it.

## Immediate predecessor

Commit `4bce1f88e` accepts only complete rules_rust platform-triple loading. It
does not accept this provider module, `rust_common`, provider behavior,
configured rules or actions.
