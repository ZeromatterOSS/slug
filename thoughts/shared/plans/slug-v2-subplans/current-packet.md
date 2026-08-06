# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-bzlmod-unit-test-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one private crate-mode Bazel target passing all 278 Bzlmod library
cases, including its source-owned `current_exe()` child path.

## Goal

Map only the now-green `slug_bzlmod_v2` library test crate as one private Bazel
target with its declared Tokio dev dependency.

## Required design

Add exactly one private, small `rust_test` with
`crate = ":slug_bzlmod_v2"`. Use generated `normal_dev` aliases and deps so the
sole Cargo dev dependency, Tokio, is available without restating production
dependencies. The target owns all 278 library cases. Preserve the existing
`current_exe()` child-marker test without env, data, runner, process adapter, or
filter. Do not add or change an integration target or Rust source.

## Allowed paths

- `app/slug_bzlmod_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the private target with credential-free nightly Bazel; all 278 cases must
pass. Run serial Cargo `-p slug_bzlmod_v2 --lib`; the same 278 must pass. Run a
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
BuildBuddy/cache/RBE, query, cquery, or aquery surface.

## Diff budget

- At most 100 net metadata/documentation lines including at most 20 BUILD
  lines. No Rust beyond the already accepted predecessor correction, Cargo,
  lock, fixture, generated-source, CI, or unrelated change.
