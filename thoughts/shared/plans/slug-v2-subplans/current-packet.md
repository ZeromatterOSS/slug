# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-core-runtime-test-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one private standalone Bazel target passing all 13 source-owned core
runtime integration cases without activating the blocked core unit host tools.

## Goal

Map only `app/slug_core_v2/tests/runtime.rs` as one standalone integration
target with its exact direct dependencies.

## Required design

Add exactly one private, small standalone `rust_test` owning
`tests/runtime.rs`, using the Cargo edition. Its direct dependencies are
`:slug_core_v2`, `//app/slug_bzlmod_v2`, `//app/slug_identity_v2`,
`//app/slug_loading_v2`, `//app/slug_query_v2`, and the generated `tempfile`
crate label. Preserve the source-owned Unix cfgs: all 13 cases execute on Unix
and the symlink-alias case remains cfg-excluded elsewhere. Do not add the
141-case crate-mode unit target or any env, data, tool, runner, fixture,
platform constraint, process, service, source adapter, Cargo input, or lock.

## Allowed paths

- `app/slug_core_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the private target with credential-free nightly Bazel; all 13 Unix-active
cases must pass from unique temporary workspaces. Run serial Cargo
`--test runtime`; the same 13 must pass. Compile the integration with the
GNU-Windows no-run target, preserving the source-owned non-Unix cfg reduction.
Run no-repin `bazel mod deps` and prove all three lock hashes stable. Run
archive, scope, cap, credential-pattern, and `git diff --check` gates; clean
stale `slugd` before and after tests.

## Stop conditions

Stop with REPLAN on any Rust source, Cargo/lock/fixture/generated-source change,
unit target, target env/rustc_env/data/tool/runner, host process, source/runfile
write, ambient repository/home/PATH dependency, platform exclusion, Cargo
execution from Bazel, rc or credential inspection, or M2/M5/M6/self-hosting
coupling. Do not add a WORKSPACE, `.bazelrc`, CI, BuildBuddy/cache/RBE, query,
cquery, or aquery surface.

## Diff budget

- At most 100 net metadata/documentation lines including at most 40 BUILD
  lines. No Rust, Cargo, lock, fixture, generated-source, CI, or unrelated
  change.
