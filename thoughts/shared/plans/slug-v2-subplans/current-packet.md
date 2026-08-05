# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-loading-tests-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: six private Bazel targets covering all 118 source-declared loading unit
and integration cases with their native platform cfg behavior preserved.

## Goal

Map the loading library unit tests through one crate-mode target and preserve
each of its five integration sources as a standalone crate. Run all
platform-applicable cases under credential-free nightly Bazel and serial Cargo.

## Required design

Add one private, small `rust_test(crate = ":slug_loading_v2")`. Add one private,
small standalone target for each of `build_file_loading.rs`,
`bzl_invalidation.rs`, `glob_boundaries.rs`, `glob_invalidation.rs`, and
`native_removed_rules.rs`; each uses the Cargo edition and only its directly
imported local/external crates through exact labels/helpers. Preserve Unix and
Windows source cfgs. Tests synthesize uniquely named workspaces under the test
temporary directory; do not rewrite source paths, serialize targets, or add a
checked-in fixture, env, data, tool, process, daemon, or platform exclusion.

## Allowed paths

- `app/slug_loading_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the six labels in one credential-free Bazel command with
`--ignore_all_rc_files` and the pinned nightly channel. Run the package's
library and integration tests in one serial Cargo command. Run no-repin
`bazel mod deps` and
prove the Cargo, rendering, and module lock hashes unchanged. Run formatting,
archive, scope, cap, credential-pattern, and `git diff --check` gates. Clean
stale `slugd` before and after test execution even though these tests must not
activate it.

## Stop conditions

Stop with REPLAN on any Rust/Cargo/lock/fixture/generated-source change, suite,
shared macro, broad dependency restatement, scratch-path rewrite,
serialization, env/data/tool input, platform exclusion, process/daemon
behavior, Cargo execution from Bazel, rc or
credential inspection, or M2/M5/M6/self-hosting coupling. Do not add a
WORKSPACE, `.bazelrc`, CI, BuildBuddy/cache/RBE, query, cquery, or aquery
surface.

## Diff budget

- At most 190 net metadata/documentation lines. No Rust, Cargo, lock, fixture,
  generated-source, CI, or unrelated change.
