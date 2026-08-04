# Current Slug V2 Packet

Packet: `WP-5-m1-external-restricted-visibility-query-oracle`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: oracle implementation worker
Evidence: the independently reviewed external Restricted-visibility consumer
design, accepted root visibility/NODEP oracle, standalone external
package-group projection, and external query package identity recorded in the
owner plan.

Create only
`tests/v2_oracle/fixtures/external-restricted-visibility-query` with exactly
seven regular files, zero links, five workspace assets, and seven commands.
Keep every existing fixture byte-identical, especially the frozen 20-command
`module-local-override` fixture.

The five assets are root `MODULE.bazel`, root `viewer/BUILD.bazel`, dependency
`MODULE.bazel`, dependency root `BUILD.bazel`, and dependency
`viewer/BUILD.bazel`. Use a direct `local_path_override`; dependency root
declares `leaf(packages = ["//viewer"])`, `top(includes = [":leaf"])`, and
an empty native `filegroup` `restricted` with explicit
`visibility = [":top"]`. Both viewer packages declare one caller filegroup.

Add exactly these commands in order: raw `labels(visibility)`, default `deps`,
`same_pkg_direct_rdeps` of top, factored graph output for the dependency
closure, root-viewer `visible()` rejection, same-external-viewer `visible()`
acceptance, and final Bazel-only `--nonodep_deps deps(...)`. The last row proves
NODEP edge kind only and is not a future Slug acceptance row. Generate all
exact output/order/DOT evidence; do not handwrite it.

Require `/usr/bin/bazel` to report exactly `bazel 9.2.0`. Run one fresh
generation, one distinct-root replay, the full oracle harness, archive/scope/
cap checks, and `git diff --check`; clean scoped Bazel processes and obtain
independent latest-diff review. Cap the fixture at 350 total text lines.

Do not edit Rust, Cargo metadata, tools, schemas, plans beyond the terminal
reviewed result, or any existing fixture. Do not run Slug or Cargo. Stop with
**REPLAN** if exact evidence needs another file/row/asset, a second external
repository or direct-local route, cross-package/repository group loading or
includes, implicit/default or direct pseudo-label visibility, an accepted
dependency-filter flag, wildcard discovery, a new key/route/owner, direct
filesystem observation,
configuration, analysis/actions/execution, JVM, Java bytecode, or Bazel
delegation.
