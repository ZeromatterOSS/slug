# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-analysis-tests-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: five private Bazel targets covering all 24 fixture-free analysis unit
and integration cases.

## Goal

Map the analysis library unit tests through one crate-mode target and preserve
each of its four integration sources as a standalone crate. Run all 24 cases
under credential-free nightly Bazel and serial Cargo.

## Required design

Add one private, small `rust_test(crate = ":slug_analysis_v2")` with the exact
declared dev-only Tokio edge. Add one private, small standalone target for each
of `configured_target.rs`, `root_analysis.rs`, `starlark_rule.rs`, and
`toolchain.rs`; each uses the Cargo edition and only its directly imported
local/external crates through explicit labels or generated crate helpers. Do
not add a suite, broad dependency restatement, env, data, tools, fixtures,
generated inputs, platform restrictions, processes, daemons, or serialization.

## Allowed paths

- `app/slug_analysis_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the five labels in one credential-free Bazel command with
`--ignore_all_rc_files` and the pinned nightly channel. Run the package's
library and integration tests in one serial Cargo command. Run no-repin `bazel mod deps` and
prove the Cargo, rendering, and module lock hashes unchanged. Run formatting,
archive, scope, cap, credential-pattern, and `git diff --check` gates. Clean
stale `slugd` before and after test execution even though these tests must not
activate it.

## Stop conditions

Stop with REPLAN on any Rust/Cargo/lock/fixture/generated-source change, suite,
shared macro, broad dependency restatement, env/data/tool input,
platform exclusion, process/daemon behavior, Cargo execution from Bazel, rc or
credential inspection, or M2/M5/M6/self-hosting coupling. Do not add a
WORKSPACE, `.bazelrc`, CI, BuildBuddy/cache/RBE, query, cquery, or aquery
surface.

## Diff budget

- At most 190 net metadata/documentation lines. No Rust, Cargo, lock, fixture,
  generated-source, CI, or unrelated change.
