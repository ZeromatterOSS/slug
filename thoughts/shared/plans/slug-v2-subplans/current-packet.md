# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-query-fixture-free-tests-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: two private Bazel targets covering the 28 query library unit cases and
the six fixture-free parser integration cases.

## Goal

Map the query library unit tests through one crate-mode target and preserve
`tests/query.rs` as one standalone integration crate. Run exactly those 34
cases under credential-free nightly Bazel and serial Cargo.

## Required design

Add one private, small `rust_test(crate = ":slug_query_v2")` with only the
Cargo-declared dev Tokio edge through generated `normal_dev` helpers. Add one
private, small standalone target owning only `tests/query.rs`, using the Cargo
edition and depending only on `:slug_query_v2`. Do not map or filter
`tests/loading_query.rs`; its 53 cases remain whole-target fixture `REPLAN`.
Do not add a suite, env, data, tools, fixtures, generated inputs, platform
restrictions, processes, daemons, or serialization.

## Allowed paths

- `app/slug_query_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the two labels in one credential-free Bazel command with
`--ignore_all_rc_files` and the pinned nightly channel. Run `--lib` plus only
the `query` integration in one serial Cargo command. Run no-repin `bazel mod deps` and
prove the Cargo, rendering, and module lock hashes unchanged. Run formatting,
archive, scope, cap, credential-pattern, and `git diff --check` gates. Clean
stale `slugd` before and after test execution even though these tests must not
activate it.

## Stop conditions

Stop with REPLAN on any Rust/Cargo/lock/fixture/generated-source change,
`loading_query.rs` target/filter, suite, shared macro, broad dependency
restatement, env/data/tool input,
platform exclusion, process/daemon behavior, Cargo execution from Bazel, rc or
credential inspection, or M2/M5/M6/self-hosting coupling. Do not add a
WORKSPACE, `.bazelrc`, CI, BuildBuddy/cache/RBE, query, cquery, or aquery
surface.

## Diff budget

- At most 120 net metadata/documentation lines. No Rust, Cargo, lock, fixture,
  generated-source, CI, or unrelated change.
