# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-transitive-v2-test-boundary-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an exact, review-bounded implementation sequence for the tests owned by
the 13 non-CLI V2 packages in the accepted production closure.

## Goal

Inventory every live unit and integration test owned by the 13 non-CLI V2
packages. Freeze source ownership, dev-only dependencies, generated inputs,
fixtures/runfiles, compile/runtime env, platform constraints, process/daemon
lifecycle, and the smallest serial implementation packets. Do not map tests or
edit Rust/BUILD/Cargo/fixtures in this design packet.

## Required design

Reconcile all 13 Cargo manifests, accepted production BUILD targets, `#[test]`
modules, and `tests/**/*.rs` integration crates. Distinguish unit tests reusable
through `rust_test(crate = ...)` from standalone integration crates. Identify
every `CARGO_MANIFEST_DIR`, binary, fixture, generated-source, host-platform,
filesystem, socket, process, and serialization requirement. Group only targets
that share one exact adapter and validation envelope; keep large test owners and
new runfiles boundaries separate. Preserve the CLI integration `REPLAN` and
name the first implementation packet with exact allowed files and line cap.

## Allowed paths

- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Inspect the live manifests, accepted Bazel metadata, test sources, and only the
fixtures they reference. Reconcile package/source/test counts mechanically.
Run documentation, scope, cap, archive, credential-pattern, and
`git diff --check` gates. No Cargo or Bazel build/test command is needed.

## Stop conditions

Stop with REPLAN on any production/test/BUILD/Cargo/lock/fixture/generated-source
change, Cargo execution from Bazel, copied fixture, repository-layout or
canonical external path, ambient daemon/process state, rc/credential inspection,
or M2/M5/M6/self-hosting coupling. Do not add a WORKSPACE, `.bazelrc`, test
target, CI, BuildBuddy/cache/RBE, query, cquery, or aquery surface.

## Diff budget

- Documentation only: at most 720 net lines. No production, test, BUILD, Cargo,
  lock, fixture, generated-source, CI, or unrelated change.
