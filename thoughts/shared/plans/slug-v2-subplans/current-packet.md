# Current Slug V2 Packet

Packet: `WP-4-7A-post-toolchain-source-order-audit`

Milestone: M7A command/ruleset bootstrap closure.

Result: authenticate the recursive return from the completed private toolchain
child, the actual next child, and the first newly evaluated unsupported
expression. Clippy is only a source-text candidate. This packet is docs-only
and selects either one bounded implementation or `REPLAN`.

## Accepted starting point

Base `4aed2438` (`Load config common toolchain requirements`) retains the sole
optional C++ requirement on `rust_toolchain`, preserves mandatory identity,
rejects optional invocation before publication, and completes top-level
evaluation of `rust/private/toolchain.bzl`. Independent terminal review
returned `ACCEPT`; all packet gates passed.

## Fixed source route

Use the already-selected rules_rust 0.73.0 archive and record these fixed
files before following their loads:

- `rust/private/toolchain.bzl`, SHA-256 `c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`;
- `rust/rust_toolchain.bzl`, SHA-256 `0de5c3ba5c8a71176f881df065810a33eb2355a7007c16e47759653dbacdbd49`;
- `rust/rustfmt_toolchain.bzl`, SHA-256 `e57f8129f8b2dfac8b820ed057ca65d8a5e6945d614d53923ac65b27aaefb6f5`;
- `rust/toolchain.bzl`, SHA-256 `b94731396dc90e4ef8bbdc753252aac80208aba9cd857a7e7ca74d23f6aabbce`;
- `rust/defs.bzl`, SHA-256 `5b71e4344a6c6ee04ade488c741784479f392b71d42f2102eedc5e4993654512`;
- `rust/private/clippy.bzl`, SHA-256 `a778d2ddc77587ffbffc72efcdaa458a1ffae0763e500da1c876b9b567b2a686`.

The current hypothesis is that both public toolchain wrappers only re-export
already-frozen declarations, after which `rust/defs.bzl` reaches clippy. Do not
promote that hypothesis to accepted source order until the live recursive
loader, manifest and cached-child behavior are checked.

## Authorities and architectural guidance

Behavior authority is clean Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. Authenticate any reached clippy
provider, build-setting rule, aspect, attribute predicate or toolchain list
against its concrete Bazel API, implementation and focused regression before
classifying it.

Architectural guidance is clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a`. Its rule/aspect declaration
owners and detached toolchain-requirement capture may guide a later bounded
Rust design if that is the reached frontier. Zabel does not define recursive
load order, accepted behavior or compatibility, and no Zig code, layout,
diagnostic or evaluator algorithm may be copied.

Read the Buck2 utility skill only if the selected follow-up changes retained
data structures, compact collections, hashing, interning, clone cost or memory
accounting. This audit itself changes none.

## Audit obligations

- Reconstruct the exact caller return from `rust/private/toolchain.bzl` through
  both alias-only wrappers and the remaining `rust/toolchain.bzl` exports.
- Verify which children are already complete/memoized and which child
  `rust/defs.bzl` evaluates next.
- Traverse that child in source/evaluation order: complete imports first,
  distinguish lazy function bodies and documentation examples from evaluated
  top-level expressions, and stop at the first unsupported expression.
- Compare the reached expression to the live Slug surface and fixed Bazel 9.2
  contract. Classify the prospective change as exact, Slug-native, or
  unsupported/deferred.
- If bounded, write one implementation packet with explicit source stop,
  allowlist, base hashes, line/addition caps, discriminating proofs, serial
  validation and STOP/REPLAN triggers. Otherwise record `REPLAN`.
- State exactly how Zabel informed ownership, or state that it supplied no
  useful guidance for the reached surface.

## Allowlist and caps

Only these plan files may change from base `4aed2438`:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`.

No Rust, tests, lockfiles, sources, DICE keys, repository data, oracle fixtures
or generated evidence may change. The audit addition cap is 260 lines across
the canonical plan and Stage 4 subplan; the rewritten manifest is capped at
220 lines. Use read-only checks only.

## Review and STOP

Independent selection review must verify the caller path, hashes, docs-only
boundary, Bazel authority and Zabel's guidance-only role. The completed audit
requires another independent review before its selected implementation packet
is committed.

STOP and `REPLAN` for a dirty authority/source checkout; unresolved source
identity; an unbounded or cyclic frontier; behavior requiring Java/JVM code;
copied Zabel behavior; a Rust/test/source edit; a claim beyond the first
unsupported expression; or cap violation.
