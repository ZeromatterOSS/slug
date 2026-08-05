# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-bzlmod-fixture-free-tests-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: eleven private Bazel targets covering the 442 Bzlmod unit and
fixture-free integration cases.

## Goal

Map the Bzlmod library unit tests through one crate-mode target and preserve ten
fixture-free integration sources as standalone crates. Run all 442 cases under
credential-free nightly Bazel and serial Cargo.

## Required design

Add one private `rust_test(crate = ":slug_bzlmod_v2")` and one private
standalone target for each of `dice_inputs.rs`, `nonroot_module_eval.rs`,
`parser.rs`, `registry_dice.rs`, `registry_mvs.rs`, `registry_snapshot.rs`,
`registry_source.rs`, `resolution.rs`, `root_module_dice.rs`, and
`source_preparation_dice.rs`. Each target uses the Cargo edition and only exact
direct local/external dependencies. Preserve Unix cfgs and the unit test that
re-executes `current_exe()` with its private child marker. Do not map
`lockfile.rs`: its 22 cases remain deferred pending a writable
manifest-relative scratch adapter. Do not add env/data/tools/fixtures,
serialization, platform exclusions, external processes, or a broad helper.

## Allowed paths

- `app/slug_bzlmod_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the eleven labels in one credential-free Bazel command with
`--ignore_all_rc_files` and the pinned nightly channel. Run the package's
library tests plus only the ten named integrations in serial Cargo commands;
do not run `lockfile`. Run no-repin `bazel mod deps` and
prove the Cargo, rendering, and module lock hashes unchanged. Run formatting,
archive, scope, cap, credential-pattern, and `git diff --check` gates. Clean
stale `slugd` before and after test execution even though these tests must not
activate it.

## Stop conditions

Stop with REPLAN on any Rust/Cargo/lock/fixture/generated-source change,
`lockfile.rs` target or execution, suite/shared macro, broad dependency
restatement, scratch-path rewrite, serialization, env/data/tool input,
platform exclusion, external process/daemon behavior, Cargo execution from
Bazel, rc or
credential inspection, or M2/M5/M6/self-hosting coupling. Do not add a
WORKSPACE, `.bazelrc`, CI, BuildBuddy/cache/RBE, query, cquery, or aquery
surface.

## Diff budget

- At most 380 net metadata/documentation lines. No Rust, Cargo, lock, fixture,
  generated-source, CI, or unrelated change.
