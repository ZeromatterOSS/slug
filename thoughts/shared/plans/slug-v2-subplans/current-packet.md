# Current Slug V2 Packet

Packet: `WP-4-7A-post-lints-parent-frontier-audit`

Milestone: M7A command/ruleset bootstrap closure.

Result: resume exact `rust/defs.bzl` source order after lints returns,
authenticate the next newly evaluated child, classify its first unsupported
loading surface, and select one bounded packet or `REPLAN`.

## Accepted starting point

Base is `227257a90` (`Prove exact lints child loading`). Exact rules_rust 0.73.0
`rust/private/lints.bzl` freezes recursively with its provider child. The
provider/export/order/schema identities are accepted; helper/rule execution and
configured behavior remain deferred.

The parent `rust/defs.bzl` SHA-256 remains
`5b71e4344a6c6ee04ade488c741784479f392b71d42f2102eedc5e4993654512`.
Its toolchain, clippy, common and lints loads now return through accepted
children. Resume at the next direct load in source order, then follow recursive
children in the existing first-seen manifest order.

## Authorities and guidance

Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority. Pin
the exact rules_rust archive files and relevant Bazel loading constructors or
tests needed to classify the first new expression.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architectural guidance only.
Consult the declaration/provider/package ownership shape relevant to the
reached frontier. Copy no Zig code, representation, owner pointer, identity,
ordinal, capture, algorithm, diagnostic or behavior. Bazel decides parity.

## Compatibility and utility review

Classify the reached behavior as **exact**, **Slug-native** or
**unsupported/deferred**. Do not turn loading reachability into configured
provider, rule, action or execution compatibility.

This is a docs-only audit. Record whether the next implementation would alter
a retained data structure, hashing, compact collection/string, interning,
clone cost, graph storage or memory accounting. If it would, route through the
Buck2 utility skill before selecting Rust.

## Allowlist and proof

Only these files may change:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Required audit:

1. Authenticate `rust/defs.bzl` and every newly reached child by full hash.
2. Trace direct-load and recursive child order, including every already
   completed dependency.
3. Identify the first newly evaluated top-level expression after lints returns.
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

`227257a90` accepted exact lints-child loading at 180 proof-only additions with
224/24/31 tests passing.
