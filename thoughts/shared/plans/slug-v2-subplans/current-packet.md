# Current Slug V2 Packet

Packet: `WP-4-7A-post-paths-rust-parent-frontier-audit`

Milestone: M7A command/ruleset bootstrap closure.

Result: resume exact `rust/private/rust.bzl` source order after the accepted
paths child, account for cached admitted children, and select the first newly
unsupported eager loading expression or record `REPLAN`.

## Accepted base and audit frontier

Base is `8440742f7` (`Prove exact bazel skylib paths loading`). It freezes exact
bazel_skylib 1.8.2 `lib/paths.bzl` under
`@@bazel_skylib+//lib:paths.bzl`, proving its exported ten-member function
composite without invoking a helper or claiming exact field iteration order.

Resume the authenticated 1,821-line rules_rust 0.73.0
`rust/private/rust.bzl`, SHA-256
`a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`.
Its next direct child is `@bazel_skylib//rules:common_settings.bzl`, already
admitted completely through the earlier toolchain route at SHA-256
`f3bcedef4b2b2cbe9750d61852917954499c4ba5e83d79fb975ec5814eb76d20`.
Do not select duplicate implementation work there.

## Authorities and compatibility discipline

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` and the
authenticated rules_rust/bazel_skylib sources are sole behavior authority.
Replay direct-load and recursive child order exactly. For each child, cite the
accepted packet that closes it or authenticate its source bytes/hash before
classifying its first unsupported eager expression.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architectural guidance only.
Use its module-boundary and frozen closure-graph concepts to check whether a
cached child is genuinely complete. Copy no Zig code, representation, owner
pointer, field order, capture algorithm, diagnostic, identity or behavior.

- **Exact:** authenticated parent/child source order and already accepted exact
  loading slices.
- **Slug-native:** Rust evaluator/frozen-value representation and the audit's
  documentation structure.
- **Unsupported/deferred:** any first unadmitted eager expression selected by
  this audit; all lazy bodies and configured rule/provider/action behavior
  unless separately accepted.

The Buck2 utility review selects no action because this is docs-only and changes
no retained data structure, hash, collection/string, interner, clone path,
graph storage or memory accounting.

## Allowlist, proof and caps

Only these files may change:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `.codex/skills/slug-agent-orchestration/references/routing-log.md` only if the
  audit yields a genuinely reusable or unusual routing decision.

Caps are 0 production and 0 proof additions. Documentation growth must remain
bounded to the authenticated audit result.

Required audit:

1. Resume immediately after `lib/paths.bzl` returns; do not restart at a later
   public ruleset or skip a direct child.
2. Account for complete cached children from accepted evidence, beginning with
   `rules/common_settings.bzl`; do not duplicate their implementation.
3. Authenticate the first newly evaluated child and recursively follow its own
   load order before examining its body.
4. Identify the first unsupported eager loading expression and its narrow
   producer/consumer proof boundary, or record `REPLAN` if no bounded Rust-
   native packet exists.
5. Update the canonical status, stage subplan and this manifest to name one
   next packet with exact/Slug-native/unsupported classification.

No new oracle is required for the docs audit. Any selected parity change must
name existing accepted discriminating Bazel 9.2 evidence or require a later
pinned-source/oracle proof.

## Validation and STOP

Run `git diff --check`, verify only allowlisted documentation changed, and run
`scripts/v2_archive_status.sh` with only its three known archive-only misses.
Independent terminal review must verify source order, cached-child accounting,
selected boundary, compatibility classification, Zabel's guidance-only role
and scope.

STOP and `REPLAN` for Rust changes, skipped source order, a duplicate packet for
an already accepted child, configured semantics, Java/JVM work, copied Zabel
content, dirty authority or an unbounded next packet.

## Immediate predecessor

`8440742f7` accepted the exact paths-child recursive freeze proof under all
caps and validation, then stopped as required when that child returned.
