# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-bzlmod-lockfile-scratch-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an exact design for the writable scratch owner needed by the 11 cases
in `slug_bzlmod_v2/tests/lockfile.rs`.

## Goal

Freeze one hermetic Bazel adapter that preserves the lockfile integration's
compile-time manifest-relative path and writable runtime semantics. Do not map
the target or edit source/BUILD/Cargo in this design packet.

## Required design

Reconcile all 11 cases, their exact
`env!("CARGO_MANIFEST_DIR")/../../.codex-cargo-target/slug_bzlmod_v2_tests`
construction, rules_rust compile-time env behavior, Bazel runfiles/sandbox
writability, `TEST_TMPDIR`, and Windows path semantics. Preserve source-defined
PID/name isolation and cleanup. Reject a compile-sandbox absolute path,
source/runfile writes, ambient repository paths, Cargo execution, copied
fixtures, or a platform-only solution. Name the smallest implementation packet
with exact allowed files, adapter attributes, validation, and cap; use
`REPLAN` if no bounded hermetic representation preserves the test semantics.

## Allowed paths

- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Inspect the live test source and pinned rules_rust 0.73 behavior; use an
isolated Bazel 9.2 scratch probe only if static ownership/writability evidence
cannot discriminate the design. Mechanically reconcile all 11 cases. Run
documentation, scope, cap, archive, credential-pattern, and `git diff --check`
gates. No Cargo or repository test target is needed.

## Stop conditions

Stop with REPLAN on any Rust/BUILD/Cargo/lock/fixture/generated-source change,
source/runfile write, ambient repository or home path, stale compile-sandbox
path, copied/archive input, platform exclusion, Cargo execution from Bazel, rc
or credential inspection, or M2/M5/M6/self-hosting coupling. Do not add a
WORKSPACE, `.bazelrc`, CI, BuildBuddy/cache/RBE, query, cquery, or aquery
surface.

## Diff budget

- Documentation only: at most 180 net lines. No Rust, BUILD, Cargo, lock,
  fixture, generated-source, CI, or unrelated change.
