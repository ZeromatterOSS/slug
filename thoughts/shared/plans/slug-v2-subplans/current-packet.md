# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-workspace-unit-test-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one private Bazel crate-mode target covering all 41 workspace unit
cases with the exact dev-only Tokio dependency edge.

## Goal

Map the workspace library unit tests through one `rust_test(crate = ...)`.
Preserve all portable and platform-gated cases and run the target under
credential-free nightly Bazel and serial Cargo.

## Required design

Add exactly one private, small `rust_test(crate = ":slug_workspace_v2")`.
Supply only its Cargo-declared dev-only Tokio edge through the generated
crate-universe `normal_dev` dependency/alias helpers as required by analysis;
inherit all normal deps and sources from the production crate. Do not add a
suite, source list, env, data, tools, fixtures, generated inputs, platform
restrictions, processes, daemons, or serialization.

## Allowed paths

- `app/slug_workspace_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the one label in a credential-free Bazel command with
`--ignore_all_rc_files` and the pinned nightly channel. Run the package's
library tests in one serial Cargo command. Run no-repin `bazel mod deps` and
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

- At most 80 net metadata/documentation lines. No Rust, Cargo, lock, fixture,
  generated-source, CI, or unrelated change.
