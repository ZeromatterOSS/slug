# Current Slug V2 Packet

Packet: `WP-4-7A-post-find-cc-toolchain-utils-frontier-audit`

Milestone: M7A command/ruleset bootstrap closure.

Result: resume exact rules_rust `rust/private/utils.bzl` source order after the
accepted find-toolchain child, account for cached children, and select one
bounded next packet or record `REPLAN`.

## Accepted base and audit frontier

Base is `ee9ef5254` (`Prove exact rules cc toolchain loading`). It freezes exact
rules_cc 0.2.17 `cc/find_cc_toolchain.bzl` under producer
`@@rules_cc+//cc:find_cc_toolchain.bzl` with exact cached child
`@@rules_cc+//cc/common:cc_common.bzl`. Its source-defined eager constants and
three functions survive freeze without execution.

Resume the authenticated 1,032-line rules_rust 0.73.0
`rust/private/utils.bzl`, SHA-256
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`.
The exact direct load sequence is bazel_skylib paths, rules_cc find-toolchain,
rules_cc `cc_common`, rules_cc `CcInfo`, then rules_rust providers. The first two
children have now returned; account for the final three from accepted evidence
before examining the module body.

## Authorities and compatibility discipline

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` and the
authenticated rules_rust/rules_cc sources are sole behavior authority. Replay
the remaining direct children and eager body in exact source order. Lazy
function bodies do not authorize configured/toolchain/action work.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architectural guidance only.
Use its frozen module-graph and declaration-ownership concepts to distinguish
an eager retained value from a lazy function body. Copy no Zig code,
representation, owner pointer, ordering, capture algorithm, diagnostic,
identity or behavior.

- **Exact:** authenticated source order and every already accepted exact child
  or eager loading slice.
- **Slug-native:** Rust frozen-value representation and audit documentation.
- **Unsupported/deferred:** the first unadmitted eager expression selected by
  this audit, every lazy utility function body, configured toolchain behavior,
  allocator semantics and later parent source.

The Buck2 utility review selects no action because this packet is docs-only and
changes no retained data structure, hash, compact collection/string, interner,
clone path, graph storage or memory accounting.

## Allowlist, audit and caps

Only these files may change:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `.codex/skills/slug-agent-orchestration/references/routing-log.md` only for a
  genuinely reusable/unusual routing decision or `REPLAN`.

Caps are 0 production and 0 proof additions. Documentation growth must remain
bounded to the authenticated audit result.

Required audit:

1. Resume immediately after exact `cc/find_cc_toolchain.bzl` returns; do not
   restart at a later direct child or parent body.
2. Account for cached `cc_common`, `CcInfo` and providers children by exact
   accepted evidence; do not duplicate their implementation.
3. Inventory every remaining eager top-level value in utils, distinguishing
   constants/composites from lazy function bodies and their captured children.
4. Identify the first unsupported eager loading expression and its narrow
   producer/consumer proof boundary. If all eager shapes are generically
   admitted, decide whether a bounded exact-source proof exists; do not schedule
   an unbounded 1,032-line fixture merely to preserve momentum.
5. Select one bounded next packet with exact/Slug-native/unsupported classes,
   or record `REPLAN` when no bounded Rust-native packet exists.

Any selected parity change must name accepted discriminating Bazel 9.2 evidence
or require a later pinned-source/oracle proof.

## Validation and STOP

Run `git diff --check`, verify only allowlisted documentation changed, and run
`scripts/v2_archive_status.sh` with only its three known archive-only misses.
Independent terminal review must verify source order, cached-child accounting,
eager/lazy classification, selected bounded boundary, compatibility classes,
Zabel's guidance-only role and scope.

STOP and `REPLAN` for Rust changes, helper invocation, skipped source order,
duplicate accepted work, an unbounded exact-source fixture, configured/allocator
semantics, Java/JVM work, copied Zabel content or dirty authority.

## Immediate predecessor

`ee9ef5254` accepted the exact rules_cc find-toolchain child proof after
correcting its cached child package/target identity and rerunning validation.
