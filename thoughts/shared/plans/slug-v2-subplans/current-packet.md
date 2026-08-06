# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-bzlmod-lockfile-test-semantic-adapter-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one private Bazel target passing all 11 lockfile integration cases with
a hermetic runtime scratch root and unchanged Cargo fallback.

## Goal

Change only the test helper to prefer Bazel's runtime `TEST_TMPDIR`, preserve
the existing Cargo manifest-relative fallback, and map `lockfile.rs` as one
standalone integration target.

## Required design

In `scratch_dir`, choose the root from runtime `TEST_TMPDIR` when present;
otherwise retain `env!("CARGO_MANIFEST_DIR")/../..`. From that root append
`.codex-cargo-target/slug_bzlmod_v2_tests/<name>-<pid>`. Preserve ignored
pre-removal errors, `create_dir_all`, the existing filename, writes, and lack of
post-cleanup. Add exactly one private, small standalone `rust_test` owning
`tests/lockfile.rs`, Cargo edition, and dependency only on
`:slug_bzlmod_v2`. Do not set target env or rustc_env, add a wrapper/runner,
write runfiles, or change production/Cargo/lock/fixture/generated source.

## Allowed paths

- `app/slug_bzlmod_v2/tests/lockfile.rs`
- `app/slug_bzlmod_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the private target with credential-free nightly Bazel; all 11 cases must
pass through `TEST_TMPDIR`. Run serial Cargo `--test lockfile`; all 11 must pass
through the unchanged fallback. Run the GNU-Windows no-run target for this
integration. Run no-repin `bazel mod deps` and prove all three lock hashes
stable. Run Rust formatting, archive, scope, cap, credential-pattern, and
`git diff --check` gates; clean stale `slugd` before and after tests.

## Stop conditions

Stop with REPLAN on any production Rust, Cargo/lock/fixture/generated-source
change, target env/rustc_env/data/tool/runner, source/runfile write, ambient
repository or home path, platform exclusion, Cargo execution from Bazel, rc or
credential inspection, or M2/M5/M6/self-hosting coupling. Do not add a
WORKSPACE, `.bazelrc`, CI, BuildBuddy/cache/RBE, query, cquery, or aquery
surface.

## Diff budget

- At most 100 net lines including at most 45 handwritten test/BUILD lines. No
  production Rust, Cargo, lock, fixture, generated-source, CI, or unrelated
  change.
