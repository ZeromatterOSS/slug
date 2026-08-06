# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-query-unit-test-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one private crate-mode Bazel target passing all 28 query library cases.

## Goal

Map only the now-green `slug_query_v2` library test crate as one private Bazel
target with its declared Tokio dev dependency.

## Required design

Add exactly one private, small `rust_test` with
`crate = ":slug_query_v2"`. Use generated `normal_dev` aliases and deps so the
sole Cargo dev dependency, Tokio, is available without restating production
dependencies. The target owns all 28 library cases. Do not alter the existing
six-case `query_test`, add the 53-case `loading_query.rs` fixture target, or
change Rust source.

## Allowed paths

- `app/slug_query_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the private target with credential-free nightly Bazel; all 28 cases must
pass. Run serial Cargo `-p slug_query_v2 --lib`; the same 28 must pass. Run a
GNU-Windows no-run library-test compile. Run no-repin `bazel mod deps` and
prove all three lock hashes stable. Run archive, exact scope, cap,
credential-pattern, and `git diff --check` gates; clean stale `slugd` before
and after tests and obtain independent latest-diff review.

## Stop conditions

Stop with REPLAN on any Rust, Cargo/lock/fixture/generated-source change, any
other target, test filter, target env/rustc_env/data/tool/runner, process or
binary adapter, source/runfile write, ambient repository/home/PATH dependency,
platform exclusion, Cargo execution from Bazel, rc or credential inspection,
or M2/M5/M6/self-hosting coupling. Do not add a WORKSPACE, `.bazelrc`, CI,
BuildBuddy/cache/RBE, query/cquery/aquery surface, or loading-query fixture
owner.

## Diff budget

- At most 100 net metadata/documentation lines including at most 20 BUILD
  lines. No Rust beyond the accepted predecessor's one test-fixture line,
  Cargo, lock, fixture, generated-source, CI, or unrelated change.
