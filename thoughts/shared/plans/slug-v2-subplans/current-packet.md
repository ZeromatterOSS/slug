# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-retained-foundation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a credential-free Bazel 9.2/rules_rust foundation that builds the
retained `dice` and `starlark` roots in the accepted CLI closure.

## Goal

Add the accepted root bzlmod/toolchain/dependency-lock boundary and fresh
rules_rust targets for exactly the 19 retained packages in the production CLI
closure. Build the retained `dice` and `starlark` roots locally. Do not add any
V2 app targets yet.

## Required implementation

Pin `.bazelversion` to 9.2.0 and rules_rust to 0.73.0/BCR integrity. Register
`nightly/2025-09-14`; pass the nightly channel explicitly. Track the validated
root `Cargo.lock`, crate_universe `Cargo.Bazel.lock`, and `MODULE.bazel.lock`
under the accepted synchronization policy. Use `cargo_build_script` for the
three compiler-channel probes and LALRPOP generation, and ordinary
`rust_proc_macro` ownership for the five local derive crates. Replace all 19
intersecting Buck/fbcode BUILD files; do not reuse their macros.

## Allowed paths

- `.gitignore`
- `.bazelversion`
- `MODULE.bazel`
- `MODULE.bazel.lock`
- `BUILD.bazel`
- `Cargo.lock`
- `Cargo.Bazel.lock`
- `scripts/v2_archive_status.sh`
- `allocative/{allocative,allocative_derive}/BUILD.bazel`
- `dice/{dice,dice_error,dice_futures}/BUILD.bazel`
- `gazebo/{cmp_any,display_container,dupe,dupe_derive,gazebo,gazebo_derive,strong_hash,strong_hash_derive}/BUILD.bazel`
- `shed/{lock_free_hashtable,lock_free_vec}/BUILD.bazel`
- `starlark-rust/{starlark,starlark_derive,starlark_map,starlark_syntax}/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest

## Required tests and validation

Run Bazel 9.2 with `--ignore_all_rc_files` and the explicit nightly channel to
build the retained `dice` and `starlark` library roots. Run the matching serial
Cargo checks, formatting, source/archive, scope, cap, lock-diff, credential
pattern, and `git diff --check` gates. Record no remote evidence.

## Stop conditions

Stop with REPLAN on any rc/credential inspection or consumption, private
registry need, untracked/unsynchronized lock, build script or proc macro that
cannot be expressed in the allowlist, Cargo execution from Bazel, V2 app target,
source/generated-source edit, or M2/M5/M6/self-hosting coupling. Do not add a
WORKSPACE, `.bazelrc`, app BUILD, fixture, CI, BuildBuddy/cache/RBE, query,
cquery, or aquery surface.

## Diff budget

- Bazel metadata/lock/BUILD and total: at most 900 net lines. No Rust, Cargo
  manifest, generated-source, fixture, CI, or unrelated change.
