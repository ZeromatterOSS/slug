# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-events-identity-tests-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: five private Bazel test targets covering all 29 fixture-free cases
owned by `slug_events_v2` and `slug_identity_v2`.

## Goal

Map the events library unit target plus the identity library unit target and
three identity integration crates. Preserve Cargo's unit/integration ownership
and run all 29 cases under credential-free nightly Bazel and serial Cargo.

## Required design

Add one private, small `rust_test(crate = ...)` for each library. Add one
private, small standalone target for each identity integration source:
`label_roundtrip.rs`, `layout.rs`, and `pattern.rs`; each owns only its source
and `:slug_identity_v2`. Do not add a suite or restate production dependencies.
Do not add env, data, tools, fixtures, generated inputs, platform restrictions,
processes, daemons, or serialization.

## Allowed paths

- `app/slug_events_v2/BUILD.bazel`
- `app/slug_identity_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the five labels in one credential-free Bazel command with
`--ignore_all_rc_files` and the pinned nightly channel. Run both packages'
tests in one serial Cargo command. Run no-repin `bazel mod deps` and
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

- At most 140 net metadata/documentation lines. No Rust, Cargo, lock, fixture,
  generated-source, CI, or unrelated change.
