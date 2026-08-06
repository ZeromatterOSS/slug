# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-bzlmod-proxy-span-repair-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an accepted, evidence-backed bounded repair contract for the sole
clean-baseline failure blocking the 278-case Bzlmod crate-mode target.

## Goal

Freeze Bazel 9.2 authority, the live source owner, exact span semantics, and a
bounded implementation/evidence successor for
`records_exact_proxy_tag_and_innate_call_spans` without editing Rust.

## Required design

Reconcile the existing expected `use_extension` proxy span (line 2, columns
9–39) with the clean actual starlark-rust call-stack span (line 2, columns
22–22). Use accepted Bazel 9.2 oracle output or pinned Bazel source plus the
live starlark-rust AST/evaluator source to establish whether the location owns
the complete call expression, callee token, or another exact range. Inventory
all consumers of the retained proxy/tag/innate locations and all assignment,
include-file, reuse/merge, and error paths affected by the selected owner.
Choose the smallest source-exact mechanism and freeze focused tests,
downstream validation, platform checks, allowed files, caps, and stop
conditions. Do not weaken/change the expected bytes merely to make Cargo green.

## Allowed paths

- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Reuse or reproduce the clean 277/278 Cargo and Bazel failure only as needed.
Record exact Bazel 9.2 or pinned-source authority, live source/consumer anchors,
the proposed evidence matrix, line arithmetic, and an independent design
review. Run structure, scope, cap, credential-pattern, and `git diff --check`
gates; no implementation suite is authorized in this packet.

## Stop conditions

Stop with REPLAN if Bazel's exact range cannot be established, if the repair
would alter unrelated directive/tag/innate spans or logical file identity, if
it needs retained source text/AST beyond a bounded compact representation, or
if it couples loading semantics, DICE ownership, query/cquery/aquery,
execution/cache, fixtures, host tools, self-hosting, Java/JVM delegation, or
Bazel 8/WORKSPACE compatibility. Do not inspect rc or credential contents.

## Diff budget

- At most 180 net documentation lines. No Rust, BUILD, Cargo, lock, fixture,
  generated-source, CI, or unrelated change.
