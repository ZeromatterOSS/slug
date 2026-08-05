# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-reapi-integration-test-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one private Bazel target covering the 14 REAPI integration cases, with
the NativeLink service case remaining ignored by default.

## Goal

Map `tests/reapi.rs` as one standalone integration crate. Run its 13 default
cases under credential-free nightly Bazel and serial Cargo without activating
the ignored NativeLink transport case.

## Required design

Add exactly one private standalone target owning `tests/reapi.rs`, using the
Cargo edition and directly depending on `:slug_reapi_v2`,
`//app/slug_build_api_v2`, and the imported `prost` crate through an exact
crate-universe label/helper. Reuse the accepted library/build-script generated
output. Do not set or inherit `SLUG_V2_NATIVELINK_ENDPOINT`; do not add a suite,
service, network tag, env, data, tools, fixtures, or generated-source owner.

## Allowed paths

- `app/slug_reapi_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the one label in a credential-free Bazel command with
`--ignore_all_rc_files` and the pinned nightly channel. Run only the REAPI
integration in one serial Cargo command. Run no-repin `bazel mod deps` and
prove the Cargo, rendering, and module lock hashes unchanged. Run formatting,
archive, scope, cap, credential-pattern, and `git diff --check` gates. Clean
stale `slugd` before and after test execution even though these tests must not
activate it.

## Stop conditions

Stop with REPLAN on any Rust/Cargo/lock/fixture/generated-source change,
activation of the ignored service test, suite, shared macro, broad dependency
restatement, endpoint/env/data/tool input,
platform exclusion, process/daemon behavior, Cargo execution from Bazel, rc or
credential inspection, or M2/M5/M6/self-hosting coupling. Do not add a
WORKSPACE, `.bazelrc`, CI, BuildBuddy/cache/RBE, query, cquery, or aquery
surface.

## Diff budget

- At most 100 net metadata/documentation lines. No Rust, Cargo, lock, fixture,
  generated-source, CI, or unrelated change.
