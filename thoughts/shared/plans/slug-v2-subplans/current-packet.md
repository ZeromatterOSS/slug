# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-simple-v2-integration-tests-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: six private Bazel integration-test targets covering the 40 pure cases
owned by `slug_bep_v2`, `slug_build_api_v2`, and `slug_commands_v2`.

## Goal

Map the one BEP, four build-API, and one commands integration crates as exact
standalone `rust_test` targets. Preserve Cargo's per-file crate ownership and
run all 40 cases under credential-free nightly Bazel and serial Cargo.

## Required design

Add one private, small `rust_test` for each live source: `bep.rs`, `actions.rs`,
`ctx.rs`, `depset.rs`, `providers.rs`, and `commands.rs`. Each target owns only
its source, its package library, and any other directly imported local library.
Do not add unit targets or test suites. Do not restate transitive dependencies
or add env, data, tools, fixtures, generated inputs, platform restrictions,
processes, daemons, or serialization.

## Allowed paths

- `app/slug_bep_v2/BUILD.bazel`
- `app/slug_build_api_v2/BUILD.bazel`
- `app/slug_commands_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the six labels in one credential-free Bazel command with
`--ignore_all_rc_files` and the pinned nightly channel. Run the three packages'
integration tests in one serial Cargo command. Run no-repin `bazel mod deps` and
prove the Cargo, rendering, and module lock hashes unchanged. Run formatting,
archive, scope, cap, credential-pattern, and `git diff --check` gates. Clean
stale `slugd` before and after test execution even though these tests must not
activate it.

## Stop conditions

Stop with REPLAN on any Rust/Cargo/lock/fixture/generated-source change, unit
target, suite, shared macro, broad dependency restatement, env/data/tool input,
platform exclusion, process/daemon behavior, Cargo execution from Bazel, rc or
credential inspection, or M2/M5/M6/self-hosting coupling. Do not add a
WORKSPACE, `.bazelrc`, CI, BuildBuddy/cache/RBE, query, cquery, or aquery
surface.

## Diff budget

- At most 180 net metadata/documentation lines. No Rust, Cargo, lock, fixture,
  generated-source, CI, or unrelated change.
