# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-cli-production-graph`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: Bazel 9.2 builds the production `slug` binary through exactly the 14 V2
packages over the accepted retained foundation.

## Goal

Add fresh rules_rust BUILD ownership for exactly the 14 V2 packages in the
accepted production closure, including REAPI proto generation, the CLI library,
and `//app/slug_cli_v2:slug`. Build that target locally. Do not map tests yet.

## Required implementation

Map each V2 Cargo library to one local `rust_library`; add the CLI `slug` binary
over its library. Use `cargo_build_script` for `slug_reapi_v2/build.rs` with
exactly the five checked-in proto inputs and wire its OUT_DIR output into the
REAPI library. Preserve Cargo features, target conditions, crate names, local
edges, and external aliases from the accepted crate universe. No handwritten or
checked-in generated Rust is allowed.

## Allowed paths

- `app/slug_{analysis,bep,build_api,bzlmod,cli,commands,core,events,identity,loading,query,reapi,server,workspace}_v2/BUILD.bazel`
- the canonical plan, Stage 10 owner, and this manifest

## Required tests and validation

Run Bazel 9.2 with `--ignore_all_rc_files` and the explicit nightly channel to
build `//app/slug_cli_v2:slug`. Run the matching serial
`cargo check -p slug_cli_v2`, archive, scope, cap, unchanged-lock, credential
pattern, and `git diff --check` gates. Record no test or remote evidence.

## Stop conditions

Stop with REPLAN on any rc/credential inspection or consumption, lock change,
Cargo execution from Bazel, REAPI build script that cannot consume only the five
declared protos, source/generated-source edit, test adapter, or
M2/M5/M6/self-hosting coupling. Do not add a WORKSPACE, `.bazelrc`, fixture,
test target, CI, BuildBuddy/cache/RBE, query, cquery, or aquery surface.

## Diff budget

- BUILD metadata, documentation, and total: at most 750 net lines. No lock,
  Rust, Cargo manifest, generated-source, fixture, test, CI, or unrelated
  change.
