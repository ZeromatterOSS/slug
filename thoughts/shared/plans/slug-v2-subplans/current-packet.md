# Current Slug V2 Packet

Packet: `WP-4-7A-post-clippy-parent-frontier-audit`

Milestone: M7A command/ruleset bootstrap closure.

Result: authenticate the next newly evaluated child after exact
`rust/private/clippy.bzl` returns to `rust/defs.bzl`, classify its first
unsupported loading surface, and select one bounded packet or `REPLAN`.

## Accepted starting point

Base is `db51996b9` (`Load imported frozen lint descriptors`). Exact
rules_rust 0.73.0 `clippy.bzl:463-596` now freezes recursively. Imported plain
lint attributes retain kinds/defaults and defining-repository label identities;
the imported transition retains its lint-child implementation and output.
Rich imported descriptors remain rejected.

The parent `rust/defs.bzl` SHA-256 is
`5b71e4344a6c6ee04ade488c741784479f392b71d42f2102eedc5e4993654512`.
Its first direct child `rust/toolchain.bzl` and second child
`rust/private/clippy.bzl` are accepted. Resume its direct loads in source order;
do not skip cached children or jump to a later named export.

## Authorities and guidance

Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority. Pin
the exact rules_rust archive files and relevant Bazel loading constructors or
tests needed to classify the first new expression.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architectural guidance only.
Consult only the declaration/provider/package ownership shape relevant to the
reached frontier. Copy no Zig code, representation, owner pointer, identity,
ordinal, capture, algorithm, diagnostic or behavior. Bazel decides parity.

## Compatibility and utility review

Classify the reached behavior as **exact**, **Slug-native** or
**unsupported/deferred**. Do not turn a loading proof into configured provider,
rule, action or execution compatibility.

This is a docs-only audit. Record whether the next selected implementation
would alter a retained data structure, hashing, compact collection/string,
interning, clone cost, graph storage or memory accounting. If it would, route
through the Buck2 utility skill before selecting Rust.

## Allowlist and proof

Only these files may change:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Required audit:

1. Authenticate `rust/defs.bzl` and every newly reached child by full hash.
2. Trace direct-load and recursive child order, including already completed
   common/provider dependencies.
3. Identify the first newly evaluated top-level expression after clippy returns.
4. Compare that expression with the live Slug loading owner and pinned Bazel
   behavior/tests; select exactly one bounded proof/implementation packet or
   `REPLAN`.
5. Record the exact compatibility boundary, Zabel guidance-only role, utility
   decision, allowlist, proof obligations, caps and serial validation.

Independent terminal review must verify source order, authority, first stop,
scope, compatibility and the guidance-only boundary.

## STOP

Edit no Rust. Stop for dirty authority, skipped source order, configured
semantics, another production owner, Java/JVM work, copied Zabel content or an
unbounded frontier.

## Immediate predecessor

`db51996b9` accepted imported frozen lint descriptors and complete exact clippy
tail loading at 39 production and 259 proof additions with 223/24/31 tests
passing.
