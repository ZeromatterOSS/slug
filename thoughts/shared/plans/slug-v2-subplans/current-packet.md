# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-cli-library-unit-test-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: Bazel 9.2 runs the one in-crate `slug_cli_v2` unit test over the accepted
production library without entering the blocked integration-runfiles boundary.

## Goal

Add exactly one rules_rust unit-test target for `slug_cli_v2` through the
existing production library target and run it locally. Do not map either CLI
integration source or any fixture.

## Required implementation

Load `rust_test` in `app/slug_cli_v2/BUILD.bazel` and add one private
`slug_cli_v2_test` with `crate = ":slug_cli_v2"`. Reuse the library's sources,
edition, dependency graph, and crate-universe resolution through the `crate`
edge; do not restate or broaden them. Add no data, env, fixture, binary, wrapper,
test suite, process, daemon, platform, or runfiles adapter.

## Allowed paths

- `app/slug_cli_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required tests and validation

Run Bazel 9.2 with `--ignore_all_rc_files` and the explicit nightly channel for
`//app/slug_cli_v2:slug_cli_v2_test`. Run serial
`cargo test -q -p slug_cli_v2 --lib`, a no-repin `bazel mod deps` lock-stability
check, archive, scope, cap, credential-pattern, formatting, and
`git diff --check` gates. Record no integration, fixture, or remote evidence.

## Stop conditions

Stop with REPLAN on any Rust/Cargo/lock/fixture/root BUILD change, integration
source or target, `CARGO_BIN_EXE_slug`, `CARGO_MANIFEST_DIR`, runfiles/data/env
adapter, test semantic change, rc/credential inspection or consumption, or
M2/M5/M6/self-hosting coupling. Do not add a WORKSPACE, `.bazelrc`, CI,
BuildBuddy/cache/RBE, query, cquery, or aquery surface.

## Diff budget

- BUILD metadata and documentation: at most 120 net lines. No Rust, Cargo,
  lock, fixture, generated-source, integration-test, CI, or unrelated change.
